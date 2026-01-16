use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// A standardized response wrapper for the API.
/// This ensures consistent JSON structure across all endpoints.
#[derive(Serialize)]
pub struct ApiResponse<T> {
    /// Indicates if the request was successful.
    pub success: bool,
    /// A localized message describing the result (mostly for errors or confirmations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The actual data payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    /// Creates a success response with data.
    /// Status code defaults to 200 OK.
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            message: None,
            data: Some(data),
        }
    }

    /// Creates a success response with a message and data.
    #[allow(dead_code)]
    pub fn success_with_message(message: String, data: T) -> Self {
        Self {
            success: true,
            message: Some(message),
            data: Some(data),
        }
    }

    /// Creates an error response with a message (typically use AppError instead).
    #[allow(dead_code)]
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            message: Some(message),
            data: None,
        }
    }

    /// Converts to a response with a custom status code.
    /// Usage: `ApiResponse::success(data).with_status(StatusCode::CREATED)`
    pub fn with_status(self, status: StatusCode) -> ApiResponseWithStatus<T> {
        ApiResponseWithStatus {
            status,
            response: self,
        }
    }

    /// Shorthand for 201 Created response.
    /// Usage: `ApiResponse::success(data).created()`
    pub fn created(self) -> ApiResponseWithStatus<T> {
        self.with_status(StatusCode::CREATED)
    }

    /// Shorthand for 202 Accepted response.
    #[allow(dead_code)]
    pub fn accepted(self) -> ApiResponseWithStatus<T> {
        self.with_status(StatusCode::ACCEPTED)
    }

    /// Shorthand for 204 No Content response.
    #[allow(dead_code)]
    pub fn no_content(self) -> ApiResponseWithStatus<T> {
        self.with_status(StatusCode::NO_CONTENT)
    }
}

/// Helper struct for responses without data (e.g., just a message)
#[derive(Serialize)]
pub struct EmptyData;

impl ApiResponse<EmptyData> {
    /// Creates a success response with just a message.
    pub fn ok(message: String) -> Self {
        Self {
            success: true,
            message: Some(message),
            data: None,
        }
    }

    /// Creates a 201 Created response with just a message.
    #[allow(dead_code)]
    pub fn created_ok(message: String) -> ApiResponseWithStatus<EmptyData> {
        Self::ok(message).created()
    }
}

/// A wrapper that pairs an ApiResponse with a custom StatusCode.
/// This allows handlers to return responses with any HTTP status code.
pub struct ApiResponseWithStatus<T> {
    status: StatusCode,
    response: ApiResponse<T>,
}

impl<T> ApiResponseWithStatus<T>
where
    T: Serialize,
{
    /// Creates a new response with a custom status code.
    #[allow(dead_code)]
    pub fn new(status: StatusCode, response: ApiResponse<T>) -> Self {
        Self { status, response }
    }
}

impl<T> IntoResponse for ApiResponseWithStatus<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        (self.status, Json(self.response)).into_response()
    }
}

/// Implement IntoResponse for ApiResponse to simplify handler returns.
/// This defaults to 200 OK. For other status codes, use `.with_status()`, `.created()`, etc.
impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}
