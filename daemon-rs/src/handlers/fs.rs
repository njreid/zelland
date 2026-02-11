use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ReadQuery {
    pub path: String,
}

pub async fn read_file(
    Query(query): Query<ReadQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    if query.path.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Path is required".into()));
    }

    let data = tokio::fs::read(&query.path).await.map_err(|e| {
        (StatusCode::NOT_FOUND, e.to_string())
    })?;

    Ok(data)
}
