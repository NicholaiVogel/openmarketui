//! Configuration types for Kalshi trading

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Backtest,
    Paper,
    Live,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub mode: RunMode,
    pub kalshi: KalshiConfig,
    pub trading: TradingConfig,
    pub persistence: PersistenceConfig,
    pub web: WebConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    #[serde(default)]
    pub fees: FeeConfig,
    #[serde(default)]
    pub paper_execution: PaperExecutionConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeeConfig {
    #[serde(default = "default_taker_rate")]
    pub taker_rate: f64,
    #[serde(default = "default_maker_rate")]
    pub maker_rate: f64,
    #[serde(default = "default_max_per_contract")]
    pub max_per_contract: f64,
    #[serde(default = "default_assume_taker")]
    pub assume_taker: bool,
    #[serde(default = "default_min_edge_after_fees")]
    pub min_edge_after_fees: f64,
}

fn default_taker_rate() -> f64 {
    0.07
}
fn default_maker_rate() -> f64 {
    0.0175
}
fn default_max_per_contract() -> f64 {
    0.02
}
fn default_assume_taker() -> bool {
    true
}
fn default_min_edge_after_fees() -> f64 {
    0.02
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            taker_rate: default_taker_rate(),
            maker_rate: default_maker_rate(),
            max_per_contract: default_max_per_contract(),
            assume_taker: default_assume_taker(),
            min_edge_after_fees: default_min_edge_after_fees(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaperExecutionConfig {
    #[serde(default = "default_spread_bps")]
    pub spread_bps: f64,
    #[serde(default = "default_slippage_bps")]
    pub slippage_bps: f64,
    #[serde(default = "default_impact_bps_per_1pct_24h")]
    pub impact_bps_per_1pct_24h: f64,
    #[serde(default = "default_max_fill_pct_24h")]
    pub max_fill_pct_24h: f64,
    #[serde(default = "default_min_fill_qty")]
    pub min_fill_qty: u64,
    #[serde(default = "default_min_latency_ms")]
    pub min_latency_ms: u64,
    #[serde(default = "default_max_latency_ms")]
    pub max_latency_ms: u64,
    #[serde(default = "default_min_trade_volume_24h")]
    pub min_trade_volume_24h: u64,
    #[serde(default = "default_max_entry_spread_bps")]
    pub max_entry_spread_bps: f64,
    #[serde(default = "default_max_entry_sweep_pct_24h")]
    pub max_entry_sweep_pct_24h: f64,
    #[serde(default = "default_urgent_entry_sweep_pct_24h")]
    pub urgent_entry_sweep_pct_24h: f64,
    #[serde(default = "default_urgency_score_threshold")]
    pub urgency_score_threshold: f64,
    #[serde(default = "default_max_limit_drift_bps")]
    pub max_limit_drift_bps: f64,
    #[serde(default = "default_urgent_max_limit_drift_bps")]
    pub urgent_max_limit_drift_bps: f64,
}

fn default_spread_bps() -> f64 {
    120.0
}
fn default_slippage_bps() -> f64 {
    12.0
}
fn default_impact_bps_per_1pct_24h() -> f64 {
    8.0
}
fn default_max_fill_pct_24h() -> f64 {
    0.01
}
fn default_min_fill_qty() -> u64 {
    1
}
fn default_min_latency_ms() -> u64 {
    120
}
fn default_max_latency_ms() -> u64 {
    700
}
fn default_min_trade_volume_24h() -> u64 {
    10_000
}
fn default_max_entry_spread_bps() -> f64 {
    180.0
}
fn default_max_entry_sweep_pct_24h() -> f64 {
    0.0025
}
fn default_urgent_entry_sweep_pct_24h() -> f64 {
    0.0075
}
fn default_urgency_score_threshold() -> f64 {
    0.80
}
fn default_max_limit_drift_bps() -> f64 {
    20.0
}
fn default_urgent_max_limit_drift_bps() -> f64 {
    80.0
}

impl Default for PaperExecutionConfig {
    fn default() -> Self {
        Self {
            spread_bps: default_spread_bps(),
            slippage_bps: default_slippage_bps(),
            impact_bps_per_1pct_24h: default_impact_bps_per_1pct_24h(),
            max_fill_pct_24h: default_max_fill_pct_24h(),
            min_fill_qty: default_min_fill_qty(),
            min_latency_ms: default_min_latency_ms(),
            max_latency_ms: default_max_latency_ms(),
            min_trade_volume_24h: default_min_trade_volume_24h(),
            max_entry_spread_bps: default_max_entry_spread_bps(),
            max_entry_sweep_pct_24h: default_max_entry_sweep_pct_24h(),
            urgent_entry_sweep_pct_24h: default_urgent_entry_sweep_pct_24h(),
            urgency_score_threshold: default_urgency_score_threshold(),
            max_limit_drift_bps: default_max_limit_drift_bps(),
            urgent_max_limit_drift_bps: default_urgent_max_limit_drift_bps(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct KalshiConfig {
    pub base_url: String,
    pub poll_interval_secs: u64,
    pub rate_limit_per_sec: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradingConfig {
    pub initial_capital: f64,
    pub max_positions: usize,
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub take_profit_pct: Option<f64>,
    pub stop_loss_pct: Option<f64>,
    pub max_hold_hours: Option<i64>,
    #[serde(default = "default_min_time_to_close")]
    pub min_time_to_close_hours: i64,
    #[serde(default = "default_max_time_to_close")]
    pub max_time_to_close_hours: i64,
    #[serde(default = "default_cash_reserve")]
    pub cash_reserve_pct: f64,
    #[serde(default = "default_max_entries_per_tick")]
    pub max_entries_per_tick: usize,
}

fn default_min_time_to_close() -> i64 {
    2
}
fn default_max_time_to_close() -> i64 {
    48
}
fn default_cash_reserve() -> f64 {
    0.20
}
fn default_max_entries_per_tick() -> usize {
    5
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersistenceConfig {
    pub db_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebConfig {
    pub enabled: bool,
    pub bind_addr: String,
    /// Path to Becker's prediction-market-analysis data/ directory for parquet backtesting
    pub parquet_data_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CircuitBreakerConfig {
    pub max_drawdown_pct: f64,
    pub max_daily_loss_pct: f64,
    pub max_positions: Option<usize>,
    pub max_single_position_pct: Option<f64>,
    pub max_consecutive_errors: Option<u32>,
    pub max_fills_per_hour: Option<u32>,
    pub max_fills_per_day: Option<u32>,
}

impl AppConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for KalshiConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.elections.kalshi.com/trade-api/v2".to_string(),
            poll_interval_secs: 300,
            rate_limit_per_sec: 5,
        }
    }
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            initial_capital: 10000.0,
            max_positions: 100,
            kelly_fraction: 0.25,
            max_position_pct: 0.10,
            take_profit_pct: Some(0.50),
            stop_loss_pct: Some(0.99),
            max_hold_hours: Some(48),
            min_time_to_close_hours: 2,
            max_time_to_close_hours: 48,
            cash_reserve_pct: 0.20,
            max_entries_per_tick: 5,
        }
    }
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            db_path: "kalshi-paper.db".to_string(),
        }
    }
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_addr: "127.0.0.1:3030".to_string(),
            parquet_data_dir: None,
        }
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_drawdown_pct: 0.15,
            max_daily_loss_pct: 0.05,
            max_positions: Some(100),
            max_single_position_pct: Some(0.10),
            max_consecutive_errors: Some(5),
            max_fills_per_hour: Some(50),
            max_fills_per_day: Some(200),
        }
    }
}

impl From<CircuitBreakerConfig> for pm_engine::CircuitBreakerConfig {
    fn from(cfg: CircuitBreakerConfig) -> Self {
        pm_engine::CircuitBreakerConfig {
            max_drawdown_pct: cfg.max_drawdown_pct,
            max_daily_loss_pct: cfg.max_daily_loss_pct,
            max_positions: cfg.max_positions,
            max_single_position_pct: cfg.max_single_position_pct,
            max_consecutive_errors: cfg.max_consecutive_errors,
            max_fills_per_hour: cfg.max_fills_per_hour,
            max_fills_per_day: cfg.max_fills_per_day,
        }
    }
}

impl From<FeeConfig> for pm_engine::FeeConfig {
    fn from(cfg: FeeConfig) -> Self {
        pm_engine::FeeConfig {
            taker_rate: cfg.taker_rate,
            maker_rate: cfg.maker_rate,
            max_per_contract: cfg.max_per_contract,
            assume_taker: cfg.assume_taker,
            min_edge_after_fees: cfg.min_edge_after_fees,
        }
    }
}
