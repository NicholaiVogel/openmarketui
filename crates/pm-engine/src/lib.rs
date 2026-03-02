//! pm-engine: Shared engine components for prediction market trading
//!
//! This crate provides reusable components for building trading engines:
//! - Circuit breaker for risk management (frost protection)
//! - Execution logic for position sizing and Kelly criterion
//! - Fee calculation for trade filtering
//! - Paper executor for simulated trading

mod circuit_breaker;
mod execution;
mod fees;

pub use circuit_breaker::{CbCheckContext, CbStatus, CircuitBreakerConfig, CircuitBreakerState};
pub use execution::{
    candidate_to_signal, compute_exit_signals, edge_to_win_probability, kelly_size,
    simple_signal_generator, PositionSizingConfig,
};
pub use fees::FeeConfig;
