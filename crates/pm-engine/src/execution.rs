//! Execution logic for position sizing and signal generation
//!
//! This module provides:
//! - Kelly criterion position sizing
//! - Signal generation from scored candidates
//! - Exit signal computation
//! - Fee-aware trade filtering

use crate::FeeConfig;
use pm_core::{ExitConfig, ExitReason, ExitSignal, MarketCandidate, Side, Signal, TradingContext};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use tracing::trace;

/// Configuration for position sizing
#[derive(Debug, Clone)]
pub struct PositionSizingConfig {
    pub kelly_fraction: f64,
    pub max_position_pct: f64,
    pub min_position_size: u64,
    pub max_position_size: u64,
}

impl Default for PositionSizingConfig {
    fn default() -> Self {
        Self {
            kelly_fraction: 0.40,
            max_position_pct: 0.30,
            min_position_size: 10,
            max_position_size: 1000,
        }
    }
}

impl PositionSizingConfig {
    pub fn conservative() -> Self {
        Self {
            kelly_fraction: 0.1,
            max_position_pct: 0.1,
            min_position_size: 10,
            max_position_size: 500,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            kelly_fraction: 0.5,
            max_position_pct: 0.4,
            min_position_size: 10,
            max_position_size: 2000,
        }
    }
}

/// Maps scoring edge [-inf, +inf] to win probability [0, 1]
pub fn edge_to_win_probability(edge: f64) -> f64 {
    (1.0 + edge.tanh()) / 2.0
}

/// Calculate Kelly criterion position size
pub fn kelly_size(edge: f64, price: f64, bankroll: f64, config: &PositionSizingConfig) -> u64 {
    if edge.abs() < 0.01 || price <= 0.0 || price >= 1.0 {
        return 0;
    }

    let win_prob = edge_to_win_probability(edge);
    let odds = (1.0 - price) / price;

    if odds <= 0.0 {
        return 0;
    }

    let kelly = (odds * win_prob - (1.0 - win_prob)) / odds;
    let safe_kelly = (kelly * config.kelly_fraction).max(0.0);
    let position_value = bankroll * safe_kelly.min(config.max_position_pct);
    let shares = (position_value / price).floor() as u64;

    shares
        .max(config.min_position_size)
        .min(config.max_position_size)
}

/// Convert a scored candidate to a trading signal
pub fn candidate_to_signal(
    candidate: &MarketCandidate,
    context: &TradingContext,
    sizing_config: &PositionSizingConfig,
    fee_config: &FeeConfig,
    max_position_size: u64,
) -> Option<Signal> {
    let current_position = context.portfolio.get_position(&candidate.ticker);
    let current_qty = current_position.map(|p| p.quantity).unwrap_or(0);

    if current_qty >= max_position_size {
        return None;
    }

    let yes_price = candidate.current_yes_price.to_f64().unwrap_or(0.5);

    let side = if candidate.final_score > 0.0 {
        if yes_price < 0.5 {
            Side::Yes
        } else {
            Side::No
        }
    } else if candidate.final_score < 0.0 {
        if yes_price > 0.5 {
            Side::No
        } else {
            Side::Yes
        }
    } else {
        return None;
    };

    let price = match side {
        Side::Yes => candidate.current_yes_price,
        Side::No => candidate.current_no_price,
    };

    let available_cash = context.portfolio.cash.to_f64().unwrap_or(0.0);
    let price_f64 = price.to_f64().unwrap_or(0.5);

    if price_f64 <= 0.0 {
        return None;
    }

    let kelly_qty = kelly_size(
        candidate.final_score,
        price_f64,
        available_cash,
        sizing_config,
    );

    let max_affordable = (available_cash / price_f64) as u64;
    let quantity = kelly_qty
        .min(max_affordable)
        .min(max_position_size - current_qty);

    if quantity < sizing_config.min_position_size {
        return None;
    }

    // fee filtering: skip trades where fees exceed expected edge
    let entry_fee_drag = fee_config.fee_drag_pct(quantity, price_f64);
    let exit_fee_drag = fee_config.fee_drag_pct(quantity, 0.5); // conservative exit estimate
    let total_fee_drag = entry_fee_drag + exit_fee_drag;

    if candidate.final_score.abs() < total_fee_drag + fee_config.min_edge_after_fees {
        trace!(
            ticker = %candidate.ticker,
            score = candidate.final_score,
            fee_drag = total_fee_drag,
            min_required = fee_config.min_edge_after_fees,
            "skipping: insufficient edge after fees"
        );
        return None;
    }

    Some(Signal {
        ticker: candidate.ticker.clone(),
        side,
        quantity,
        limit_price: Some(price),
        reason: format!(
            "score={:.3}, side={:?}, price={:.2}, fee_drag={:.3}",
            candidate.final_score, side, price_f64, total_fee_drag
        ),
    })
}

/// Compute exit signals for current positions
pub fn compute_exit_signals(
    context: &TradingContext,
    candidate_scores: &HashMap<String, f64>,
    exit_config: &ExitConfig,
    price_lookup: &dyn Fn(&str) -> Option<Decimal>,
) -> Vec<ExitSignal> {
    let mut exits = Vec::new();

    for (ticker, position) in &context.portfolio.positions {
        // check time stop FIRST - doesn't need current price
        let hours_held = (context.timestamp - position.entry_time).num_hours();

        trace!(
            ticker = %ticker,
            hours_held = hours_held,
            max_hold = exit_config.max_hold_hours,
            "checking position"
        );

        if hours_held >= exit_config.max_hold_hours {
            // use entry price as fallback if no current price available
            let exit_price = price_lookup(ticker).unwrap_or(position.avg_entry_price);
            exits.push(ExitSignal {
                ticker: ticker.clone(),
                reason: ExitReason::TimeStop { hours_held },
                current_price: exit_price,
            });
            continue;
        }

        // for other exit types, we need a current price
        let current_price = match price_lookup(ticker) {
            Some(p) => p,
            None => {
                trace!(ticker = %ticker, "no price available, skipping other exit checks");
                continue;
            }
        };

        let effective_price = match position.side {
            Side::Yes => current_price,
            Side::No => Decimal::ONE - current_price,
        };

        let entry_price_f64 = position.avg_entry_price.to_f64().unwrap_or(0.5);
        let current_price_f64 = effective_price.to_f64().unwrap_or(0.5);

        if entry_price_f64 <= 0.0 {
            continue;
        }

        let pnl_pct = (current_price_f64 - entry_price_f64) / entry_price_f64;

        trace!(
            ticker = %ticker,
            entry = entry_price_f64,
            current = current_price_f64,
            pnl_pct = pnl_pct,
            tp_threshold = exit_config.take_profit_pct,
            sl_threshold = exit_config.stop_loss_pct,
            "evaluating pnl"
        );

        if pnl_pct >= exit_config.take_profit_pct {
            exits.push(ExitSignal {
                ticker: ticker.clone(),
                reason: ExitReason::TakeProfit { pnl_pct },
                current_price,
            });
            continue;
        }

        if pnl_pct <= -exit_config.stop_loss_pct {
            exits.push(ExitSignal {
                ticker: ticker.clone(),
                reason: ExitReason::StopLoss { pnl_pct },
                current_price,
            });
            continue;
        }

        if let Some(&new_score) = candidate_scores.get(ticker) {
            if new_score < exit_config.score_reversal_threshold {
                exits.push(ExitSignal {
                    ticker: ticker.clone(),
                    reason: ExitReason::ScoreReversal { new_score },
                    current_price,
                });
            }
        }
    }

    exits
}

/// Simple signal generator for basic strategies
pub fn simple_signal_generator(
    candidates: &[MarketCandidate],
    context: &TradingContext,
    position_size: u64,
) -> Vec<Signal> {
    candidates
        .iter()
        .filter(|c| c.final_score > 0.0)
        .filter(|c| !context.portfolio.has_position(&c.ticker))
        .map(|c| {
            let yes_price = c.current_yes_price.to_f64().unwrap_or(0.5);
            let (side, price) = if yes_price < 0.5 {
                (Side::Yes, c.current_yes_price)
            } else {
                (Side::No, c.current_no_price)
            };

            Signal {
                ticker: c.ticker.clone(),
                side,
                quantity: position_size,
                limit_price: Some(price),
                reason: format!("simple: score={:.3}", c.final_score),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_to_probability() {
        assert!((edge_to_win_probability(0.0) - 0.5).abs() < 0.001);
        assert!(edge_to_win_probability(1.0) > 0.7);
        assert!(edge_to_win_probability(-1.0) < 0.3);
    }

    #[test]
    fn test_kelly_size_zero_edge() {
        let config = PositionSizingConfig::default();
        assert_eq!(kelly_size(0.0, 0.5, 10000.0, &config), 0);
    }

    #[test]
    fn test_kelly_size_positive_edge() {
        let config = PositionSizingConfig::default();
        let size = kelly_size(0.5, 0.3, 10000.0, &config);
        assert!(size >= config.min_position_size);
        assert!(size <= config.max_position_size);
    }

    #[test]
    fn test_position_sizing_presets() {
        let conservative = PositionSizingConfig::conservative();
        let aggressive = PositionSizingConfig::aggressive();
        assert!(conservative.kelly_fraction < aggressive.kelly_fraction);
        assert!(conservative.max_position_pct < aggressive.max_position_pct);
    }
}
