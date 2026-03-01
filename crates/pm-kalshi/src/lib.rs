//! pm-kalshi: Kalshi prediction market trading engine
//!
//! This crate provides:
//! - Kalshi API client
//! - Market data sources (live and historical)
//! - Paper trading engine with circuit breaker
//! - Backtesting framework
//! - Web dashboard

pub mod api;
pub mod backtest;
pub mod config;
pub mod data;
pub mod engine;
pub mod metrics;
pub mod pipeline;
pub mod sources;
pub mod web;

// Re-export key types
pub use api::KalshiClient;
pub use backtest::{Backtester, RandomBaseline};
pub use config::{AppConfig, CircuitBreakerConfig, KalshiConfig, TradingConfig, WebConfig};
pub use data::HistoricalData;
pub use engine::{EngineState, EngineStatus, PaperTradingEngine, TickMetrics};
pub use metrics::{BacktestResult, MetricsCollector};
pub use pipeline::{
    HistoricalMarketSource, LiveKalshiSource, ThresholdSelector, TopKSelector, TradingPipeline,
};
pub use sources::PaperExecutor;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};

pub fn parse_date(s: &str) -> Result<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
    }

    Err(anyhow::anyhow!("could not parse date: {}", s))
}
