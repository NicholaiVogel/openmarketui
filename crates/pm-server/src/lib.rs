//! pm-server: The watchtower of the prediction market garden
//!
//! A REST + WebSocket server to observe and control the garden.
//! From here you can monitor specimens, view harvests, and adjust
//! the garden's health in real-time.

mod routes;
mod state;
mod ws;

pub use routes::build_router;
pub use state::AppState;
pub use ws::{GardenUpdate, HarvestUpdate, SpecimenUpdate, StatusUpdate, YieldUpdate};
