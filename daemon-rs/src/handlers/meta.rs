use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::server::AppState;

#[derive(Serialize)]
pub struct VersionResponse {
    pub name: &'static str,
    pub version: &'static str,
}

pub async fn get_version(State(_state): State<AppState>) -> Json<VersionResponse> {
    Json(VersionResponse {
        name: "zlnd",
        version: env!("CARGO_PKG_VERSION"),
    })
}
