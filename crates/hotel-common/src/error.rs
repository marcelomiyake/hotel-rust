use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid request: {0}")]
    BadRequest(String),
    #[error("resource was not found")]
    NotFound,
    #[error("request conflicts with current availability")]
    Conflict,
    #[error("staff access is required")]
    Unauthorized,
    #[error("an upstream service is unavailable")]
    Upstream,
    #[error("internal service error")]
    Internal,
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        if error
            .as_database_error()
            .and_then(|db| db.code())
            .as_deref()
            == Some("23505")
        {
            return Self::Conflict;
        }
        tracing::error!(error = %error, "database operation failed");
        Self::Internal
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Upstream => StatusCode::BAD_GATEWAY,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = ErrorBody {
            error: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, response::IntoResponse};

    #[tokio::test]
    async fn maps_conflict_to_http_409_with_safe_message() {
        let response = AppError::Conflict.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"], "request conflicts with current availability");
    }

    #[tokio::test]
    async fn maps_validation_to_http_400() {
        assert_eq!(
            AppError::BadRequest("bad dates".into())
                .into_response()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_missing_and_unauthorized_errors() {
        assert_eq!(
            AppError::NotFound.into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Unauthorized.into_response().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn maps_upstream_and_internal_errors() {
        assert_eq!(
            AppError::Upstream.into_response().status(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            AppError::Internal.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
