use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait HasTicker {
    fn ticker(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionResponse {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub side: String,
    pub quantity: u64,
    pub entry_price: f64,
    pub current_price: Option<f64>,
    pub entry_time: String,
    pub close_time: Option<String>,
    pub unrealized_pnl: f64,
    pub pnl_pct: f64,
    pub hours_held: i64,
}

impl HasTicker for PositionResponse {
    fn ticker(&self) -> &str {
        &self.ticker
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketResponse {
    pub ticker: String,
    pub title: String,
    pub category: Option<String>,
    pub status: String,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub volume_24h: Option<f64>,
    pub in_watchlist: bool,
}

impl HasTicker for MarketResponse {
    fn ticker(&self) -> &str {
        &self.ticker
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedResponse {
    pub name: String,
    pub description: String,
    pub specimen_count: usize,
    pub active_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecimenResponse {
    pub name: String,
    pub bed: String,
    pub status: String,
    pub weight: f64,
    pub hit_rate: Option<f64>,
    pub avg_contribution: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedWithSpecimens {
    #[serde(flatten)]
    pub bed: BedResponse,
    pub specimens: Vec<SpecimenResponse>,
}

#[derive(Debug, Serialize)]
pub struct ScorerToggleRequest {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct DataFetchRequest {
    pub start_date: String,
    pub end_date: String,
    pub trades_per_day: usize,
    pub fetch_markets: bool,
    pub fetch_trades: bool,
}

#[derive(Debug, Serialize)]
pub struct BacktestRequest {
    pub start: String,
    pub end: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capital: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_positions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_hours: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kelly_fraction: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_hold_hours: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestStatusResponse {
    pub status: String,
    pub run_id: Option<String>,
    pub elapsed_secs: Option<u64>,
    pub error: Option<String>,
    pub phase: Option<String>,
    pub current_step: Option<u64>,
    pub total_steps: Option<u64>,
    pub progress_pct: Option<f64>,
    pub live_snapshot: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct SessionStartRequest {
    pub mode: String,
    pub config: SessionConfig,
}

#[derive(Debug, Serialize)]
pub struct SessionConfig {
    pub initial_capital: f64,
    pub max_positions: usize,
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub max_hold_hours: i64,
    pub min_time_to_close_hours: i64,
    pub max_time_to_close_hours: i64,
    pub cash_reserve_pct: f64,
    pub max_entries_per_tick: usize,
    pub fees: SessionFeeConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtest_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtest_end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtest_interval_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SessionFeeConfig {
    pub taker_rate: f64,
    pub maker_rate: f64,
    pub max_per_contract: f64,
    pub assume_taker: bool,
    pub min_edge_after_fees: f64,
}
