use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::{error, info};

use crate::proto::zelland::{self, envelope::Payload, Envelope, OpenViewRequest};
use crate::server::AppState;
use crate::watcher::WatchCommand;

#[derive(Deserialize)]
pub struct TriggerRequest {
    pub path: String,
    #[serde(default)]
    pub title: String,
}

pub async fn trigger_show(
    State(state): State<AppState>,
    Json(req): Json<TriggerRequest>,
) -> Result<String, (StatusCode, String)> {
    generic_trigger(state, req, zelland::open_view_request::FileType::Image).await
}

pub async fn trigger_md(
    State(state): State<AppState>,
    Json(req): Json<TriggerRequest>,
) -> Result<String, (StatusCode, String)> {
    generic_trigger(state, req, zelland::open_view_request::FileType::Markdown).await
}

async fn generic_trigger(
    state: AppState,
    req: TriggerRequest,
    file_type: zelland::open_view_request::FileType,
) -> Result<String, (StatusCode, String)> {
    let path = PathBuf::from(&req.path);

    let asset_id = state
        .asset_manager
        .register(&path)
        .await
        .map_err(|e| {
            error!("Failed to register asset: {}", e);
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to access file: {}", e),
            )
        })?;

    // Store path mapping
    state
        .asset_paths
        .write()
        .await
        .insert(asset_id.clone(), req.path.clone());

    // Add to file watcher
    let _ = state.watcher_tx.send(WatchCommand::Add(path)).await;

    // Broadcast to connected clients
    let title = if req.title.is_empty() {
        PathBuf::from(&req.path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        req.title
    };

    let envelope = Envelope {
        payload: Some(Payload::OpenView(OpenViewRequest {
            asset_id: asset_id.clone(),
            url: format!("/assets/{}", asset_id),
            file_type: file_type as i32,
            title,
        })),
    };

    state.registry.broadcast(&envelope);
    info!("Triggered {} for {}", file_type.as_str_name(), req.path);

    Ok(format!("Showing {} (ID: {})", req.path, asset_id))
}
