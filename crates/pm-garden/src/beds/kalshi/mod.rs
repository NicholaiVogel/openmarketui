//! Kalshi-specific garden beds
//!
//! Specimens optimized for Kalshi prediction markets.

pub mod ensemble;
pub mod mean_reversion;
pub mod momentum;
pub mod osint;
pub mod volume;

// Re-export all specimens for convenience
pub use ensemble::*;
pub use mean_reversion::*;
pub use momentum::*;
pub use osint::*;
pub use volume::*;
