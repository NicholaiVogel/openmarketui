//! Trading signals and fills
//!
//! Signals are intents to trade. Fills are executed trades (harvests).

use crate::Side;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// An intent to trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub limit_price: Option<Decimal>,
    #[serde(default)]
    pub urgency_score: f64,
    pub reason: String,
}

/// An executed trade (a harvest from the garden)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee: Option<Decimal>,
}
