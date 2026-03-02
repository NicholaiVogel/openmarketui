//! Circuit breaker for risk management (frost protection)
//!
//! Monitors trading activity and trips when limits are exceeded:
//! - Max drawdown
//! - Daily loss limits
//! - Position limits
//! - Consecutive errors
//! - Fill rate limits

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tracing::warn;

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub max_drawdown_pct: f64,
    pub max_daily_loss_pct: f64,
    pub max_positions: Option<usize>,
    pub max_single_position_pct: Option<f64>,
    pub max_consecutive_errors: Option<u32>,
    pub max_fills_per_hour: Option<u32>,
    pub max_fills_per_day: Option<u32>,
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

/// Circuit breaker status
#[derive(Debug, Clone, PartialEq)]
pub enum CbStatus {
    Ok,
    Tripped(String),
}

/// Data needed to evaluate circuit breaker rules
pub struct CbCheckContext {
    pub current_equity: Decimal,
    pub peak_equity: Decimal,
    pub positions_count: usize,
    pub daily_pnl: f64,
    pub hourly_fills: u32,
    pub daily_fills: u32,
}

/// Mutable state tracked by the circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerState {
    config: CircuitBreakerConfig,
    consecutive_errors: u32,
}

impl CircuitBreakerState {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            consecutive_errors: 0,
        }
    }

    pub fn record_error(&mut self) {
        self.consecutive_errors += 1;
    }

    pub fn record_success(&mut self) {
        self.consecutive_errors = 0;
    }

    /// Check all circuit breaker rules
    pub fn check(&self, ctx: &CbCheckContext) -> CbStatus {
        if let Some(status) = self.check_drawdown(ctx.current_equity, ctx.peak_equity) {
            return status;
        }

        if let Some(status) = self.check_daily_loss(ctx.daily_pnl, ctx.peak_equity) {
            return status;
        }

        if let Some(status) = self.check_max_positions(ctx.positions_count) {
            return status;
        }

        if let Some(status) = self.check_consecutive_errors() {
            return status;
        }

        if let Some(status) = self.check_fill_rate(ctx.hourly_fills, ctx.daily_fills) {
            return status;
        }

        CbStatus::Ok
    }

    fn check_drawdown(&self, current_equity: Decimal, peak_equity: Decimal) -> Option<CbStatus> {
        if peak_equity <= Decimal::ZERO {
            return None;
        }

        let drawdown = (peak_equity - current_equity).to_f64().unwrap_or(0.0)
            / peak_equity.to_f64().unwrap_or(1.0);

        if drawdown >= self.config.max_drawdown_pct {
            let msg = format!(
                "drawdown {:.1}% exceeds max {:.1}%",
                drawdown * 100.0,
                self.config.max_drawdown_pct * 100.0
            );
            warn!(rule = "max_drawdown", %msg);
            return Some(CbStatus::Tripped(msg));
        }
        None
    }

    fn check_daily_loss(&self, daily_pnl: f64, peak_equity: Decimal) -> Option<CbStatus> {
        let peak_f64 = peak_equity.to_f64().unwrap_or(10000.0);
        if peak_f64 <= 0.0 {
            return None;
        }

        let daily_loss_pct = (-daily_pnl) / peak_f64;
        if daily_loss_pct >= self.config.max_daily_loss_pct {
            let msg = format!(
                "daily loss {:.1}% exceeds max {:.1}%",
                daily_loss_pct * 100.0,
                self.config.max_daily_loss_pct * 100.0
            );
            warn!(rule = "max_daily_loss", %msg);
            return Some(CbStatus::Tripped(msg));
        }
        None
    }

    fn check_max_positions(&self, count: usize) -> Option<CbStatus> {
        let max = self.config.max_positions.unwrap_or(100);
        if count >= max {
            let msg = format!("positions {} at max {}", count, max);
            warn!(rule = "max_positions", %msg);
            return Some(CbStatus::Tripped(msg));
        }
        None
    }

    fn check_consecutive_errors(&self) -> Option<CbStatus> {
        let max = self.config.max_consecutive_errors.unwrap_or(5);
        if self.consecutive_errors >= max {
            let msg = format!(
                "{} consecutive errors (max {})",
                self.consecutive_errors, max
            );
            warn!(rule = "consecutive_errors", %msg);
            return Some(CbStatus::Tripped(msg));
        }
        None
    }

    fn check_fill_rate(&self, hourly_fills: u32, daily_fills: u32) -> Option<CbStatus> {
        let max_hourly = self.config.max_fills_per_hour.unwrap_or(50);
        let max_daily = self.config.max_fills_per_day.unwrap_or(200);

        if hourly_fills >= max_hourly {
            let msg = format!("{} fills/hour exceeds max {}", hourly_fills, max_hourly);
            warn!(rule = "fill_rate_hourly", %msg);
            return Some(CbStatus::Tripped(msg));
        }

        if daily_fills >= max_daily {
            let msg = format!("{} fills/day exceeds max {}", daily_fills, max_daily);
            warn!(rule = "fill_rate_daily", %msg);
            return Some(CbStatus::Tripped(msg));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.max_drawdown_pct, 0.15);
        assert_eq!(config.max_daily_loss_pct, 0.05);
    }

    #[test]
    fn test_cb_ok_when_within_limits() {
        let state = CircuitBreakerState::new(CircuitBreakerConfig::default());
        let ctx = CbCheckContext {
            current_equity: Decimal::new(9500, 0),
            peak_equity: Decimal::new(10000, 0),
            positions_count: 5,
            daily_pnl: -100.0,
            hourly_fills: 10,
            daily_fills: 50,
        };
        assert_eq!(state.check(&ctx), CbStatus::Ok);
    }

    #[test]
    fn test_cb_trips_on_drawdown() {
        let state = CircuitBreakerState::new(CircuitBreakerConfig::default());
        let ctx = CbCheckContext {
            current_equity: Decimal::new(8000, 0),
            peak_equity: Decimal::new(10000, 0),
            positions_count: 5,
            daily_pnl: 0.0,
            hourly_fills: 10,
            daily_fills: 50,
        };
        match state.check(&ctx) {
            CbStatus::Tripped(msg) => assert!(msg.contains("drawdown")),
            CbStatus::Ok => panic!("expected trip"),
        }
    }

    #[test]
    fn test_consecutive_errors() {
        let mut state = CircuitBreakerState::new(CircuitBreakerConfig::default());
        for _ in 0..5 {
            state.record_error();
        }
        let ctx = CbCheckContext {
            current_equity: Decimal::new(10000, 0),
            peak_equity: Decimal::new(10000, 0),
            positions_count: 0,
            daily_pnl: 0.0,
            hourly_fills: 0,
            daily_fills: 0,
        };
        match state.check(&ctx) {
            CbStatus::Tripped(msg) => assert!(msg.contains("consecutive errors")),
            CbStatus::Ok => panic!("expected trip"),
        }

        state.record_success();
        assert_eq!(state.check(&ctx), CbStatus::Ok);
    }
}
