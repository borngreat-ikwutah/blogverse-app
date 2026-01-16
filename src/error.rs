use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[allow(dead_code)]
pub enum AppError {
    InternalServerError,
    BadRequest(String),
    Unauthorized,
    NotFound(String),
    Conflict(String),
    UnprocessableEntity(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            ),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
        };

        // Standardized failure response matching ApiResponse structure
        let body = Json(json!({
            "success": false,
            "message": error_message,
            "data": null
        }));

        (status, body).into_response()
    }
}
