//! Kalshi API response types

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MarketsResponse {
    pub markets: Vec<ApiMarket>,
    #[serde(default)]
    pub cursor: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiMarket {
    pub ticker: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub event_ticker: String,
    #[serde(default)]
    pub status: String,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    #[serde(default)]
    pub yes_ask: i64,
    #[serde(default)]
    pub yes_bid: i64,
    #[serde(default)]
    pub no_ask: i64,
    #[serde(default)]
    pub no_bid: i64,
    #[serde(default)]
    pub last_price: i64,
    #[serde(default)]
    pub volume: i64,
    #[serde(default)]
    pub volume_24h: i64,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub subtitle: String,
}

impl ApiMarket {
    /// Returns yes price as a fraction (0.0 - 1.0)
    /// Prices from API are in cents (0-100)
    pub fn mid_yes_price(&self) -> f64 {
        let bid = self.yes_bid as f64 / 100.0;
        let ask = self.yes_ask as f64 / 100.0;

        if bid > 0.0 && ask > 0.0 {
            (bid + ask) / 2.0
        } else if bid > 0.0 {
            bid
        } else if ask > 0.0 {
            ask
        } else if self.last_price > 0 {
            self.last_price as f64 / 100.0
        } else {
            0.0
        }
    }

    pub fn category_from_event(&self) -> String {
        let lower = self.event_ticker.to_lowercase();
        if lower.contains("nba") || lower.contains("nfl") || lower.contains("sport") {
            "sports".to_string()
        } else if lower.contains("btc") || lower.contains("crypto") || lower.contains("eth") {
            "crypto".to_string()
        } else if lower.contains("weather") || lower.contains("temp") {
            "weather".to_string()
        } else if lower.contains("econ")
            || lower.contains("fed")
            || lower.contains("cpi")
            || lower.contains("gdp")
        {
            "economics".to_string()
        } else if lower.contains("elect")
            || lower.contains("polit")
            || lower.contains("trump")
            || lower.contains("biden")
        {
            "politics".to_string()
        } else {
            "other".to_string()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradesResponse {
    #[serde(default)]
    pub trades: Vec<ApiTrade>,
    #[serde(default)]
    pub cursor: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiTrade {
    #[serde(default)]
    pub trade_id: String,
    #[serde(default)]
    pub ticker: String,
    pub created_time: DateTime<Utc>,
    #[serde(default)]
    pub yes_price: i64,
    #[serde(default)]
    pub no_price: i64,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub taker_side: String,
}
