//! REST API routes for the watchtower
//!
//! Endpoints to observe and control the garden.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use pm_garden::SpecimenStatus;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;
use crate::ws::ws_handler;

/// Build the router with all garden endpoints
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/garden/status", get(get_garden_status))
        .route("/api/beds", get(get_beds))
        .route("/api/beds/:bed/specimens", get(get_bed_specimens))
        .route("/api/positions", get(get_positions))
        .route("/api/history", get(get_history))
        .route("/api/decisions", get(get_decisions))
        .route("/api/decisions/:id", get(get_decision_by_id))
        .route("/api/specimens/:name/status", post(set_specimen_status))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(state)
}

// === Response types ===

#[derive(Serialize)]
pub struct GardenStatusResponse {
    pub status: String,
    pub beds_count: usize,
    pub specimens_count: usize,
    pub blooming_count: usize,
    pub dormant_count: usize,
    pub positions_count: usize,
    pub total_yield: f64,
}

#[derive(Serialize)]
pub struct BedResponse {
    pub name: String,
    pub specimens_count: usize,
    pub blooming_count: usize,
}

#[derive(Serialize)]
pub struct SpecimenResponse {
    pub name: String,
    pub bed: String,
    pub status: String,
    pub weight: f64,
}

#[derive(Serialize)]
pub struct PositionResponse {
    pub ticker: String,
    pub side: String,
    pub quantity: u64,
    pub entry_price: f64,
    pub entry_time: String,
    pub unrealized_pnl: f64,
}

#[derive(Serialize)]
pub struct FillResponse {
    pub ticker: String,
    pub side: String,
    pub quantity: u64,
    pub price: f64,
    pub timestamp: String,
    pub pnl: Option<f64>,
    pub exit_reason: Option<String>,
}

#[derive(Serialize)]
pub struct DecisionResponse {
    pub id: i64,
    pub timestamp: String,
    pub ticker: String,
    pub action: String,
    pub side: Option<String>,
    pub score: f64,
    pub confidence: f64,
    pub scorer_breakdown: std::collections::HashMap<String, f64>,
    pub reason: Option<String>,
    pub signal_id: Option<i64>,
    pub fill_id: Option<i64>,
    pub latency_ms: Option<i64>,
}

#[derive(Deserialize)]
pub struct SetStatusRequest {
    pub status: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// === Handlers ===

/// GET /api/garden/status - overall garden health
async fn get_garden_status(State(state): State<Arc<AppState>>) -> Json<GardenStatusResponse> {
    let specimens = state.specimens.read().await;

    let beds: std::collections::HashSet<_> = specimens.values().map(|s| &s.bed).collect();

    let blooming_count = specimens
        .values()
        .filter(|s| s.status == SpecimenStatus::Blooming)
        .count();

    let dormant_count = specimens
        .values()
        .filter(|s| s.status == SpecimenStatus::Dormant)
        .count();

    let portfolio = state.store.load_portfolio().await.ok().flatten();
    let positions_count = portfolio.as_ref().map(|p| p.positions.len()).unwrap_or(0);

    let equity_curve = state.store.get_equity_curve().await.ok();
    let total_yield = equity_curve
        .and_then(|curve| {
            let first = curve.first()?.equity.to_f64()?;
            let last = curve.last()?.equity.to_f64()?;
            Some(last - first)
        })
        .unwrap_or(0.0);

    let status = if blooming_count > 0 {
        "healthy"
    } else {
        "dormant"
    };

    Json(GardenStatusResponse {
        status: status.to_string(),
        beds_count: beds.len(),
        specimens_count: specimens.len(),
        blooming_count,
        dormant_count,
        positions_count,
        total_yield,
    })
}

/// GET /api/beds - list all beds and their specimens
async fn get_beds(State(state): State<Arc<AppState>>) -> Json<Vec<BedResponse>> {
    let specimens = state.specimens.read().await;

    let mut beds: std::collections::HashMap<String, (usize, usize)> =
        std::collections::HashMap::new();

    for specimen in specimens.values() {
        let entry = beds.entry(specimen.bed.clone()).or_insert((0, 0));
        entry.0 += 1;
        if specimen.status == SpecimenStatus::Blooming {
            entry.1 += 1;
        }
    }

    let response: Vec<BedResponse> = beds
        .into_iter()
        .map(|(name, (total, blooming))| BedResponse {
            name,
            specimens_count: total,
            blooming_count: blooming,
        })
        .collect();

    Json(response)
}

/// GET /api/beds/:bed/specimens - specimens in a bed
async fn get_bed_specimens(
    State(state): State<Arc<AppState>>,
    Path(bed): Path<String>,
) -> Json<Vec<SpecimenResponse>> {
    let specimens = state.specimens.read().await;

    let response: Vec<SpecimenResponse> = specimens
        .values()
        .filter(|s| s.bed == bed)
        .map(|s| SpecimenResponse {
            name: s.name.clone(),
            bed: s.bed.clone(),
            status: s.status.to_string(),
            weight: s.weight,
        })
        .collect();

    Json(response)
}

/// GET /api/positions - current harvest (positions)
async fn get_positions(State(state): State<Arc<AppState>>) -> Json<Vec<PositionResponse>> {
    let portfolio = match state.store.load_portfolio().await {
        Ok(Some(p)) => p,
        _ => return Json(vec![]),
    };

    let positions: Vec<PositionResponse> = portfolio
        .positions
        .values()
        .map(|p| PositionResponse {
            ticker: p.ticker.clone(),
            side: format!("{:?}", p.side),
            quantity: p.quantity,
            entry_price: p.avg_entry_price.to_f64().unwrap_or(0.0),
            entry_time: p.entry_time.to_rfc3339(),
            unrealized_pnl: 0.0, // would need current prices to calculate
        })
        .collect();

    Json(positions)
}

/// GET /api/history - harvest history (fills)
async fn get_history(State(state): State<Arc<AppState>>) -> Json<Vec<FillResponse>> {
    let fills = match state.store.get_recent_fills(100).await {
        Ok(f) => f,
        Err(_) => return Json(vec![]),
    };

    let response: Vec<FillResponse> = fills
        .into_iter()
        .map(|f| FillResponse {
            ticker: f.ticker,
            side: format!("{:?}", f.side),
            quantity: f.quantity,
            price: f.price.to_f64().unwrap_or(0.0),
            timestamp: f.timestamp.to_rfc3339(),
            pnl: f.pnl.and_then(|p| p.to_f64()),
            exit_reason: f.exit_reason,
        })
        .collect();

    Json(response)
}

/// POST /api/specimens/:name/status - set specimen status
async fn set_specimen_status(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(req): Json<SetStatusRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let status = match req.status.as_str() {
        "blooming" => SpecimenStatus::Blooming,
        "dormant" => SpecimenStatus::Dormant,
        "pruned" => SpecimenStatus::Pruned,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "invalid status: {}. use blooming, dormant, or pruned",
                        req.status
                    ),
                }),
            ));
        }
    };

    state
        .set_specimen_status(&name, status)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, Json(ErrorResponse { error: e })))?;

    // broadcast the status change
    state.broadcast(crate::ws::GardenUpdate::Status(crate::ws::StatusUpdate {
        status: req.status,
        message: format!("specimen {} status updated", name),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }));

    Ok(StatusCode::OK)
}

/// GET /api/decisions - recent decisions
async fn get_decisions(State(state): State<Arc<AppState>>) -> Json<Vec<DecisionResponse>> {
    let decisions = match state.store.get_recent_decisions(100).await {
        Ok(d) => d,
        Err(_) => return Json(vec![]),
    };

    let response: Vec<DecisionResponse> = decisions
        .into_iter()
        .map(|d| DecisionResponse {
            id: d.id,
            timestamp: d.timestamp.to_rfc3339(),
            ticker: d.ticker,
            action: d.action.to_string(),
            side: d.side.map(|s| format!("{:?}", s)),
            score: d.score,
            confidence: d.confidence,
            scorer_breakdown: d.scorer_breakdown,
            reason: d.reason,
            signal_id: d.signal_id,
            fill_id: d.fill_id,
            latency_ms: d.latency_ms,
        })
        .collect();

    Json(response)
}

/// GET /api/decisions/:id - single decision by ID
async fn get_decision_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<Json<DecisionResponse>, StatusCode> {
    let decision = state
        .store
        .get_decision(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(DecisionResponse {
        id: decision.id,
        timestamp: decision.timestamp.to_rfc3339(),
        ticker: decision.ticker,
        action: decision.action.to_string(),
        side: decision.side.map(|s| format!("{:?}", s)),
        score: decision.score,
        confidence: decision.confidence,
        scorer_breakdown: decision.scorer_breakdown,
        reason: decision.reason,
        signal_id: decision.signal_id,
        fill_id: decision.fill_id,
        latency_ms: decision.latency_ms,
    }))
}
