//! WebSocket handler for real-time TUI updates

use super::{AppState, SessionMode};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub mode: SessionMode,
    pub session_id: String,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    Welcome {
        version: String,
        session: SessionInfo,
    },
    Snapshot {
        session: SessionInfo,
        engine: EngineSnapshot,
        portfolio: PortfolioSnapshot,
        positions: Vec<PositionSnapshot>,
        recent_fills: Vec<FillSnapshot>,
        equity_curve: Vec<EquityPointSnapshot>,
        beds: Vec<BedSnapshot>,
        circuit_breaker: CircuitBreakerSnapshot,
    },
    TickUpdate {
        session: SessionInfo,
        timestamp: String,
        engine: EngineSnapshot,
        portfolio: PortfolioSnapshot,
        positions: Vec<PositionSnapshot>,
        recent_fills: Vec<FillSnapshot>,
        equity_point: Option<EquityPointSnapshot>,
        pipeline: PipelineMetrics,
    },
    PositionOpened {
        position: PositionSnapshot,
        fill: FillSnapshot,
    },
    PositionClosed {
        ticker: String,
        pnl: f64,
        reason: String,
        fill: FillSnapshot,
    },
    SpecimenChanged {
        name: String,
        status: String,
        weight: f64,
    },
    CircuitBreakerTripped {
        timestamp: String,
        rule: String,
        details: String,
    },
    Decision {
        id: i64,
        timestamp: String,
        ticker: String,
        action: String,
        side: Option<String>,
        score: f64,
        confidence: f64,
        scorer_breakdown: std::collections::HashMap<String, f64>,
        reason: Option<String>,
        fill_id: Option<i64>,
        latency_ms: Option<u64>,
    },
    CommandAck {
        command: String,
        success: bool,
        message: Option<String>,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize)]
pub struct EngineSnapshot {
    pub state: String,
    pub uptime_secs: u64,
    pub last_tick: Option<String>,
    pub ticks_completed: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PortfolioSnapshot {
    pub cash: f64,
    pub equity: f64,
    pub initial_capital: f64,
    pub return_pct: f64,
    pub drawdown_pct: f64,
    pub positions_count: usize,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_pnl: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionSnapshot {
    pub ticker: String,
    pub title: String,
    pub category: String,
    pub side: String,
    pub quantity: u64,
    pub entry_price: f64,
    pub current_price: Option<f64>,
    pub entry_time: String,
    pub unrealized_pnl: f64,
    pub pnl_pct: f64,
    pub hours_held: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FillSnapshot {
    pub ticker: String,
    pub side: String,
    pub quantity: u64,
    pub price: f64,
    pub timestamp: String,
    pub fee: Option<f64>,
    pub pnl: Option<f64>,
    pub exit_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EquityPointSnapshot {
    pub timestamp: String,
    pub equity: f64,
    pub cash: f64,
    pub positions_value: f64,
    pub drawdown_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BedSnapshot {
    pub name: String,
    pub specimens: Vec<SpecimenSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecimenSnapshot {
    pub name: String,
    pub status: String,
    pub weight: f64,
    pub hit_rate: Option<f64>,
    pub avg_contribution: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CircuitBreakerSnapshot {
    pub status: String,
    pub drawdown_pct: f64,
    pub daily_loss_pct: f64,
    pub open_positions: usize,
    pub fills_last_hour: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineMetrics {
    pub candidates_fetched: usize,
    pub candidates_filtered: usize,
    pub candidates_selected: usize,
    pub signals_generated: usize,
    pub fills_executed: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    RequestSnapshot,
    Ping,
    PauseEngine,
    ResumeEngine,
    SetSpecimenStatus { name: String, status: String },
    SetSpecimenWeight { name: String, weight: f64 },
    ForceRefresh,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    let client_id = uuid::Uuid::new_v4().to_string();
    info!(client_id = %client_id, "websocket client connected");

    let session_info = get_session_info(&state).await;
    let welcome = ServerMessage::Welcome {
        version: env!("CARGO_PKG_VERSION").to_string(),
        session: session_info,
    };
    if let Err(e) = send_message(&mut sender, &welcome).await {
        error!(error = %e, "failed to send welcome");
        return;
    }

    let snapshot = build_snapshot(&state).await;
    if let Err(e) = send_message(&mut sender, &snapshot).await {
        error!(error = %e, "failed to send snapshot");
        return;
    }

    let mut updates_rx = state.updates_tx.subscribe();

    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    let sender_clone = sender.clone();
    let client_id_clone = client_id.clone();

    let forward_task = tokio::spawn(async move {
        loop {
            match updates_rx.recv().await {
                Ok(msg) => {
                    let mut guard = sender_clone.lock().await;
                    if let Err(e) = send_message(&mut *guard, &msg).await {
                        debug!(
                            client_id = %client_id_clone,
                            error = %e,
                            "failed to forward update, client likely disconnected"
                        );
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(
                        client_id = %client_id_clone,
                        missed = n,
                        "client lagging behind broadcasts"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!(client_id = %client_id_clone, "broadcast channel closed");
                    break;
                }
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(cmd) => {
                    let response = handle_command(cmd, &state).await;
                    let mut guard = sender.lock().await;
                    if let Err(e) = send_message(&mut *guard, &response).await {
                        debug!(error = %e, "failed to send command response");
                        break;
                    }
                }
                Err(e) => {
                    warn!(error = %e, text = %text, "invalid client message");
                }
            },
            Ok(Message::Close(_)) => {
                info!(client_id = %client_id, "client closed connection");
                break;
            }
            Ok(Message::Ping(data)) => {
                let mut guard = sender.lock().await;
                let _ = guard.send(Message::Pong(data)).await;
            }
            Ok(_) => {}
            Err(e) => {
                debug!(error = %e, "websocket error");
                break;
            }
        }
    }

    forward_task.abort();
    info!(client_id = %client_id, "websocket client disconnected");
}

async fn send_message<S>(sender: &mut S, msg: &ServerMessage) -> Result<(), axum::Error>
where
    S: SinkExt<Message, Error = axum::Error> + Unpin,
{
    let json = serde_json::to_string(msg).map_err(|e| {
        axum::Error::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    })?;
    sender.send(Message::Text(json.into())).await
}

async fn handle_command(cmd: ClientMessage, state: &Arc<AppState>) -> ServerMessage {
    match cmd {
        ClientMessage::Ping => ServerMessage::Pong,

        ClientMessage::RequestSnapshot => build_snapshot(state).await,

        ClientMessage::PauseEngine => {
            state
                .engine
                .pause("manual pause via websocket".to_string())
                .await;
            ServerMessage::CommandAck {
                command: "pause_engine".to_string(),
                success: true,
                message: None,
            }
        }

        ClientMessage::ResumeEngine => {
            state.engine.resume().await;
            ServerMessage::CommandAck {
                command: "resume_engine".to_string(),
                success: true,
                message: None,
            }
        }

        ClientMessage::SetSpecimenStatus { name, status } => {
            let mut specimens = state.specimens.write().await;
            if let Some(info) = specimens.get_mut(&name) {
                info.status = status.clone();

                let _ = state.updates_tx.send(ServerMessage::SpecimenChanged {
                    name: name.clone(),
                    status: info.status.clone(),
                    weight: info.weight,
                });

                ServerMessage::CommandAck {
                    command: "set_specimen_status".to_string(),
                    success: true,
                    message: Some(format!("set {} to {}", name, status)),
                }
            } else {
                ServerMessage::CommandAck {
                    command: "set_specimen_status".to_string(),
                    success: false,
                    message: Some(format!("specimen '{}' not found", name)),
                }
            }
        }

        ClientMessage::SetSpecimenWeight { name, weight } => {
            let mut specimens = state.specimens.write().await;
            if let Some(info) = specimens.get_mut(&name) {
                info.weight = weight.clamp(0.0, 2.0);

                let _ = state.updates_tx.send(ServerMessage::SpecimenChanged {
                    name: name.clone(),
                    status: info.status.clone(),
                    weight: info.weight,
                });

                ServerMessage::CommandAck {
                    command: "set_specimen_weight".to_string(),
                    success: true,
                    message: Some(format!("set {} weight to {:.2}", name, info.weight)),
                }
            } else {
                ServerMessage::CommandAck {
                    command: "set_specimen_weight".to_string(),
                    success: false,
                    message: Some(format!("specimen '{}' not found", name)),
                }
            }
        }

        ClientMessage::ForceRefresh => build_snapshot(state).await,
    }
}

async fn get_session_info(state: &Arc<AppState>) -> SessionInfo {
    let session = state.session.read().await;
    SessionInfo {
        mode: session.mode.clone(),
        session_id: session.session_id.clone(),
        started_at: session.started_at.map(|t| t.to_rfc3339()),
    }
}

async fn build_snapshot(state: &Arc<AppState>) -> ServerMessage {
    let session_info = get_session_info(state).await;
    let engine_status = state.engine.get_status().await;
    let ctx = state.engine.get_context().await;
    let current_prices = state.engine.get_current_prices().await;
    let now = chrono::Utc::now();

    let engine = EngineSnapshot {
        state: format!("{}", engine_status.state),
        uptime_secs: engine_status.uptime_secs,
        last_tick: engine_status.last_tick.map(|t| t.to_rfc3339()),
        ticks_completed: engine_status.ticks_completed,
    };

    let positions_value: f64 = ctx
        .portfolio
        .positions
        .values()
        .map(|p| p.avg_entry_price.to_f64().unwrap_or(0.0) * p.quantity as f64)
        .sum();

    let cash = ctx.portfolio.cash.to_f64().unwrap_or(0.0);
    let equity = cash + positions_value;
    let initial = ctx.portfolio.initial_capital.to_f64().unwrap_or(10000.0);
    let return_pct = if initial > 0.0 {
        (equity - initial) / initial * 100.0
    } else {
        0.0
    };

    let peak = state
        .store
        .get_peak_equity()
        .await
        .ok()
        .flatten()
        .and_then(|p| p.to_f64())
        .unwrap_or(equity);

    let drawdown_pct = if peak > 0.0 {
        ((peak - equity) / peak * 100.0).max(0.0)
    } else {
        0.0
    };

    let unrealized_pnl: f64 = ctx
        .portfolio
        .positions
        .values()
        .map(|p| {
            let entry = p.avg_entry_price.to_f64().unwrap_or(0.0);
            let current = current_prices.get(&p.ticker).and_then(|d| d.to_f64());
            if let Some(curr) = current {
                let effective_curr = match p.side {
                    pm_core::Side::Yes => curr,
                    pm_core::Side::No => 1.0 - curr,
                };
                (effective_curr - entry) * p.quantity as f64
            } else {
                0.0
            }
        })
        .sum();

    let realized_pnl = ctx.portfolio.realized_pnl.to_f64().unwrap_or(0.0);
    let total_pnl = realized_pnl + unrealized_pnl;

    let portfolio = PortfolioSnapshot {
        cash,
        equity,
        initial_capital: initial,
        return_pct,
        drawdown_pct,
        positions_count: ctx.portfolio.positions.len(),
        realized_pnl,
        unrealized_pnl,
        total_pnl,
    };

    let positions: Vec<PositionSnapshot> = ctx
        .portfolio
        .positions
        .values()
        .map(|p| {
            let entry = p.avg_entry_price.to_f64().unwrap_or(0.0);
            let current = current_prices.get(&p.ticker).and_then(|d| d.to_f64());

            let (unrealized_pnl, pnl_pct) = if let Some(curr) = current {
                let effective_curr = match p.side {
                    pm_core::Side::Yes => curr,
                    pm_core::Side::No => 1.0 - curr,
                };
                let pnl = (effective_curr - entry) * p.quantity as f64;
                let pct = if entry > 0.0 {
                    (effective_curr - entry) / entry * 100.0
                } else {
                    0.0
                };
                (pnl, pct)
            } else {
                (0.0, 0.0)
            };

            let hours_held = (now - p.entry_time).num_hours();

            PositionSnapshot {
                ticker: p.ticker.clone(),
                title: p.title.clone(),
                category: p.category.clone(),
                side: format!("{:?}", p.side),
                quantity: p.quantity,
                entry_price: entry,
                current_price: current,
                entry_time: p.entry_time.to_rfc3339(),
                unrealized_pnl,
                pnl_pct,
                hours_held,
            }
        })
        .collect();

    let fills = state.store.get_recent_fills(50).await.unwrap_or_default();
    let recent_fills: Vec<FillSnapshot> = fills
        .into_iter()
        .map(|f| FillSnapshot {
            ticker: f.ticker,
            side: format!("{:?}", f.side),
            quantity: f.quantity,
            price: f.price.to_f64().unwrap_or(0.0),
            timestamp: f.timestamp.to_rfc3339(),
            fee: f.fee.and_then(|fee| fee.to_f64()),
            pnl: f.pnl.and_then(|p| p.to_f64()),
            exit_reason: f.exit_reason,
        })
        .collect();

    let snapshots = state.store.get_equity_curve().await.unwrap_or_default();
    let equity_curve: Vec<EquityPointSnapshot> = snapshots
        .into_iter()
        .map(|s| EquityPointSnapshot {
            timestamp: s.timestamp.to_rfc3339(),
            equity: s.equity.to_f64().unwrap_or(0.0),
            cash: s.cash.to_f64().unwrap_or(0.0),
            positions_value: s.positions_value.to_f64().unwrap_or(0.0),
            drawdown_pct: s.drawdown_pct,
        })
        .collect();

    let specimens = state.specimens.read().await;
    let mut beds_map: std::collections::HashMap<String, Vec<SpecimenSnapshot>> =
        std::collections::HashMap::new();

    for (name, info) in specimens.iter() {
        let bed_name = info.bed.clone();
        beds_map
            .entry(bed_name)
            .or_default()
            .push(SpecimenSnapshot {
                name: name.clone(),
                status: info.status.clone(),
                weight: info.weight,
                hit_rate: info.hit_rate,
                avg_contribution: info.avg_contribution,
            });
    }

    let beds: Vec<BedSnapshot> = beds_map
        .into_iter()
        .map(|(name, specimens)| BedSnapshot { name, specimens })
        .collect();

    let cb_status = match engine_status.state {
        crate::engine::EngineState::Paused(ref reason) => format!("tripped: {}", reason),
        _ => "ok".to_string(),
    };

    let circuit_breaker = CircuitBreakerSnapshot {
        status: cb_status,
        drawdown_pct,
        daily_loss_pct: 0.0,
        open_positions: ctx.portfolio.positions.len(),
        fills_last_hour: 0,
    };

    ServerMessage::Snapshot {
        session: session_info,
        engine,
        portfolio,
        positions,
        recent_fills,
        equity_curve,
        beds,
        circuit_breaker,
    }
}

pub async fn build_tick_update(state: &Arc<AppState>, pipeline: PipelineMetrics) -> ServerMessage {
    let session_info = get_session_info(state).await;
    let engine_status = state.engine.get_status().await;
    let ctx = state.engine.get_context().await;
    let current_prices = state.engine.get_current_prices().await;
    let now = chrono::Utc::now();

    let engine = EngineSnapshot {
        state: format!("{}", engine_status.state),
        uptime_secs: engine_status.uptime_secs,
        last_tick: engine_status.last_tick.map(|t| t.to_rfc3339()),
        ticks_completed: engine_status.ticks_completed,
    };

    let positions_value: f64 = ctx
        .portfolio
        .positions
        .values()
        .map(|p| p.avg_entry_price.to_f64().unwrap_or(0.0) * p.quantity as f64)
        .sum();

    let cash = ctx.portfolio.cash.to_f64().unwrap_or(0.0);
    let equity = cash + positions_value;
    let initial = ctx.portfolio.initial_capital.to_f64().unwrap_or(10000.0);
    let return_pct = if initial > 0.0 {
        (equity - initial) / initial * 100.0
    } else {
        0.0
    };

    let peak = state
        .store
        .get_peak_equity()
        .await
        .ok()
        .flatten()
        .and_then(|p| p.to_f64())
        .unwrap_or(equity);

    let drawdown_pct = if peak > 0.0 {
        ((peak - equity) / peak * 100.0).max(0.0)
    } else {
        0.0
    };

    let unrealized_pnl: f64 = ctx
        .portfolio
        .positions
        .values()
        .map(|p| {
            let entry = p.avg_entry_price.to_f64().unwrap_or(0.0);
            let current = current_prices.get(&p.ticker).and_then(|d| d.to_f64());
            if let Some(curr) = current {
                let effective_curr = match p.side {
                    pm_core::Side::Yes => curr,
                    pm_core::Side::No => 1.0 - curr,
                };
                (effective_curr - entry) * p.quantity as f64
            } else {
                0.0
            }
        })
        .sum();

    let realized_pnl = ctx.portfolio.realized_pnl.to_f64().unwrap_or(0.0);
    let total_pnl = realized_pnl + unrealized_pnl;

    let portfolio = PortfolioSnapshot {
        cash,
        equity,
        initial_capital: initial,
        return_pct,
        drawdown_pct,
        positions_count: ctx.portfolio.positions.len(),
        realized_pnl,
        unrealized_pnl,
        total_pnl,
    };

    let positions: Vec<PositionSnapshot> = ctx
        .portfolio
        .positions
        .values()
        .map(|p| {
            let entry = p.avg_entry_price.to_f64().unwrap_or(0.0);
            let current = current_prices.get(&p.ticker).and_then(|d| d.to_f64());

            let (unrealized_pnl, pnl_pct) = if let Some(curr) = current {
                let effective_curr = match p.side {
                    pm_core::Side::Yes => curr,
                    pm_core::Side::No => 1.0 - curr,
                };
                let pnl = (effective_curr - entry) * p.quantity as f64;
                let pct = if entry > 0.0 {
                    (effective_curr - entry) / entry * 100.0
                } else {
                    0.0
                };
                (pnl, pct)
            } else {
                (0.0, 0.0)
            };

            let hours_held = (now - p.entry_time).num_hours();

            PositionSnapshot {
                ticker: p.ticker.clone(),
                title: p.title.clone(),
                category: p.category.clone(),
                side: format!("{:?}", p.side),
                quantity: p.quantity,
                entry_price: entry,
                current_price: current,
                entry_time: p.entry_time.to_rfc3339(),
                unrealized_pnl,
                pnl_pct,
                hours_held,
            }
        })
        .collect();

    let fills = state.store.get_recent_fills(10).await.unwrap_or_default();
    let recent_fills: Vec<FillSnapshot> = fills
        .into_iter()
        .map(|f| FillSnapshot {
            ticker: f.ticker,
            side: format!("{:?}", f.side),
            quantity: f.quantity,
            price: f.price.to_f64().unwrap_or(0.0),
            timestamp: f.timestamp.to_rfc3339(),
            fee: f.fee.and_then(|fee| fee.to_f64()),
            pnl: f.pnl.and_then(|p| p.to_f64()),
            exit_reason: f.exit_reason,
        })
        .collect();

    let equity_point = Some(EquityPointSnapshot {
        timestamp: now.to_rfc3339(),
        equity,
        cash,
        positions_value,
        drawdown_pct,
    });

    ServerMessage::TickUpdate {
        session: session_info,
        timestamp: now.to_rfc3339(),
        engine,
        portfolio,
        positions,
        recent_fills,
        equity_point,
        pipeline,
    }
}
