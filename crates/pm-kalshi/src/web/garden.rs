//! Garden abstraction endpoints

use super::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct GardenStatus {
    pub total_specimens: usize,
    pub blooming: usize,
    pub dormant: usize,
    pub pruned: usize,
    pub total_beds: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BedResponse {
    pub name: String,
    pub description: String,
    pub specimen_count: usize,
    pub active_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecimenResponse {
    pub name: String,
    pub bed: String,
    pub status: String,
    pub weight: f64,
    pub hit_rate: Option<f64>,
    pub avg_contribution: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpecimenStatusUpdate {
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScorerToggleRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeightsUpdateRequest {
    pub weights: std::collections::HashMap<String, f64>,
}

pub async fn get_garden_status(State(state): State<Arc<AppState>>) -> Json<GardenStatus> {
    let specimens = state.specimens.read().await;

    let mut blooming = 0;
    let mut dormant = 0;
    let mut pruned = 0;

    for info in specimens.values() {
        match info.status.as_str() {
            "blooming" => blooming += 1,
            "dormant" => dormant += 1,
            "pruned" => pruned += 1,
            _ => {}
        }
    }

    let beds: std::collections::HashSet<_> = specimens.values().map(|s| &s.bed).collect();

    Json(GardenStatus {
        total_specimens: specimens.len(),
        blooming,
        dormant,
        pruned,
        total_beds: beds.len(),
    })
}

pub async fn get_beds(State(state): State<Arc<AppState>>) -> Json<Vec<BedResponse>> {
    let specimens = state.specimens.read().await;

    let mut beds: std::collections::HashMap<String, (usize, usize)> =
        std::collections::HashMap::new();

    for info in specimens.values() {
        let entry = beds.entry(info.bed.clone()).or_insert((0, 0));
        entry.0 += 1;
        if info.status == "blooming" {
            entry.1 += 1;
        }
    }

    let bed_descriptions = [
        ("momentum", "trend following strategies"),
        (
            "mean_reversion",
            "mean reversion and bollinger band strategies",
        ),
        ("volume", "volume and order flow analysis"),
        ("ensemble", "combination and weighted strategies"),
        ("experimental", "experimental and unproven strategies"),
    ];

    let desc_map: std::collections::HashMap<_, _> = bed_descriptions.into_iter().collect();

    let response: Vec<BedResponse> = beds
        .into_iter()
        .map(|(name, (total, active))| BedResponse {
            description: desc_map
                .get(name.as_str())
                .unwrap_or(&"custom bed")
                .to_string(),
            name,
            specimen_count: total,
            active_count: active,
        })
        .collect();

    Json(response)
}

pub async fn get_bed_specimens(
    State(state): State<Arc<AppState>>,
    Path(bed_name): Path<String>,
) -> Result<Json<Vec<SpecimenResponse>>, StatusCode> {
    let specimens = state.specimens.read().await;

    let bed_specimens: Vec<SpecimenResponse> = specimens
        .iter()
        .filter(|(_, info)| info.bed == bed_name)
        .map(|(name, info)| SpecimenResponse {
            name: name.clone(),
            bed: info.bed.clone(),
            status: info.status.clone(),
            weight: info.weight,
            hit_rate: info.hit_rate,
            avg_contribution: info.avg_contribution,
        })
        .collect();

    if bed_specimens.is_empty() {
        let has_bed = specimens.values().any(|s| s.bed == bed_name);
        if !has_bed {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    Ok(Json(bed_specimens))
}

pub async fn post_specimen_status(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(update): Json<SpecimenStatusUpdate>,
) -> Result<StatusCode, StatusCode> {
    let valid_statuses = ["blooming", "dormant", "pruned"];
    if !valid_statuses.contains(&update.status.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut specimens = state.specimens.write().await;
    if let Some(info) = specimens.get_mut(&name) {
        info.status = update.status.clone();

        let _ = state
            .updates_tx
            .send(super::ws::ServerMessage::SpecimenChanged {
                name: name.clone(),
                status: info.status.clone(),
                weight: info.weight,
            });

        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn post_scorer_toggle(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<ScorerToggleRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut specimens = state.specimens.write().await;
    if let Some(info) = specimens.get_mut(&name) {
        info.status = if req.enabled {
            "blooming".to_string()
        } else {
            "dormant".to_string()
        };

        let _ = state
            .updates_tx
            .send(super::ws::ServerMessage::SpecimenChanged {
                name: name.clone(),
                status: info.status.clone(),
                weight: info.weight,
            });

        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn put_weights(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WeightsUpdateRequest>,
) -> StatusCode {
    let mut specimens = state.specimens.write().await;

    for (name, weight) in req.weights {
        if let Some(info) = specimens.get_mut(&name) {
            info.weight = weight.clamp(0.0, 2.0);

            let _ = state
                .updates_tx
                .send(super::ws::ServerMessage::SpecimenChanged {
                    name: name.clone(),
                    status: info.status.clone(),
                    weight: info.weight,
                });
        }
    }

    StatusCode::OK
}
