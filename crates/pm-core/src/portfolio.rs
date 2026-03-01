//! Portfolio and position management
//!
//! Tracks the current harvest - positions we hold and their yields.

use crate::{Fill, MarketResult, Side};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A position in a single market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub side: Side,
    pub quantity: u64,
    pub avg_entry_price: Decimal,
    pub entry_time: DateTime<Utc>,
    pub close_time: Option<DateTime<Utc>>,
}

impl Position {
    pub fn cost_basis(&self) -> Decimal {
        self.avg_entry_price * Decimal::from(self.quantity)
    }
}

/// The full portfolio state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub positions: HashMap<String, Position>,
    pub cash: Decimal,
    pub initial_capital: Decimal,
    #[serde(default)]
    pub realized_pnl: Decimal,
}

impl Portfolio {
    pub fn new(initial_capital: Decimal) -> Self {
        Self {
            positions: HashMap::new(),
            cash: initial_capital,
            initial_capital,
            realized_pnl: Decimal::ZERO,
        }
    }

    pub fn total_value(&self, market_prices: &HashMap<String, Decimal>) -> Decimal {
        let position_value: Decimal = self
            .positions
            .values()
            .map(|p| {
                let price = market_prices
                    .get(&p.ticker)
                    .copied()
                    .unwrap_or(p.avg_entry_price);
                let effective_price = match p.side {
                    Side::Yes => price,
                    Side::No => Decimal::ONE - price,
                };
                effective_price * Decimal::from(p.quantity)
            })
            .sum();
        self.cash + position_value
    }

    pub fn has_position(&self, ticker: &str) -> bool {
        self.positions.contains_key(ticker)
    }

    pub fn get_position(&self, ticker: &str) -> Option<&Position> {
        self.positions.get(ticker)
    }

    pub fn apply_fill(&mut self, fill: &Fill) {
        self.apply_fill_with_metadata(fill, None, None, None);
    }

    pub fn apply_fill_with_metadata(
        &mut self,
        fill: &Fill,
        title: Option<&str>,
        category: Option<&str>,
        close_time: Option<DateTime<Utc>>,
    ) {
        let cost = fill.price * Decimal::from(fill.quantity);

        match fill.side {
            Side::Yes | Side::No => {
                self.cash -= cost;
                let position = self
                    .positions
                    .entry(fill.ticker.clone())
                    .or_insert_with(|| Position {
                        ticker: fill.ticker.clone(),
                        title: title.unwrap_or(&fill.ticker).to_string(),
                        category: category.unwrap_or("").to_string(),
                        side: fill.side,
                        quantity: 0,
                        avg_entry_price: Decimal::ZERO,
                        entry_time: fill.timestamp,
                        close_time,
                    });

                let total_cost = position.avg_entry_price * Decimal::from(position.quantity) + cost;
                position.quantity += fill.quantity;
                if position.quantity > 0 {
                    position.avg_entry_price = total_cost / Decimal::from(position.quantity);
                }
            }
        }
    }

    /// Resolve a position when market closes
    /// Returns the P&L from the resolution
    pub fn resolve_position(&mut self, ticker: &str, result: MarketResult) -> Option<Decimal> {
        let position = self.positions.remove(ticker)?;

        let payout = match (result, position.side) {
            (MarketResult::Yes, Side::Yes) | (MarketResult::No, Side::No) => {
                Decimal::from(position.quantity)
            }
            (MarketResult::Cancelled, _) => {
                position.avg_entry_price * Decimal::from(position.quantity)
            }
            _ => Decimal::ZERO,
        };

        self.cash += payout;

        let cost = position.avg_entry_price * Decimal::from(position.quantity);
        let pnl = payout - cost;
        self.realized_pnl += pnl;
        Some(pnl)
    }

    /// Close a position at current market price
    /// Returns the P&L from the exit
    pub fn close_position(&mut self, ticker: &str, exit_price: Decimal) -> Option<Decimal> {
        let position = self.positions.remove(ticker)?;

        let effective_exit_price = match position.side {
            Side::Yes => exit_price,
            Side::No => Decimal::ONE - exit_price,
        };

        let exit_value = effective_exit_price * Decimal::from(position.quantity);
        self.cash += exit_value;

        let cost = position.avg_entry_price * Decimal::from(position.quantity);
        let pnl = exit_value - cost;
        self.realized_pnl += pnl;
        Some(pnl)
    }
}

/// A recorded trade in trading history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub ticker: String,
    pub side: Side,
    pub quantity: u64,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub trade_type: TradeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    Open,
    Close,
    Resolution,
}
