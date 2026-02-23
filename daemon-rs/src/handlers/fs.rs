use axum::extract::{Json, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::handlers::utils::resolve_filepath;
use crate::server::AppState;
use std::path::Path;

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

#[derive(Deserialize)]
pub struct AnnotateRequest {
    pub path: String,
    pub ann_id: String,
    pub quote: String,
    pub prefix: String,
    pub author: String,
    pub body: String,
}

pub async fn annotate_file(
    State(state): State<AppState>,
    Json(req): Json<AnnotateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if req.path.is_empty() || req.ann_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Path and ann_id are required".into()));
    }

    let abs_path_str = resolve_filepath(&state, &req.path);
    let abs_path = Path::new(&abs_path_str);
    let content = tokio::fs::read_to_string(&abs_path).await.map_err(|e| {
        (StatusCode::NOT_FOUND, e.to_string())
    })?;

    let anchor = format!("[{}](#{})", req.quote, req.ann_id);
    if content.contains(&anchor) {
        return Ok(StatusCode::OK);
    }

    // Insert anchor in text
    let mut new_content = content.clone();
    let search_str = format!("{}{}", req.prefix, req.quote);
    if let Some(pos) = content.find(&search_str) {
        let insert_pos = pos + req.prefix.len();
        new_content.replace_range(insert_pos..insert_pos + req.quote.len(), &anchor);
    } else if let Some(pos) = content.find(&req.quote) {
        new_content.replace_range(pos..pos + req.quote.len(), &anchor);
    } else {
        return Err((StatusCode::NOT_FOUND, "Could not find quote in source file".into()));
    }

    tokio::fs::write(&abs_path, new_content).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Add to Loro
    state.loro_manager.add_annotation(abs_path, req.ann_id, req.author, req.body).await;

    Ok(StatusCode::OK)
}
