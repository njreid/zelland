use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::{error, info};

use crate::handlers::utils::resolve_filepath;
use crate::proto::zelland::{self, envelope::Payload, Envelope, NavigationTarget, Notification, NotificationSource, OpenViewRequest};
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

#[derive(Deserialize)]
pub struct NotifyRequest {
    pub title: String,
    pub body: String,
    pub question: Option<String>,
    pub session_name: Option<String>,
    pub tab_index: Option<u32>,
    pub pane_id: Option<u32>,
    pub source: Option<String>,
}

pub async fn trigger_notify(
    State(state): State<AppState>,
    Json(req): Json<NotifyRequest>,
) -> StatusCode {
    let target = req.session_name.as_ref().map(|s| NavigationTarget {
        session_name: s.clone(),
        tab_index: req.tab_index.unwrap_or(0),
        pane_id: req.pane_id.unwrap_or(0),
    });

    let source = match req.source.as_deref() {
        Some("claude_code")   => NotificationSource::ClaudeCode,
        Some("gemini_cli")    => NotificationSource::GeminiCli,
        Some("zellij_plugin") => NotificationSource::ZellijPlugin,
        _                     => NotificationSource::User,
    } as i32;

    let envelope = Envelope {
        payload: Some(Payload::Notification(Notification {
            title: req.title.clone(),
            body: req.body.clone(),
            target,
            question: req.question.unwrap_or_default(),
            source,
        })),
    };
    state.registry.broadcast(&envelope);
    info!("Sent notification: {}", req.title);
    StatusCode::OK
}

async fn generic_trigger(
    state: AppState,
    req: TriggerRequest,
    file_type: zelland::open_view_request::FileType,
) -> Result<String, (StatusCode, String)> {
    let abs_path_str = resolve_filepath(&state, &req.path);
    let path = PathBuf::from(&abs_path_str);

    let asset_id = state
        .asset_manager
        .register(&path)
        .await
        .map_err(|e| {
            error!("Failed to register asset: {}", e);
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to access file {}: {}", req.path, e),
            )
        })?;

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
