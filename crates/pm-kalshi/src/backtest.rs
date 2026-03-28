//! Backtesting framework for Kalshi

use crate::data::HistoricalData;
use crate::metrics::{BacktestResult, MetricsCollector};
use crate::pipeline::{
    AlreadyPositionedFilter, BollingerMeanReversionScorer, CategoryWeightedScorer,
    HistoricalMarketSource, LiquidityFilter, MeanReversionScorer, MomentumScorer,
    MultiTimeframeMomentumScorer, OrderFlowScorer, TimeDecayScorer, TimeToCloseFilter,
    TopKSelector, TradingPipeline, VolumeScorer,
};
use crate::web::BacktestProgress;
use chrono::{DateTime, Utc};
use pm_core::{
    BacktestConfig, ExitConfig, Fill, Filter, MarketResult, OrderExecutor, Portfolio, Scorer,
    Selector, Side, Source, Trade, TradeType, TradingContext,
};
use pm_engine::PositionSizingConfig;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tracing::info;

fn resolve_closed_positions(
    portfolio: &mut Portfolio,
    data: &HistoricalData,
    resolved: &mut HashSet<String>,
    at: DateTime<Utc>,
    history: &mut Vec<Trade>,
    metrics: &mut MetricsCollector,
) -> Vec<(String, MarketResult, Option<Decimal>)> {
    let tickers: Vec<String> = portfolio.positions.keys().cloned().collect();
    let mut resolutions = Vec::new();

    for ticker in tickers {
        if resolved.contains(&ticker) {
            continue;
        }

        let Some(result) = data.get_resolution_at(&ticker, at) else {
            continue;
        };

        resolved.insert(ticker.clone());
        let Some(pos) = portfolio.positions.get(&ticker).cloned() else {
            continue;
        };

        let pnl = portfolio.resolve_position(&ticker, result);

        let exit_price = match result {
            MarketResult::Yes => match pos.side {
                Side::Yes => Decimal::ONE,
                Side::No => Decimal::ZERO,
            },
            MarketResult::No => match pos.side {
                Side::Yes => Decimal::ZERO,
                Side::No => Decimal::ONE,
            },
            MarketResult::Cancelled => pos.avg_entry_price,
        };

        let category = data
            .markets
            .get(&ticker)
            .map(|m| m.category.clone())
            .unwrap_or_default();

        let trade = Trade {
            ticker: ticker.clone(),
            side: pos.side,
            quantity: pos.quantity,
            price: exit_price,
            timestamp: at,
            trade_type: TradeType::Resolution,
        };

        history.push(trade.clone());
        metrics.record_trade(&trade, &category);
        resolutions.push((ticker, result, pnl));
    }

    resolutions
}

pub struct BacktestExecutor {
    data: Arc<HistoricalData>,
    slippage_bps: u64,
    max_position_size: u64,
    sizing_config: PositionSizingConfig,
    exit_config: ExitConfig,
}

impl BacktestExecutor {
    pub fn new(data: Arc<HistoricalData>, slippage_bps: u64, max_position_size: u64) -> Self {
        Self {
            data,
            slippage_bps,
            max_position_size,
            sizing_config: PositionSizingConfig::default(),
            exit_config: ExitConfig::default(),
        }
    }

    pub fn with_sizing_config(mut self, config: PositionSizingConfig) -> Self {
        self.sizing_config = config;
        self
    }

    pub fn with_exit_config(mut self, config: ExitConfig) -> Self {
        self.exit_config = config;
        self
    }

    pub fn generate_signals(
        &self,
        candidates: &[pm_core::MarketCandidate],
        context: &TradingContext,
    ) -> Vec<pm_core::Signal> {
        candidates
            .iter()
            .filter(|c| c.final_score != 0.0)
            .filter(|c| !context.portfolio.has_position(&c.ticker))
            .map(|c| {
                let yes_price = c.current_yes_price.to_f64().unwrap_or(0.5);
                let (side, price) = if c.final_score > 0.0 {
                    if yes_price < 0.5 {
                        (Side::Yes, c.current_yes_price)
                    } else {
                        (Side::No, c.current_no_price)
                    }
                } else if yes_price > 0.5 {
                    (Side::No, c.current_no_price)
                } else {
                    (Side::Yes, c.current_yes_price)
                };

                pm_core::Signal {
                    ticker: c.ticker.clone(),
                    side,
                    quantity: self.sizing_config.max_position_size,
                    limit_price: Some(price),
                    reason: format!("backtest: score={:.3}", c.final_score),
                }
            })
            .collect()
    }

    pub fn generate_exit_signals(
        &self,
        context: &TradingContext,
        candidate_scores: &HashMap<String, f64>,
    ) -> Vec<pm_core::ExitSignal> {
        let data = self.data.clone();
        pm_engine::compute_exit_signals(context, candidate_scores, &self.exit_config, &|ticker| {
            data.get_current_price(ticker, context.timestamp)
        })
    }
}

#[async_trait::async_trait]
impl OrderExecutor for BacktestExecutor {
    async fn execute_signal(
        &self,
        signal: &pm_core::Signal,
        context: &TradingContext,
    ) -> Option<Fill> {
        let price = self
            .data
            .get_current_price(&signal.ticker, context.timestamp)?;

        let effective_price = match signal.side {
            Side::Yes => price,
            Side::No => Decimal::ONE - price,
        };

        let slippage = Decimal::new(self.slippage_bps as i64, 4);
        let fill_price = effective_price * (Decimal::ONE + slippage);

        if let Some(limit) = signal.limit_price {
            // Permit small modeled slippage (and minor bar-close mismatch) in backtests.
            let tolerance = Decimal::new(5, 2); // 5%
            if fill_price > limit * (Decimal::ONE + tolerance) {
                return None;
            }
        }

        let cost = fill_price * Decimal::from(signal.quantity);
        let quantity = if cost > context.portfolio.cash {
            let affordable = (context.portfolio.cash / fill_price).to_u64().unwrap_or(0);
            if affordable == 0 {
                return None;
            }
            affordable.min(self.max_position_size)
        } else {
            signal.quantity.min(self.max_position_size)
        };

        Some(Fill {
            ticker: signal.ticker.clone(),
            side: signal.side,
            quantity,
            price: fill_price,
            timestamp: context.timestamp,
            fee: None,
        })
    }

    fn generate_signals(
        &self,
        candidates: &[pm_core::MarketCandidate],
        context: &TradingContext,
    ) -> Vec<pm_core::Signal> {
        self.generate_signals(candidates, context)
    }

    fn generate_exit_signals(
        &self,
        context: &TradingContext,
        candidate_scores: &HashMap<String, f64>,
    ) -> Vec<pm_core::ExitSignal> {
        self.generate_exit_signals(context, candidate_scores)
    }
}

pub struct Backtester {
    config: BacktestConfig,
    data: Arc<HistoricalData>,
    pipeline: TradingPipeline,
    executor: BacktestExecutor,
    progress: Option<Arc<BacktestProgress>>,
    step_callback: Option<Arc<dyn Fn(BacktestLiveSnapshot) + Send + Sync>>,
}

#[derive(Debug, Clone)]
pub struct BacktestLiveSnapshot {
    pub cash: f64,
    pub invested: f64,
    pub equity: f64,
    pub initial_capital: f64,
    pub return_pct: f64,
    pub total_pnl: f64,
    pub open_positions: usize,
    pub fills_this_step: usize,
}

impl Backtester {
    pub fn new(config: BacktestConfig, data: Arc<HistoricalData>) -> Self {
        let pipeline = Self::build_default_pipeline(data.clone(), &config);
        let executor = BacktestExecutor::new(data.clone(), 10, config.max_position_size);

        Self {
            config,
            data,
            pipeline,
            executor,
            progress: None,
            step_callback: None,
        }
    }

    pub fn with_configs(
        config: BacktestConfig,
        data: Arc<HistoricalData>,
        sizing_config: PositionSizingConfig,
        exit_config: ExitConfig,
    ) -> Self {
        let pipeline = Self::build_default_pipeline(data.clone(), &config);
        let executor = BacktestExecutor::new(data.clone(), 10, config.max_position_size)
            .with_sizing_config(sizing_config)
            .with_exit_config(exit_config);

        Self {
            config,
            data,
            pipeline,
            executor,
            progress: None,
            step_callback: None,
        }
    }

    pub fn with_progress(mut self, progress: Arc<BacktestProgress>) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn with_pipeline(mut self, pipeline: TradingPipeline) -> Self {
        self.pipeline = pipeline;
        self
    }

    pub fn with_step_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(BacktestLiveSnapshot) + Send + Sync + 'static,
    {
        self.step_callback = Some(Arc::new(callback));
        self
    }

    fn build_default_pipeline(
        data: Arc<HistoricalData>,
        config: &BacktestConfig,
    ) -> TradingPipeline {
        let sources: Vec<Box<dyn Source>> = vec![Box::new(HistoricalMarketSource::new(data, 24))];

        let filters: Vec<Box<dyn Filter>> = vec![
            // Backtest datasets can be sparse (sampled trades), and many Kalshi
            // markets are short-dated. Use permissive defaults so the simulation
            // can actually participate instead of filtering everything out.
            Box::new(LiquidityFilter::new(10)),
            Box::new(TimeToCloseFilter::new(0, None)),
            Box::new(AlreadyPositionedFilter::new(config.max_position_size)),
        ];

        let scorers: Vec<Box<dyn Scorer>> = vec![
            Box::new(MomentumScorer::new(6)),
            Box::new(MultiTimeframeMomentumScorer::default_windows()),
            Box::new(MeanReversionScorer::new(24)),
            Box::new(BollingerMeanReversionScorer::default_config()),
            Box::new(VolumeScorer::new(6)),
            Box::new(OrderFlowScorer::new()),
            Box::new(TimeDecayScorer::new()),
            Box::new(CategoryWeightedScorer::with_defaults()),
        ];

        let selector: Box<dyn Selector> = Box::new(TopKSelector::new(config.max_positions));

        TradingPipeline::new(sources, filters, scorers, selector, config.max_positions)
    }

    pub async fn run(&self) -> BacktestResult {
        let mut context = TradingContext::new(self.config.initial_capital, self.config.start_time);
        let mut metrics = MetricsCollector::new(self.config.initial_capital);
        let mut resolved_markets: HashSet<String> = HashSet::new();

        let mut current_time = self.config.start_time;

        let total_steps = (self.config.end_time - self.config.start_time).num_seconds()
            / self.config.interval.num_seconds().max(1);

        if let Some(ref progress) = self.progress {
            progress
                .total_steps
                .store(total_steps as u64, Ordering::Relaxed);
            progress
                .phase
                .store(BacktestProgress::PHASE_RUNNING, Ordering::Relaxed);
        }

        info!(
            start = %self.config.start_time,
            end = %self.config.end_time,
            interval_hours = self.config.interval.num_hours(),
            total_steps = total_steps,
            "starting backtest"
        );

        if let Some(ref callback) = self.step_callback {
            callback(self.snapshot_for_context(&context, &HashMap::new(), 0));
        }

        let mut step: u64 = 0;
        while current_time < self.config.end_time {
            let eval_time =
                std::cmp::min(current_time + self.config.interval, self.config.end_time);
            context.timestamp = eval_time;
            context.request_id = uuid::Uuid::new_v4().to_string();

            let resolutions = resolve_closed_positions(
                &mut context.portfolio,
                &self.data,
                &mut resolved_markets,
                eval_time,
                &mut context.trading_history,
                &mut metrics,
            );
            let mut fills_this_step = resolutions.len();
            for (ticker, result, pnl) in resolutions {
                info!(ticker = %ticker, result = ?result, pnl = ?pnl, "market resolved");
            }

            let result = self.pipeline.execute(context.clone()).await;

            let candidate_scores: HashMap<String, f64> = result
                .selected_candidates
                .iter()
                .map(|c| (c.ticker.clone(), c.final_score))
                .collect();

            let exit_signals = self
                .executor
                .generate_exit_signals(&context, &candidate_scores);
            for exit in exit_signals {
                if let Some(position) = context.portfolio.positions.get(&exit.ticker).cloned() {
                    let exit_contract_price = match position.side {
                        Side::Yes => exit.current_price,
                        Side::No => Decimal::ONE - exit.current_price,
                    };
                    let pnl = context
                        .portfolio
                        .close_position(&exit.ticker, exit_contract_price);

                    info!(
                        ticker = %exit.ticker,
                        reason = ?exit.reason,
                        pnl = ?pnl,
                        "exit triggered"
                    );

                    let category = self
                        .data
                        .markets
                        .get(&exit.ticker)
                        .map(|m| m.category.clone())
                        .unwrap_or_default();

                    let exit_price = match position.side {
                        Side::Yes => exit.current_price,
                        Side::No => Decimal::ONE - exit.current_price,
                    };

                    let trade = Trade {
                        ticker: exit.ticker.clone(),
                        side: position.side,
                        quantity: position.quantity,
                        price: exit_price,
                        timestamp: eval_time,
                        trade_type: TradeType::Close,
                    };

                    context.trading_history.push(trade.clone());
                    metrics.record_trade(&trade, &category);
                    fills_this_step += 1;
                }
            }

            let signals = self
                .executor
                .generate_signals(&result.selected_candidates, &context);

            for signal in signals {
                if context.portfolio.positions.len() >= self.config.max_positions {
                    break;
                }

                if let Some(fill) = self.executor.execute_signal(&signal, &context).await {
                    info!(
                        ticker = %fill.ticker,
                        side = ?fill.side,
                        quantity = fill.quantity,
                        price = %fill.price,
                        "executed trade"
                    );

                    context.portfolio.apply_fill(&fill);

                    let category = self
                        .data
                        .markets
                        .get(&fill.ticker)
                        .map(|m| m.category.clone())
                        .unwrap_or_default();

                    let trade = Trade {
                        ticker: fill.ticker.clone(),
                        side: fill.side,
                        quantity: fill.quantity,
                        price: fill.price,
                        timestamp: fill.timestamp,
                        trade_type: TradeType::Open,
                    };

                    context.trading_history.push(trade.clone());
                    metrics.record_trade(&trade, &category);
                    fills_this_step += 1;
                }
            }

            let market_prices = self.get_current_prices(eval_time);
            metrics.record(eval_time, &context.portfolio, &market_prices);
            if let Some(ref callback) = self.step_callback {
                callback(self.snapshot_for_context(&context, &market_prices, fills_this_step));
            }

            step += 1;
            if let Some(ref progress) = self.progress {
                progress.current_step.store(step, Ordering::Relaxed);
            }

            current_time = current_time + self.config.interval;
        }

        let resolutions = resolve_closed_positions(
            &mut context.portfolio,
            &self.data,
            &mut resolved_markets,
            self.config.end_time,
            &mut context.trading_history,
            &mut metrics,
        );
        let final_fills_this_step = resolutions.len();
        for (ticker, result, pnl) in resolutions {
            info!(ticker = %ticker, result = ?result, pnl = ?pnl, "market resolved");
        }

        if let Some(ref callback) = self.step_callback {
            let market_prices = self.get_current_prices(self.config.end_time);
            callback(self.snapshot_for_context(&context, &market_prices, final_fills_this_step));
        }

        info!(
            trades = context.trading_history.len(),
            positions = context.portfolio.positions.len(),
            cash = %context.portfolio.cash,
            "backtest complete"
        );

        metrics.finalize()
    }

    fn get_current_prices(&self, at: DateTime<Utc>) -> HashMap<String, Decimal> {
        self.data
            .markets
            .keys()
            .filter_map(|ticker| {
                self.data
                    .get_current_price(ticker, at)
                    .map(|p| (ticker.clone(), p))
            })
            .collect()
    }

    fn snapshot_for_context(
        &self,
        context: &TradingContext,
        market_prices: &HashMap<String, Decimal>,
        fills_this_step: usize,
    ) -> BacktestLiveSnapshot {
        let invested = context
            .portfolio
            .positions
            .values()
            .map(|p| {
                (p.avg_entry_price * Decimal::from(p.quantity))
                    .to_f64()
                    .unwrap_or(0.0)
            })
            .sum::<f64>();

        let positions_value = context
            .portfolio
            .positions
            .values()
            .map(|p| {
                let price = market_prices
                    .get(&p.ticker)
                    .copied()
                    .unwrap_or(p.avg_entry_price);
                (price * Decimal::from(p.quantity)).to_f64().unwrap_or(0.0)
            })
            .sum::<f64>();

        let cash = context.portfolio.cash.to_f64().unwrap_or(0.0);
        let equity = cash + positions_value;
        let initial_capital = self.config.initial_capital.to_f64().unwrap_or(10000.0);
        let total_pnl = equity - initial_capital;
        let return_pct = if initial_capital > 0.0 {
            total_pnl / initial_capital * 100.0
        } else {
            0.0
        };

        BacktestLiveSnapshot {
            cash,
            invested,
            equity,
            initial_capital,
            return_pct,
            total_pnl,
            open_positions: context.portfolio.positions.len(),
            fills_this_step,
        }
    }
}

pub struct RandomBaseline {
    config: BacktestConfig,
    data: Arc<HistoricalData>,
}

impl RandomBaseline {
    pub fn new(config: BacktestConfig, data: Arc<HistoricalData>) -> Self {
        Self { config, data }
    }

    pub async fn run(&self) -> BacktestResult {
        let mut context = TradingContext::new(self.config.initial_capital, self.config.start_time);
        let mut metrics = MetricsCollector::new(self.config.initial_capital);
        let mut resolved_markets: HashSet<String> = HashSet::new();
        let mut rng_state: u64 = 42;

        let mut current_time = self.config.start_time;

        while current_time < self.config.end_time {
            let eval_time =
                std::cmp::min(current_time + self.config.interval, self.config.end_time);
            context.timestamp = eval_time;

            resolve_closed_positions(
                &mut context.portfolio,
                &self.data,
                &mut resolved_markets,
                eval_time,
                &mut context.trading_history,
                &mut metrics,
            );

            if let Some(fill) = self.try_random_trade(&context, eval_time, &mut rng_state) {
                let category = self
                    .data
                    .markets
                    .get(&fill.ticker)
                    .map(|m| m.category.clone())
                    .unwrap_or_default();

                context.portfolio.apply_fill(&fill);

                let trade = Trade {
                    ticker: fill.ticker.clone(),
                    side: fill.side,
                    quantity: fill.quantity,
                    price: fill.price,
                    timestamp: eval_time,
                    trade_type: TradeType::Open,
                };

                context.trading_history.push(trade.clone());
                metrics.record_trade(&trade, &category);
            }

            let market_prices = self.get_current_prices(eval_time);
            metrics.record(eval_time, &context.portfolio, &market_prices);

            current_time = current_time + self.config.interval;
        }

        resolve_closed_positions(
            &mut context.portfolio,
            &self.data,
            &mut resolved_markets,
            self.config.end_time,
            &mut context.trading_history,
            &mut metrics,
        );

        metrics.finalize()
    }

    fn try_random_trade(
        &self,
        context: &TradingContext,
        at: DateTime<Utc>,
        rng_state: &mut u64,
    ) -> Option<Fill> {
        if context.portfolio.positions.len() >= self.config.max_positions {
            return None;
        }

        let active_markets = self.data.get_active_markets(at);
        let unpositioned: Vec<_> = active_markets
            .iter()
            .filter(|m| !context.portfolio.has_position(&m.ticker))
            .collect();

        if unpositioned.is_empty() {
            return None;
        }

        *rng_state = lcg_next(*rng_state);
        let idx = (*rng_state as usize) % unpositioned.len();
        let market = unpositioned[idx];

        let price = self.data.get_current_price(&market.ticker, at)?;
        let side = if *rng_state % 2 == 0 {
            Side::Yes
        } else {
            Side::No
        };

        let effective_price = match side {
            Side::Yes => price,
            Side::No => Decimal::ONE - price,
        };

        let quantity = self.config.max_position_size.min(
            (context.portfolio.cash / effective_price)
                .to_u64()
                .unwrap_or(0),
        );

        if quantity == 0 {
            return None;
        }

        Some(Fill {
            ticker: market.ticker.clone(),
            side,
            quantity,
            price: effective_price,
            timestamp: at,
            fee: None,
        })
    }

    fn get_current_prices(&self, at: DateTime<Utc>) -> HashMap<String, Decimal> {
        self.data
            .markets
            .keys()
            .filter_map(|ticker| {
                self.data
                    .get_current_price(ticker, at)
                    .map(|p| (ticker.clone(), p))
            })
            .collect()
    }
}

fn lcg_next(state: u64) -> u64 {
    state.wrapping_mul(1103515245).wrapping_add(12345)
}
