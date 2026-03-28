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
        let fee = fill.fee.unwrap_or(Decimal::ZERO);

        match fill.side {
            Side::Yes | Side::No => {
                self.cash -= cost + fee;
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
    ///
    /// `exit_price` is expected to be the contract price for the held side
    /// (YES price for YES positions, NO price for NO positions).
    pub fn close_position(&mut self, ticker: &str, exit_price: Decimal) -> Option<Decimal> {
        let quantity = self.positions.get(ticker)?.quantity;
        self.close_position_partial(ticker, quantity, exit_price, None)
    }

    /// Partially close a position at current market price.
    ///
    /// Returns realized P&L for the closed quantity.
    pub fn close_position_partial(
        &mut self,
        ticker: &str,
        quantity: u64,
        exit_price: Decimal,
        fee: Option<Decimal>,
    ) -> Option<Decimal> {
        if quantity == 0 {
            return None;
        }

        let (close_qty, avg_entry_price, remaining_qty_after) = {
            let position = self.positions.get_mut(ticker)?;
            let close_qty = quantity.min(position.quantity);
            position.quantity -= close_qty;
            (close_qty, position.avg_entry_price, position.quantity)
        };

        let fee = fee.unwrap_or(Decimal::ZERO);
        let close_qty_dec = Decimal::from(close_qty);
        let exit_value = exit_price * close_qty_dec;
        self.cash += exit_value - fee;

        let cost = avg_entry_price * close_qty_dec;
        let pnl = exit_value - fee - cost;
        self.realized_pnl += pnl;

        if remaining_qty_after == 0 {
            self.positions.remove(ticker);
        }

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
