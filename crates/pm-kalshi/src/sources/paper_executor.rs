//! Paper executor for simulated trading

use crate::config::PaperExecutionConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pm_core::{ExitSignal, Fill, MarketCandidate, OrderExecutor, Side, Signal, TradingContext};
use pm_engine::{candidate_to_signal, compute_exit_signals, FeeConfig, PositionSizingConfig};
use pm_store::SqliteStore;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
struct MarketSnapshot {
    yes_mid: Decimal,
    volume_24h: u64,
    yes_spread_bps: Option<f64>,
    no_spread_bps: Option<f64>,
}

pub struct PaperExecutor {
    max_position_size: u64,
    sizing_config: PositionSizingConfig,
    exit_config: pm_core::ExitConfig,
    fee_config: FeeConfig,
    execution_config: PaperExecutionConfig,
    store: Arc<SqliteStore>,
    market_state: Arc<RwLock<HashMap<String, MarketSnapshot>>>,
}

impl PaperExecutor {
    pub fn new(
        max_position_size: u64,
        sizing_config: PositionSizingConfig,
        exit_config: pm_core::ExitConfig,
        fee_config: FeeConfig,
        execution_config: PaperExecutionConfig,
        store: Arc<SqliteStore>,
    ) -> Self {
        Self {
            max_position_size,
            sizing_config,
            exit_config,
            fee_config,
            execution_config,
            store,
            market_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_market_state(&self, candidates: &[MarketCandidate]) {
        let mut state = self.market_state.write().await;
        state.clear();

        for c in candidates {
            state.insert(
                c.ticker.clone(),
                MarketSnapshot {
                    yes_mid: c.current_yes_price,
                    volume_24h: c.volume_24h,
                    yes_spread_bps: c.scores.get("tradeability_yes_spread_bps").copied(),
                    no_spread_bps: c.scores.get("tradeability_no_spread_bps").copied(),
                },
            );
        }
    }

    pub async fn get_current_prices(&self) -> HashMap<String, Decimal> {
        self.market_state
            .read()
            .await
            .iter()
            .map(|(ticker, snap)| (ticker.clone(), snap.yes_mid))
            .collect()
    }

    pub async fn execute_exit_fill(
        &self,
        ticker: &str,
        side: Side,
        requested_qty: u64,
        timestamp: DateTime<Utc>,
        fallback_yes_price: Option<Decimal>,
    ) -> Option<Fill> {
        self.execute_order(
            ticker,
            side,
            requested_qty,
            None,
            None,
            false,
            0.0,
            false,
            timestamp,
            fallback_yes_price,
        )
        .await
    }

    fn contract_mid_from_yes(yes_mid: Decimal, side: Side) -> Decimal {
        match side {
            Side::Yes => yes_mid,
            Side::No => Decimal::ONE - yes_mid,
        }
    }

    fn clamp_contract_price(price: f64) -> f64 {
        price.max(0.001).min(0.999)
    }

    fn spread_adjusted_price(&self, mid_price: f64, is_buy: bool) -> f64 {
        let half_spread = (self.execution_config.spread_bps / 10_000.0) / 2.0;
        if is_buy {
            Self::clamp_contract_price(mid_price * (1.0 + half_spread))
        } else {
            Self::clamp_contract_price(mid_price * (1.0 - half_spread))
        }
    }

    fn slippage_adjusted_price(
        &self,
        spread_price: f64,
        requested_qty: u64,
        volume_24h: u64,
        is_buy: bool,
    ) -> f64 {
        let volume = volume_24h.max(1) as f64;
        let requested_pct_of_24h = requested_qty as f64 / volume;
        let impact_bps =
            (requested_pct_of_24h * 100.0) * self.execution_config.impact_bps_per_1pct_24h;
        let total_bps = (self.execution_config.slippage_bps + impact_bps).max(0.0);
        let slippage = total_bps / 10_000.0;

        if is_buy {
            Self::clamp_contract_price(spread_price * (1.0 + slippage))
        } else {
            Self::clamp_contract_price(spread_price * (1.0 - slippage))
        }
    }

    fn partial_fill_qty(&self, requested_qty: u64, volume_24h: u64) -> u64 {
        if requested_qty == 0 {
            return 0;
        }

        let liq_cap = ((volume_24h as f64) * self.execution_config.max_fill_pct_24h).round() as u64;
        let liq_cap = liq_cap.max(self.execution_config.min_fill_qty);
        requested_qty.min(liq_cap)
    }

    fn entry_limit_tolerance_bps(&self, urgency_score: f64) -> f64 {
        if urgency_score.abs() >= self.execution_config.urgency_score_threshold {
            self.execution_config.urgent_max_limit_drift_bps
        } else {
            self.execution_config.max_limit_drift_bps
        }
    }

    fn entry_sweep_cap(&self, volume_24h: u64, urgency_score: f64) -> u64 {
        let pct = if urgency_score.abs() >= self.execution_config.urgency_score_threshold {
            self.execution_config.urgent_entry_sweep_pct_24h
        } else {
            self.execution_config.max_entry_sweep_pct_24h
        };

        ((volume_24h as f64) * pct)
            .floor()
            .max(self.execution_config.min_fill_qty as f64) as u64
    }

    fn snapshot_spread_bps(snapshot: &MarketSnapshot, side: Side) -> Option<f64> {
        match side {
            Side::Yes => snapshot.yes_spread_bps.or(snapshot.no_spread_bps),
            Side::No => snapshot.no_spread_bps.or(snapshot.yes_spread_bps),
        }
    }

    fn enforce_entry_tradeability(
        &self,
        ticker: &str,
        side: Side,
        requested_qty: u64,
        urgency_score: f64,
        snapshot: &MarketSnapshot,
    ) -> Option<u64> {
        if snapshot.volume_24h < self.execution_config.min_trade_volume_24h {
            tracing::debug!(
                ticker = %ticker,
                volume_24h = snapshot.volume_24h,
                min_volume_24h = self.execution_config.min_trade_volume_24h,
                "entry rejected: insufficient market depth"
            );
            return None;
        }

        if let Some(spread_bps) = Self::snapshot_spread_bps(snapshot, side) {
            if spread_bps > self.execution_config.max_entry_spread_bps {
                tracing::debug!(
                    ticker = %ticker,
                    spread_bps = spread_bps,
                    max_spread_bps = self.execution_config.max_entry_spread_bps,
                    "entry rejected: spread too wide"
                );
                return None;
            }
        }

        let cap = self.entry_sweep_cap(snapshot.volume_24h, urgency_score);
        if requested_qty <= cap {
            return Some(requested_qty);
        }

        if urgency_score.abs() >= self.execution_config.urgency_score_threshold {
            tracing::debug!(
                ticker = %ticker,
                requested_qty,
                capped_qty = cap,
                urgency_score,
                "urgent entry downsized to avoid sweeping the book"
            );
            Some(cap)
        } else {
            tracing::debug!(
                ticker = %ticker,
                requested_qty,
                cap_qty = cap,
                urgency_score,
                threshold = self.execution_config.urgency_score_threshold,
                "entry rejected: sweep too aggressive for current urgency"
            );
            None
        }
    }

    fn latency_ms(&self, requested_qty: u64, filled_qty: u64) -> u64 {
        let min_latency = self.execution_config.min_latency_ms;
        let max_latency = self.execution_config.max_latency_ms.max(min_latency);

        if max_latency == min_latency || requested_qty == 0 {
            return min_latency;
        }

        let fill_ratio = (filled_qty as f64 / requested_qty as f64).clamp(0.0, 1.0);
        let stress = 1.0 - fill_ratio;
        let delta = (max_latency - min_latency) as f64 * stress;
        min_latency + delta.round() as u64
    }

    async fn execute_order(
        &self,
        ticker: &str,
        side: Side,
        requested_qty: u64,
        limit_price: Option<Decimal>,
        available_cash: Option<Decimal>,
        is_buy: bool,
        urgency_score: f64,
        enforce_entry_tradeability: bool,
        timestamp: DateTime<Utc>,
        fallback_yes_price: Option<Decimal>,
    ) -> Option<Fill> {
        if requested_qty == 0 {
            return None;
        }

        let (snapshot, used_fallback_snapshot) = {
            let state = self.market_state.read().await;
            if let Some(snap) = state.get(ticker) {
                (snap.clone(), false)
            } else {
                (
                    MarketSnapshot {
                        yes_mid: fallback_yes_price?,
                        volume_24h: requested_qty.saturating_mul(100).max(1),
                        yes_spread_bps: None,
                        no_spread_bps: None,
                    },
                    true,
                )
            }
        };

        let execution_request_qty = if is_buy && enforce_entry_tradeability {
            self.enforce_entry_tradeability(ticker, side, requested_qty, urgency_score, &snapshot)?
        } else {
            requested_qty
        };

        let mid_contract = Self::contract_mid_from_yes(snapshot.yes_mid, side).to_f64()?;
        let spread_price = self.spread_adjusted_price(mid_contract, is_buy);
        let slipped_price = self.slippage_adjusted_price(
            spread_price,
            execution_request_qty,
            snapshot.volume_24h,
            is_buy,
        );
        let mut fill_price = Decimal::from_f64(slipped_price)?;

        if let Some(limit) = limit_price {
            let tolerance_bps = if is_buy && enforce_entry_tradeability {
                self.entry_limit_tolerance_bps(urgency_score)
            } else {
                500.0 // keep wider tolerance for non-entry execution paths
            };
            let tolerance = Decimal::from_f64((tolerance_bps / 10_000.0).max(0.0))?;
            if is_buy && fill_price > limit * (Decimal::ONE + tolerance) {
                return None;
            }
            if !is_buy && fill_price < limit * (Decimal::ONE - tolerance) {
                return None;
            }
        }

        let mut quantity = if !is_buy && used_fallback_snapshot {
            execution_request_qty
        } else {
            self.partial_fill_qty(execution_request_qty, snapshot.volume_24h)
        };
        if quantity == 0 {
            return None;
        }

        let price_f64 = fill_price.to_f64()?;
        if let Some(cash) = available_cash {
            loop {
                if quantity == 0 {
                    return None;
                }
                let tentative_cost = fill_price * Decimal::from(quantity);
                let tentative_fee =
                    Decimal::from_f64(self.fee_config.calculate(quantity, price_f64))
                        .unwrap_or(Decimal::ZERO);
                if tentative_cost + tentative_fee <= cash {
                    break;
                }
                quantity -= 1;
            }
        }

        // recompute fee from final quantity
        let fee = Decimal::from_f64(self.fee_config.calculate(quantity, price_f64));

        // model exchange/queue latency before fill appears
        let latency_ms = self.latency_ms(execution_request_qty, quantity);
        if latency_ms > 0 {
            tokio::time::sleep(Duration::from_millis(latency_ms)).await;
        }
        let fill_timestamp = timestamp + chrono::Duration::milliseconds(latency_ms as i64);

        // normalize final price bounds after any numeric conversions
        let bounded = Self::clamp_contract_price(fill_price.to_f64().unwrap_or(0.5));
        fill_price = Decimal::from_f64(bounded)?;

        Some(Fill {
            ticker: ticker.to_string(),
            side,
            quantity,
            price: fill_price,
            timestamp: fill_timestamp,
            fee,
        })
    }
}

#[async_trait]
impl OrderExecutor for PaperExecutor {
    async fn execute_signal(&self, signal: &Signal, context: &TradingContext) -> Option<Fill> {
        let fill = self
            .execute_order(
                &signal.ticker,
                signal.side,
                signal.quantity.min(self.max_position_size),
                signal.limit_price,
                Some(context.portfolio.cash),
                true,
                signal.urgency_score,
                true,
                context.timestamp,
                None,
            )
            .await?;

        if let Err(e) = self.store.record_fill(&fill, None, None).await {
            tracing::error!(error = %e, "failed to persist fill");
        }

        Some(fill)
    }

    fn generate_signals(
        &self,
        candidates: &[MarketCandidate],
        context: &TradingContext,
    ) -> Vec<Signal> {
        candidates
            .iter()
            .filter_map(|c| {
                candidate_to_signal(
                    c,
                    context,
                    &self.sizing_config,
                    &self.fee_config,
                    self.max_position_size,
                )
            })
            .collect()
    }

    fn generate_exit_signals(
        &self,
        context: &TradingContext,
        candidate_scores: &HashMap<String, f64>,
    ) -> Vec<ExitSignal> {
        let prices = self.market_state.try_read();
        match prices {
            Ok(prices) => {
                let prices_ref = prices.clone();
                compute_exit_signals(context, candidate_scores, &self.exit_config, &|ticker| {
                    prices_ref.get(ticker).map(|s| s.yes_mid)
                })
            }
            Err(_) => Vec::new(),
        }
    }
}
