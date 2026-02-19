use axum::extract::{Json, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use tracing::{info, warn};

use crate::handlers::utils::resolve_filepath;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct ReadQuery {
    pub path: String,
}

pub async fn read_file(
    State(state): State<AppState>,
    Query(query): Query<ReadQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if query.path.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Path is required".into()));
    }

    let abs_path = resolve_filepath(&state, &query.path);
    let data = tokio::fs::read(&abs_path).await.map_err(|e| {
        (StatusCode::NOT_FOUND, e.to_string())
    })?;

    Ok(data)
}

#[derive(Deserialize)]
pub struct MutateRequest {
    pub path: String,
    pub ann_id: String,
    pub quote: String,
    pub prefix: String,
    pub suffix: String,
}

pub async fn mutate_file(
    State(state): State<AppState>,
    Json(req): Json<MutateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if req.path.is_empty() || req.ann_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Path and ann_id are required".into()));
    }

    let abs_path = resolve_filepath(&state, &req.path);
    let content = tokio::fs::read_to_string(&abs_path).await.map_err(|e| {
        (StatusCode::NOT_FOUND, e.to_string())
    })?;

    // Check if marker already exists
    let marker = format!("[|{}|]", req.ann_id);
    if content.contains(&marker) {
        return Ok(StatusCode::OK);
    }

    // Try to find the exact position using prefix + quote
    let search_str = format!("{}{}", req.prefix, req.quote);
    if let Some(pos) = content.find(&search_str) {
        let insert_pos = pos + req.prefix.len();
        let mut new_content = content.clone();
        new_content.insert_str(insert_pos, &marker);

        tokio::fs::write(&abs_path, new_content).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

        info!("Inserted marker {} into {} at offset {}", marker, abs_path, insert_pos);
        return Ok(StatusCode::OK);
    }

    // Fallback: try finding just the quote if prefix is empty or didn't match
    if let Some(pos) = content.find(&req.quote) {
        let mut new_content = content.clone();
        new_content.insert_str(pos, &marker);

        tokio::fs::write(&abs_path, new_content).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

        warn!("Inserted marker {} into {} using quote-only match (prefix failed)", marker, abs_path);
        return Ok(StatusCode::OK);
    }

    Err((StatusCode::NOT_FOUND, "Could not find anchor text in source file".into()))
}
