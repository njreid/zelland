use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::projects::{scan_projects, Project};
use crate::server::AppState;

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

pub async fn activate_project() -> StatusCode {
    // Just acknowledge for now — client handles SSH/session connection
    StatusCode::OK
}
