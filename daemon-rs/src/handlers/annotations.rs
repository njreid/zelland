use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tracing::info;
use std::sync::Arc;
use loro::ExportMode;

use crate::handlers::utils::resolve_filepath;
use crate::server::AppState;
use crate::loro_manager::LoroManager;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub author: String,
    pub timestamp: String,
    pub body: String,
}

/// WS /annotations/sync/*filepath — Loro WebSocket sync endpoint.
pub async fn annotation_sync(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(filepath): Path<String>,
) -> impl IntoResponse {
    let abs_path = resolve_filepath(&state, &filepath);
    ws.on_upgrade(move |socket| handle_annotation_ws(socket, state.loro_manager, abs_path))
}

async fn handle_annotation_ws(socket: WebSocket, loro_manager: Arc<LoroManager>, filepath: String) {
    let path = std::path::Path::new(&filepath);
    info!("Annotation sync connected for: {:?}", path);
    
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (doc, mut broadcast_rx) = loro_manager.get_or_create(path).await;

    // Send initial state
    let initial_state = doc.export(ExportMode::Snapshot).unwrap();
    if ws_sender.send(Message::Binary(initial_state.into())).await.is_err() {
        return;
    }

    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

    // Relay broadcast updates to this client
    let relay_tx = out_tx.clone();
    tokio::spawn(async move {
        while let Ok(update) = broadcast_rx.recv().await {
            if relay_tx.send(update).await.is_err() {
                break;
            }
        }
    });

    // Handle outbound messages
    let out_task = tokio::spawn(async move {
        while let Some(data) = out_rx.recv().await {
            if ws_sender.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop: process incoming sync messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                loro_manager.apply_update(path, &data).await;
            }
            Ok(Message::Close(_)) => break,
            _ => {}
        }
    }

    out_task.abort();
    info!("Annotation sync disconnected for: {:?}", path);
}

// Placeholder for legacy handlers if needed, or we can remove them
pub async fn get_annotations() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn put_annotations() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
