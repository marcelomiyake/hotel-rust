use axum::{Json, http::StatusCode};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: &'static str,
}

pub async fn health() -> (StatusCode, Json<HealthStatus>) {
    (StatusCode::OK, Json(HealthStatus { status: "ok" }))
}
