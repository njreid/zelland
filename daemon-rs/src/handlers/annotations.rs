use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tracing::info;
use std::sync::Arc;

use crate::handlers::utils::resolve_filepath;
use crate::server::AppState;
use crate::loro_manager::LoroManager;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub author: String,
    pub timestamp: String,
    pub body: String,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientCommand {
    AddComment { ann_id: String, author: String, body: String },
    Delete { ann_id: String },
}

/// WS /annotations/sync/*filepath — JSON WebSocket sync endpoint.
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

    // Subscribe: get initial JSON state and a broadcast receiver
    let (initial_json, mut broadcast_rx) = loro_manager.subscribe(path).await;

    // Send initial state as JSON text
    if ws_sender.send(Message::Text(initial_json.into())).await.is_err() {
        return;
    }

    // Spawn a relay task: forward broadcast JSON updates to this client
    let relay_task = tokio::spawn(async move {
        while let Ok(json) = broadcast_rx.recv().await {
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Read loop: accept JSON commands from client
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<ClientCommand>(&text) {
                    Ok(ClientCommand::AddComment { ann_id, author, body }) => {
                        loro_manager.add_comment(path, &ann_id, author, body).await;
                    }
                    Ok(ClientCommand::Delete { ann_id }) => {
                        loro_manager.delete_annotation(path, &ann_id).await;
                    }
                    Err(e) => {
                        tracing::warn!("Unknown annotation command from client: {} — {:?}", text, e);
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            _ => {}
        }
    }

    relay_task.abort();
    info!("Annotation sync disconnected for: {:?}", path);
}

pub async fn get_annotations() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
pub async fn put_annotations() -> StatusCode { StatusCode::NOT_IMPLEMENTED }
