//! Backtest metrics collection and reporting

use chrono::{DateTime, Utc};
use pm_core::{Portfolio, Trade, TradeType};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub total_return: f64,
    pub total_return_pct: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub max_drawdown_pct: f64,
    pub win_rate: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub avg_trade_pnl: f64,
    pub avg_hold_time_hours: f64,
    pub trades_per_day: f64,
    pub return_by_category: HashMap<String, f64>,
    pub equity_curve: Vec<EquityPoint>,
    pub trade_log: Vec<TradeRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub timestamp: DateTime<Utc>,
    pub equity: f64,
    pub cash: f64,
    pub positions_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub ticker: String,
    pub entry_time: DateTime<Utc>,
    pub exit_time: Option<DateTime<Utc>>,
    pub side: String,
    pub quantity: u64,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub pnl: Option<f64>,
    pub category: String,
}

pub struct MetricsCollector {
    initial_capital: Decimal,
    equity_curve: Vec<EquityPoint>,
    trade_records: HashMap<String, TradeRecord>,
    closed_trades: Vec<TradeRecord>,
    daily_returns: Vec<f64>,
    last_equity: f64,
    peak_equity: f64,
    max_drawdown: f64,
}

impl MetricsCollector {
    pub fn new(initial_capital: Decimal) -> Self {
        let capital = initial_capital.to_f64().unwrap_or(10000.0);
        Self {
            initial_capital,
            equity_curve: Vec::new(),
            trade_records: HashMap::new(),
            closed_trades: Vec::new(),
            daily_returns: Vec::new(),
            last_equity: capital,
            peak_equity: capital,
            max_drawdown: 0.0,
        }
    }

    pub fn record(
        &mut self,
        timestamp: DateTime<Utc>,
        portfolio: &Portfolio,
        market_prices: &HashMap<String, Decimal>,
    ) {
        let positions_value = portfolio
            .positions
            .values()
            .map(|p| {
                let price = market_prices
                    .get(&p.ticker)
                    .copied()
                    .unwrap_or(p.avg_entry_price);
                (price * Decimal::from(p.quantity)).to_f64().unwrap_or(0.0)
            })
            .sum();

        let cash = portfolio.cash.to_f64().unwrap_or(0.0);
        let equity = cash + positions_value;

        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        let drawdown = (self.peak_equity - equity) / self.peak_equity;
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
        }

        if self.last_equity > 0.0 {
            let daily_return = (equity - self.last_equity) / self.last_equity;
            self.daily_returns.push(daily_return);
        }
        self.last_equity = equity;

        self.equity_curve.push(EquityPoint {
            timestamp,
            equity,
            cash,
            positions_value,
        });
    }

    pub fn record_trade(&mut self, trade: &Trade, category: &str) {
        match trade.trade_type {
            TradeType::Open => {
                let record = TradeRecord {
                    ticker: trade.ticker.clone(),
                    entry_time: trade.timestamp,
                    exit_time: None,
                    side: format!("{:?}", trade.side),
                    quantity: trade.quantity,
                    entry_price: trade.price.to_f64().unwrap_or(0.0),
                    exit_price: None,
                    pnl: None,
                    category: category.to_string(),
                };
                self.trade_records.insert(trade.ticker.clone(), record);
            }
            TradeType::Close | TradeType::Resolution => {
                if let Some(mut record) = self.trade_records.remove(&trade.ticker) {
                    let exit_price = trade.price.to_f64().unwrap_or(0.0);
                    let entry_cost = record.entry_price * record.quantity as f64;
                    let exit_value = exit_price * record.quantity as f64;
                    let pnl = exit_value - entry_cost;

                    record.exit_time = Some(trade.timestamp);
                    record.exit_price = Some(exit_price);
                    record.pnl = Some(pnl);

                    self.closed_trades.push(record);
                }
            }
        }
    }

    pub fn finalize(self) -> BacktestResult {
        let initial = self.initial_capital.to_f64().unwrap_or(10000.0);
        let final_equity = self
            .equity_curve
            .last()
            .map(|e| e.equity)
            .unwrap_or(initial);
        let total_return = final_equity - initial;
        let total_return_pct = total_return / initial * 100.0;

        let sharpe_ratio = if self.daily_returns.len() > 1 {
            let mean: f64 =
                self.daily_returns.iter().sum::<f64>() / self.daily_returns.len() as f64;
            let variance: f64 = self
                .daily_returns
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / (self.daily_returns.len() - 1) as f64;
            let std_dev = variance.sqrt();
            if std_dev > 0.0 {
                (mean / std_dev) * (252.0_f64).sqrt()
            } else {
                0.0
            }
        } else {
            0.0
        };

        let winning_trades = self
            .closed_trades
            .iter()
            .filter(|t| t.pnl.unwrap_or(0.0) > 0.0)
            .count();
        let losing_trades = self
            .closed_trades
            .iter()
            .filter(|t| t.pnl.unwrap_or(0.0) < 0.0)
            .count();
        let closed_trades_count = self.closed_trades.len();
        let open_trades_count = self.trade_records.len();
        let total_trades = closed_trades_count + open_trades_count;

        let win_rate = if closed_trades_count > 0 {
            winning_trades as f64 / closed_trades_count as f64 * 100.0
        } else {
            0.0
        };

        let avg_trade_pnl = if closed_trades_count > 0 {
            self.closed_trades.iter().filter_map(|t| t.pnl).sum::<f64>() / closed_trades_count as f64
        } else {
            0.0
        };

        let avg_hold_time_hours = if closed_trades_count > 0 {
            self.closed_trades
                .iter()
                .filter_map(|t| {
                    t.exit_time
                        .map(|exit| (exit - t.entry_time).num_hours() as f64)
                })
                .sum::<f64>()
                / closed_trades_count as f64
        } else {
            0.0
        };

        let duration_days = if self.equity_curve.len() >= 2 {
            let start = self.equity_curve.first().unwrap().timestamp;
            let end = self.equity_curve.last().unwrap().timestamp;
            (end - start).num_days().max(1) as f64
        } else {
            1.0
        };

        let trades_per_day = total_trades as f64 / duration_days;

        let mut return_by_category: HashMap<String, f64> = HashMap::new();
        for trade in &self.closed_trades {
            *return_by_category
                .entry(trade.category.clone())
                .or_insert(0.0) += trade.pnl.unwrap_or(0.0);
        }

        // Include both closed and still-open entries in the trade log so UI can
        // display deployed capital even if positions have not closed yet.
        let mut trade_log = self.closed_trades;
        trade_log.extend(self.trade_records.into_values());
        trade_log.sort_by_key(|t| t.entry_time);

        BacktestResult {
            total_return,
            total_return_pct,
            sharpe_ratio,
            max_drawdown: self.max_drawdown * 100.0,
            max_drawdown_pct: self.max_drawdown * 100.0,
            win_rate,
            total_trades,
            winning_trades,
            losing_trades,
            avg_trade_pnl,
            avg_hold_time_hours,
            trades_per_day,
            return_by_category,
            equity_curve: self.equity_curve,
            trade_log,
        }
    }
}

impl BacktestResult {
    pub fn summary(&self) -> String {
        format!(
            r#"
backtest results
================

performance
-----------
total return:     ${:.2} ({:.2}%)
sharpe ratio:     {:.3}
max drawdown:     {:.2}%

trades
------
total trades:     {}
win rate:         {:.1}%
avg trade pnl:    ${:.2}
avg hold time:    {:.1} hours
trades per day:   {:.2}

by category
-----------
{}
"#,
            self.total_return,
            self.total_return_pct,
            self.sharpe_ratio,
            self.max_drawdown_pct,
            self.total_trades,
            self.win_rate,
            self.avg_trade_pnl,
            self.avg_hold_time_hours,
            self.trades_per_day,
            self.return_by_category
                .iter()
                .map(|(k, v)| format!("  {}: ${:.2}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
