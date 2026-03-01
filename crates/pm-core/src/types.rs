//! Core types for prediction market trading
//!
//! These are the fundamental data structures used throughout
//! the garden - from seeds to harvests.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which side of a binary market
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Yes,
    No,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Side::Yes => Side::No,
            Side::No => Side::Yes,
        }
    }
}

/// The outcome of a resolved market
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketResult {
    Yes,
    No,
    Cancelled,
}

/// A market candidate being evaluated by the pipeline
///
/// This is the primary data structure that flows through
/// sources -> filters -> scorers -> selectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCandidate {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub current_yes_price: Decimal,
    pub current_no_price: Decimal,
    pub volume_24h: u64,
    pub total_volume: u64,
    pub buy_volume_24h: u64,
    pub sell_volume_24h: u64,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub result: Option<MarketResult>,
    pub price_history: Vec<PricePoint>,

    /// Scores from individual specimens (scorers)
    pub scores: HashMap<String, f64>,
    /// Final combined score after ensemble
    pub final_score: f64,
}

impl MarketCandidate {
    pub fn time_to_close(&self, now: DateTime<Utc>) -> chrono::Duration {
        self.close_time - now
    }

    pub fn is_open(&self, now: DateTime<Utc>) -> bool {
        now >= self.open_time && now < self.close_time
    }
}

impl Default for MarketCandidate {
    fn default() -> Self {
        Self {
            ticker: String::new(),
            title: String::new(),
            category: String::new(),
            current_yes_price: Decimal::ZERO,
            current_no_price: Decimal::ZERO,
            volume_24h: 0,
            total_volume: 0,
            buy_volume_24h: 0,
            sell_volume_24h: 0,
            open_time: Utc::now(),
            close_time: Utc::now(),
            result: None,
            price_history: Vec::new(),
            scores: HashMap::new(),
            final_score: 0.0,
        }
    }
}

/// A single price observation at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub yes_price: Decimal,
    pub volume: u64,
}

/// Static market metadata (from data source)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub open_time: DateTime<Utc>,
    pub close_time: DateTime<Utc>,
    pub result: Option<MarketResult>,
}

/// A single trade record from historical data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub timestamp: DateTime<Utc>,
    pub ticker: String,
    pub price: Decimal,
    pub volume: u64,
    pub taker_side: Side,
}

/// Configuration for a backtest run
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub interval: chrono::Duration,
    pub initial_capital: Decimal,
    pub max_position_size: u64,
    pub max_positions: usize,
}

/// The trading context passed through the pipeline
///
/// Contains current state including portfolio, timestamp,
/// and trading history for this session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingContext {
    pub request_id: String,
    pub timestamp: DateTime<Utc>,
    pub portfolio: crate::Portfolio,
    pub trading_history: Vec<crate::Trade>,
}

impl TradingContext {
    pub fn new(initial_capital: Decimal, start_time: DateTime<Utc>) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            timestamp: start_time,
            portfolio: crate::Portfolio::new(initial_capital),
            trading_history: Vec::new(),
        }
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

/// What action was decided for a candidate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    Enter,
    Exit,
    Skip,
}

impl std::fmt::Display for DecisionAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionAction::Enter => write!(f, "enter"),
            DecisionAction::Exit => write!(f, "exit"),
            DecisionAction::Skip => write!(f, "skip"),
        }
    }
}

impl std::str::FromStr for DecisionAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "enter" => Ok(DecisionAction::Enter),
            "exit" => Ok(DecisionAction::Exit),
            "skip" => Ok(DecisionAction::Skip),
            _ => Err(format!("unknown decision action: {}", s)),
        }
    }
}

/// A decision made about a market candidate
///
/// Captures why a trade was or wasn't taken, for observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub ticker: String,
    pub action: DecisionAction,
    pub side: Option<Side>,
    pub score: f64,
    pub confidence: f64,
    pub scorer_breakdown: HashMap<String, f64>,
    pub reason: Option<String>,
    pub signal_id: Option<i64>,
    pub fill_id: Option<i64>,
    pub latency_ms: Option<i64>,
}

impl Decision {
    pub fn enter(
        ticker: String,
        side: Side,
        score: f64,
        scorer_breakdown: HashMap<String, f64>,
        reason: Option<String>,
    ) -> Self {
        let confidence = Self::compute_confidence(&scorer_breakdown);
        Self {
            id: None,
            timestamp: Utc::now(),
            ticker,
            action: DecisionAction::Enter,
            side: Some(side),
            score,
            confidence,
            scorer_breakdown,
            reason,
            signal_id: None,
            fill_id: None,
            latency_ms: None,
        }
    }

    pub fn exit(ticker: String, side: Side, score: f64, reason: String) -> Self {
        Self {
            id: None,
            timestamp: Utc::now(),
            ticker,
            action: DecisionAction::Exit,
            side: Some(side),
            score,
            confidence: 1.0,
            scorer_breakdown: HashMap::new(),
            reason: Some(reason),
            signal_id: None,
            fill_id: None,
            latency_ms: None,
        }
    }

    pub fn skip(
        ticker: String,
        score: f64,
        scorer_breakdown: HashMap<String, f64>,
        reason: String,
    ) -> Self {
        let confidence = Self::compute_confidence(&scorer_breakdown);
        Self {
            id: None,
            timestamp: Utc::now(),
            ticker,
            action: DecisionAction::Skip,
            side: None,
            score,
            confidence,
            scorer_breakdown,
            reason: Some(reason),
            signal_id: None,
            fill_id: None,
            latency_ms: None,
        }
    }

    fn compute_confidence(breakdown: &HashMap<String, f64>) -> f64 {
        if breakdown.is_empty() {
            return 0.0;
        }
        let scores: Vec<f64> = breakdown.values().copied().collect();
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        let std_dev = variance.sqrt();
        // confidence is inversely related to disagreement
        (1.0 - std_dev.min(1.0)).max(0.0)
    }

    pub fn with_fill_id(mut self, fill_id: i64) -> Self {
        self.fill_id = Some(fill_id);
        self
    }

    pub fn with_latency(mut self, start_time: DateTime<Utc>) -> Self {
        let latency = (Utc::now() - start_time).num_milliseconds();
        self.latency_ms = Some(latency);
        self
    }
}
