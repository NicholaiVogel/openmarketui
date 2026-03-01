//! Historical market source for backtesting

use crate::data::HistoricalData;
use async_trait::async_trait;
use pm_core::{MarketCandidate, Source, TradingContext};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;

pub struct HistoricalMarketSource {
    data: Arc<HistoricalData>,
    lookback_hours: i64,
}

impl HistoricalMarketSource {
    pub fn new(data: Arc<HistoricalData>, lookback_hours: i64) -> Self {
        Self {
            data,
            lookback_hours,
        }
    }
}

#[async_trait]
impl Source for HistoricalMarketSource {
    fn name(&self) -> &'static str {
        "HistoricalMarketSource"
    }

    async fn get_candidates(
        &self,
        context: &TradingContext,
    ) -> Result<Vec<MarketCandidate>, String> {
        let now = context.timestamp;
        let active_markets = self.data.get_active_markets(now);

        let candidates: Vec<MarketCandidate> = active_markets
            .into_iter()
            .filter_map(|market| {
                let current_price = self.data.get_current_price(&market.ticker, now)?;
                let lookback_start = now - chrono::Duration::hours(self.lookback_hours);
                let price_history =
                    self.data
                        .get_price_history(&market.ticker, lookback_start, now);
                let volume_24h = self.data.get_volume_24h(&market.ticker, now);

                let total_volume: u64 = self
                    .data
                    .get_trades_for_market(&market.ticker, market.open_time, now)
                    .iter()
                    .map(|t| t.volume)
                    .sum();

                let (buy_volume_24h, sell_volume_24h) =
                    self.data.get_order_flow_24h(&market.ticker, now);

                Some(MarketCandidate {
                    ticker: market.ticker.clone(),
                    title: market.title.clone(),
                    category: market.category.clone(),
                    current_yes_price: current_price,
                    current_no_price: Decimal::ONE - current_price,
                    volume_24h,
                    total_volume,
                    buy_volume_24h,
                    sell_volume_24h,
                    open_time: market.open_time,
                    close_time: market.close_time,
                    result: market.result,
                    price_history,
                    scores: HashMap::new(),
                    final_score: 0.0,
                })
            })
            .collect();

        Ok(candidates)
    }
}
