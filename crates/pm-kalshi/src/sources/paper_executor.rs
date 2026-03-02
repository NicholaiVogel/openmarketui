//! Paper executor for simulated trading

use async_trait::async_trait;
use pm_core::{ExitSignal, Fill, MarketCandidate, OrderExecutor, Side, Signal, TradingContext};
use pm_engine::{candidate_to_signal, compute_exit_signals, FeeConfig, PositionSizingConfig};
use pm_store::SqliteStore;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PaperExecutor {
    max_position_size: u64,
    sizing_config: PositionSizingConfig,
    exit_config: pm_core::ExitConfig,
    fee_config: FeeConfig,
    store: Arc<SqliteStore>,
    current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
}

impl PaperExecutor {
    pub fn new(
        max_position_size: u64,
        sizing_config: PositionSizingConfig,
        exit_config: pm_core::ExitConfig,
        fee_config: FeeConfig,
        store: Arc<SqliteStore>,
    ) -> Self {
        Self {
            max_position_size,
            sizing_config,
            exit_config,
            fee_config,
            store,
            current_prices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_prices(&self, prices: HashMap<String, Decimal>) {
        let mut current = self.current_prices.write().await;
        *current = prices;
    }

    pub async fn get_current_prices(&self) -> HashMap<String, Decimal> {
        self.current_prices.read().await.clone()
    }
}

#[async_trait]
impl OrderExecutor for PaperExecutor {
    async fn execute_signal(&self, signal: &Signal, context: &TradingContext) -> Option<Fill> {
        let prices = self.current_prices.read().await;
        let market_price = prices.get(&signal.ticker).copied()?;

        let effective_price = match signal.side {
            Side::Yes => market_price,
            Side::No => Decimal::ONE - market_price,
        };

        if let Some(limit) = signal.limit_price {
            let tolerance = Decimal::new(5, 2);
            if effective_price > limit * (Decimal::ONE + tolerance) {
                return None;
            }
        }

        let cost = effective_price * Decimal::from(signal.quantity);
        let quantity = if cost > context.portfolio.cash {
            let affordable = (context.portfolio.cash / effective_price)
                .to_u64()
                .unwrap_or(0);
            if affordable == 0 {
                return None;
            }
            affordable
        } else {
            signal.quantity
        };

        // calculate fee for this trade
        let price_f64 = effective_price.to_f64().unwrap_or(0.5);
        let fee_amount = self.fee_config.calculate(quantity, price_f64);
        let fee = Decimal::try_from(fee_amount).ok();

        let fill = Fill {
            ticker: signal.ticker.clone(),
            side: signal.side,
            quantity,
            price: effective_price,
            timestamp: context.timestamp,
            fee,
        };

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
        let prices = self.current_prices.try_read();
        match prices {
            Ok(prices) => {
                let prices_ref = prices.clone();
                compute_exit_signals(context, candidate_scores, &self.exit_config, &|ticker| {
                    prices_ref.get(ticker).copied()
                })
            }
            Err(_) => Vec::new(),
        }
    }
}
