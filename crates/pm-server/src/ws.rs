//! WebSocket handler for real-time garden updates
//!
//! Clients connect to /ws and receive updates as JSON messages
//! whenever the garden changes.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::state::AppState;

/// Garden update variants sent over WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GardenUpdate {
    Specimen(SpecimenUpdate),
    Harvest(HarvestUpdate),
    Yield(YieldUpdate),
    Status(StatusUpdate),
    Decision(DecisionUpdate),
}

/// Update when a specimen's state changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecimenUpdate {
    pub bed: String,
    pub specimen: String,
    pub status: String,
    pub score: f64,
    pub contribution: f64,
}

/// Update when a harvest (fill) occurs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestUpdate {
    pub ticker: String,
    pub side: String,
    pub quantity: u64,
    pub price: f64,
    pub yield_pnl: f64,
    pub reason: String,
}

/// Update for yield (P&L) changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldUpdate {
    pub total_yield: f64,
    pub daily_yield: f64,
    pub equity: f64,
    pub drawdown_pct: f64,
}

/// Update for overall garden status changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusUpdate {
    pub status: String,
    pub message: String,
    pub timestamp: String,
}

/// Update when a decision is made about a market candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionUpdate {
    pub id: i64,
    pub timestamp: String,
    pub ticker: String,
    pub action: String,
    pub side: Option<String>,
    pub score: f64,
    pub confidence: f64,
    pub scorer_breakdown: std::collections::HashMap<String, f64>,
    pub reason: Option<String>,
    pub fill_id: Option<i64>,
    pub latency_ms: Option<i64>,
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    debug!("new websocket connection");

    let mut rx = state.subscribe();

    loop {
        tokio::select! {
            // receive updates and send to client
            result = rx.recv() => {
                match result {
                    Ok(update) => {
                        let json = match serde_json::to_string(&update) {
                            Ok(j) => j,
                            Err(e) => {
                                warn!(error = %e, "failed to serialize update");
                                continue;
                            }
                        };
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            // client disconnected
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "client lagged, skipping messages");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // handle incoming messages (ping/pong, close)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }

    debug!("websocket connection closed");
}
