use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use tracing::info;

use crate::projects::{scan_projects, Project};
use crate::server::AppState;
use crate::watcher::WatchCommand;

pub async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<Project>>, (StatusCode, String)> {
    let projects = scan_projects(&state.config.projects_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read projects directory: {}", e),
        )
    })?;
    Ok(Json(projects))
}

#[derive(serde::Deserialize)]
pub struct ActivateRequest {
    pub project_id: String,
}

pub async fn activate_project(
    State(state): State<AppState>,
    Json(req): Json<ActivateRequest>,
) -> StatusCode {
    let project_path = state.config.projects_path.join(&req.project_id);
    if project_path.exists() && project_path.is_dir() {
        info!("Activating project: {}", req.project_id);
        let _ = state.watcher_tx.send(WatchCommand::AddRecursive(project_path)).await;
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}
