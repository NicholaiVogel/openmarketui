//! Advanced scorers - battle-tested strategies from the Kalshi garden
//!
//! These scorers use more sophisticated signal processing techniques:
//! - Kalman filtering for price estimation
//! - Entropy analysis for uncertainty measurement
//! - Regime detection for market state classification
//! - Granger causality for correlation analysis
//! - Momentum acceleration (second-order momentum)
//! - Volatility estimation with multiple measures

mod entropy;
mod granger;
mod kalman;
mod momentum_accel;
mod regime;
mod volatility;

pub use entropy::EntropyScorer;
pub use granger::GrangerCorrelationScorer;
pub use kalman::KalmanPriceFilter;
pub use momentum_accel::MomentumAccelerationScorer;
pub use regime::RegimeDetector;
pub use volatility::VolatilityScorer;
