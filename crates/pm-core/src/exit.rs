//! Exit management for positions
//!
//! Defines when to prune positions from the garden.

use crate::MarketResult;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Configuration for exit rules
#[derive(Debug, Clone)]
pub struct ExitConfig {
    pub take_profit_pct: f64,
    pub stop_loss_pct: f64,
    pub max_hold_hours: i64,
    pub score_reversal_threshold: f64,
}

impl Default for ExitConfig {
    fn default() -> Self {
        // optimized for prediction markets based on testing
        // - 50% take profit balances locking gains vs letting winners run
        // - stop loss disabled (prices gap through, doesn't help)
        Self {
            take_profit_pct: 0.50,
            stop_loss_pct: 0.99, // effectively disabled
            max_hold_hours: 48,
            score_reversal_threshold: -0.5,
        }
    }
}

impl ExitConfig {
    pub fn conservative() -> Self {
        Self {
            take_profit_pct: 0.15,
            stop_loss_pct: 0.10,
            max_hold_hours: 48,
            score_reversal_threshold: -0.2,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            take_profit_pct: 0.30,
            stop_loss_pct: 0.20,
            max_hold_hours: 120,
            score_reversal_threshold: -0.5,
        }
    }

    /// Optimized for prediction markets with binary outcomes
    /// - disables mechanical stop loss (prices gap through anyway)
    /// - raises take profit to 100% (let winners run)
    /// - relies on signal reversal for early exits
    /// - position sizing limits max loss per trade
    pub fn prediction_market() -> Self {
        Self {
            take_profit_pct: 1.00,          // only exit at +100% (doubled)
            stop_loss_pct: 0.99,            // effectively disabled
            max_hold_hours: 48,             // shorter for 2-day backtest
            score_reversal_threshold: -0.5, // exit on strong signal reversal
        }
    }
}

/// Why we're exiting a position (pruning)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExitReason {
    Resolution(MarketResult),
    TakeProfit { pnl_pct: f64 },
    StopLoss { pnl_pct: f64 },
    TimeStop { hours_held: i64 },
    ScoreReversal { new_score: f64 },
}

/// Signal to exit a position
#[derive(Debug, Clone)]
pub struct ExitSignal {
    pub ticker: String,
    pub reason: ExitReason,
    pub current_price: Decimal,
}
