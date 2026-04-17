//! Paper trading engine

mod state;
mod trading;

pub use state::EngineState;
pub use trading::{
    DecisionInfo, EngineStatus, ManualRedeemResult, PaperTradingEngine, TickMetrics,
};
