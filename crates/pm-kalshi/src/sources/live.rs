//! Live market source from Kalshi API

use crate::api::KalshiClient;
use async_trait::async_trait;
use chrono::Utc;
use pm_core::{MarketCandidate, Source, TradingContext};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;

pub struct LiveKalshiSource {
    client: Arc<KalshiClient>,
}

impl LiveKalshiSource {
    pub fn new(client: Arc<KalshiClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for LiveKalshiSource {
    fn name(&self) -> &'static str {
        "LiveKalshiSource"
    }

    async fn get_candidates(
        &self,
        _context: &TradingContext,
    ) -> Result<Vec<MarketCandidate>, String> {
        let markets = self
            .client
            .get_open_markets()
            .await
            .map_err(|e| format!("API error: {}", e))?;

        let _now = Utc::now();

        let mut candidates = Vec::with_capacity(markets.len());

        for market in markets {
            let yes_price = market.mid_yes_price();
            if yes_price <= 0.0 || yes_price >= 1.0 {
                continue;
            }

            let yes_dec = Decimal::try_from(yes_price).unwrap_or(Decimal::new(50, 2));
            let no_dec = Decimal::ONE - yes_dec;

            let volume_24h = market.volume_24h.max(0) as u64;
            let total_volume = market.volume.max(0) as u64;

            let price_history = Vec::new();
            let buy_vol = 0u64;
            let sell_vol = 0u64;

            let category = market.category_from_event();

            candidates.push(MarketCandidate {
                ticker: market.ticker,
                title: market.title,
                category,
                current_yes_price: yes_dec,
                current_no_price: no_dec,
                volume_24h,
                total_volume,
                buy_volume_24h: buy_vol,
                sell_volume_24h: sell_vol,
                open_time: market.open_time,
                close_time: market.close_time,
                result: None,
                price_history,
                scores: HashMap::new(),
                final_score: 0.0,
            });
        }

        Ok(candidates)
    }
}
