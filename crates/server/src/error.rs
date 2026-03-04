use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
// use serde_json::json; // Unused for now
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Authentication failed")]
    Unauthorized,

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

use crate::api::{ApiErrorDetail, ApiResponse, ResponseCode};

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::Database(ref e) => {
                tracing::error!("Database error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseCode::InternalError,
                    "Internal server error".to_string(),
                )
            }
            ApiError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                ResponseCode::Unauthorized,
                "Authentication failed".to_string(),
            ),
            ApiError::Forbidden(ref msg) => {
                (StatusCode::FORBIDDEN, ResponseCode::Forbidden, msg.clone())
            }
            ApiError::NotFound(ref msg) => {
                (StatusCode::NOT_FOUND, ResponseCode::NotFound, msg.clone())
            }
            ApiError::Conflict(ref msg) => {
                (StatusCode::CONFLICT, ResponseCode::Conflict, msg.clone())
            }
            ApiError::Validation(ref msg) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                ResponseCode::ValidationError,
                msg.clone(),
            ),
            ApiError::BadRequest(ref msg) => (
                StatusCode::BAD_REQUEST,
                ResponseCode::BadRequest,
                msg.clone(),
            ),
            ApiError::Internal(ref msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseCode::InternalError,
                    "Internal server error".to_string(),
                )
            }
        };

        let response: ApiResponse<()> = ApiResponse {
            success: false,
            code,
            message,
            data: None,
            metadata: None,
            error: Some(ApiErrorDetail {
                details: Some(self.to_string()),
                trace_id: None, // Could add trace ID here if available
            }),
        };

        (status, Json(response)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
