//! Application state for the watchtower
//!
//! Holds the store, registry, and broadcast channel for
//! real-time garden updates.

use pm_garden::{SpecimenInfo, SpecimenRegistry, SpecimenStatus};
use pm_store::SqliteStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::ws::GardenUpdate;

/// Shared application state
pub struct AppState {
    /// Database store (root cellar)
    pub store: Arc<SqliteStore>,
    /// Specimen registry (seed catalog)
    pub registry: Arc<SpecimenRegistry>,
    /// Active specimens with their status
    pub specimens: Arc<RwLock<HashMap<String, SpecimenInfo>>>,
    /// Broadcast channel for WebSocket updates
    pub updates_tx: broadcast::Sender<GardenUpdate>,
}

impl AppState {
    pub fn new(store: Arc<SqliteStore>, registry: Arc<SpecimenRegistry>) -> Self {
        let (updates_tx, _) = broadcast::channel(256);

        // populate specimens from registry
        let mut specimens = HashMap::new();
        for name in registry.list_scorers() {
            let bed = infer_bed(name);
            specimens.insert(name.to_string(), SpecimenInfo::new(name, &bed));
        }

        Self {
            store,
            registry,
            specimens: Arc::new(RwLock::new(specimens)),
            updates_tx,
        }
    }

    /// Set specimen status (blooming, dormant, pruned)
    pub async fn set_specimen_status(
        &self,
        name: &str,
        status: SpecimenStatus,
    ) -> Result<(), String> {
        let mut specimens = self.specimens.write().await;
        let specimen = specimens
            .get_mut(name)
            .ok_or_else(|| format!("specimen not found: {}", name))?;
        specimen.status = status;
        Ok(())
    }

    /// Broadcast a garden update to all connected clients
    pub fn broadcast(&self, update: GardenUpdate) {
        // ignore send errors (no receivers)
        let _ = self.updates_tx.send(update);
    }

    /// Subscribe to garden updates
    pub fn subscribe(&self) -> broadcast::Receiver<GardenUpdate> {
        self.updates_tx.subscribe()
    }
}

/// Infer bed from specimen name
fn infer_bed(name: &str) -> String {
    match name {
        "momentum" | "mtf_momentum" | "time_decay" => "momentum".to_string(),
        "mean_reversion" | "bollinger" => "mean_reversion".to_string(),
        "volume" | "order_flow" | "vpin" => "volume".to_string(),
        _ => "ensemble".to_string(),
    }
}
