use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::handlers::utils::resolve_filepath;
use crate::server::AppState;
use crate::store::Ann;
use crate::sync::{self, SyncMessage};
use crate::yjs::DocManager;

/// GET /annotations/*filepath — Read annotations as JSON.
pub async fn get_annotations(
    State(state): State<AppState>,
    Path(filepath): Path<String>,
) -> Result<Json<Vec<Ann>>, (StatusCode, String)> {
    let abs_path = resolve_filepath(&state, &filepath);
    let anns = state
        .doc_manager
        .read_annotations(&abs_path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(anns))
}

/// PUT /annotations/*filepath — Write annotations (non-YJS fallback).
pub async fn put_annotations(
    State(state): State<AppState>,
    Path(filepath): Path<String>,
    Json(anns): Json<Vec<Ann>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let abs_path = resolve_filepath(&state, &filepath);
    let ann_path = crate::store::ann_kdl_path(std::path::Path::new(&abs_path));
    crate::store::save_anns(&ann_path, &anns)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

/// WS /annotations/sync/*filepath — YJS WebSocket sync endpoint.
pub async fn annotation_sync(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(filepath): Path<String>,
) -> impl IntoResponse {
    let abs_path = resolve_filepath(&state, &filepath);
    ws.on_upgrade(move |socket| handle_annotation_ws(socket, state.doc_manager, abs_path))
}

async fn handle_annotation_ws(socket: WebSocket, doc_manager: DocManager, filepath: String) {
    info!("Annotation sync connected for: {}", filepath);
    let (ws_sender, mut ws_receiver) = socket.split();

    // Ensure the Doc exists (loads from disk if needed)
    let _doc_state = doc_manager.get_or_create(&filepath).await;

    // Channel for outbound messages (replies + broadcast relay)
    let (out_tx, mut out_rx) = mpsc::channel::<Vec<u8>>(64);

    // Send our SyncStep1 (state vector) to the client
    let sv = match doc_manager.state_vector(&filepath).await {
        Ok(sv) => sv,
        Err(e) => {
            error!("Failed to get state vector: {}", e);
            return;
        }
    };
    let step1_msg = sync::encode_sync_step1(&sv);
    if out_tx.send(step1_msg).await.is_err() {
        return;
    }

    // Subscribe to broadcast updates for this document
    let mut update_rx = doc_manager.subscribe(&filepath).await;

    // Spawn relay task: merges broadcast updates + direct replies → WebSocket sender
    let relay_out_tx = out_tx.clone();
    let relay = tokio::spawn(async move {
        let mut ws_sender = ws_sender;
        loop {
            tokio::select! {
                Ok(update_data) = update_rx.recv() => {
                    let msg = sync::encode_update(&update_data);
                    if ws_sender.send(Message::Binary(msg.into())).await.is_err() {
                        break;
                    }
                }
                Some(data) = out_rx.recv() => {
                    if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
        drop(relay_out_tx);
    });

    // Read loop: process incoming sync messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Binary(data)) => match sync::decode_message(&data) {
                Ok(SyncMessage::SyncStep1(client_sv)) => {
                    // Client sent their state vector — respond with our diff
                    match doc_manager.encode_diff(&filepath, &client_sv).await {
                        Ok(update) => {
                            let reply = sync::encode_sync_step2(&update);
                            if out_tx.send(reply).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to encode diff: {}", e);
                        }
                    }
                }
                Ok(SyncMessage::SyncStep2(update)) | Ok(SyncMessage::Update(update)) => {
                    if let Err(e) = doc_manager.apply_update(&filepath, &update).await {
                        warn!("Failed to apply update: {}", e);
                    }
                }
                Ok(SyncMessage::Awareness(_)) => {
                    debug!("Awareness message received (not yet forwarded)");
                }
                Err(e) => {
                    warn!("Invalid sync message: {}", e);
                }
            },
            Ok(Message::Close(_)) => {
                info!("Annotation sync client sent close");
                break;
            }
            Ok(_) => {} // Ignore text, ping, pong
            Err(e) => {
                debug!("WebSocket read error: {}", e);
                break;
            }
        }
    }

    relay.abort();

    // On disconnect, flush the document
    if let Err(e) = doc_manager.flush(&filepath).await {
        error!("Failed to flush on disconnect: {}", e);
    }
    info!("Annotation sync disconnected for: {}", filepath);
}
