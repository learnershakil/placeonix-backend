use std::{error::Error, fmt};

use axum::{response::IntoResponse, Json};
use http::StatusCode;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    BadRequest,
    ValidationFailed,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    RateLimited,
    PayloadTooLarge,
    UnsupportedMediaType,
    ServiceUnavailable,
    Internal,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadRequest => "bad_request",
            Self::ValidationFailed => "validation_failed",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::RateLimited => "rate_limited",
            Self::PayloadTooLarge => "payload_too_large",
            Self::UnsupportedMediaType => "unsupported_media_type",
            Self::ServiceUnavailable => "service_unavailable",
            Self::Internal => "internal",
        }
    }

    pub const fn status(self) -> StatusCode {
        match self {
            Self::BadRequest | Self::ValidationFailed => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl Serialize for ErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct AppError {
    code: ErrorCode,
    message: String,
    request_id: Option<String>,
    details: Option<Value>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            request_id: None,
            details: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ValidationFailed, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::RateLimited, message)
    }

    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PayloadTooLarge, message)
    }

    pub fn unsupported_media_type(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::UnsupportedMediaType, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ServiceUnavailable, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn status(&self) -> StatusCode {
        self.code.status()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn envelope(&self) -> ErrorEnvelope {
        ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message.clone(),
                request_id: self.request_id.clone(),
                details: self.details.clone(),
            },
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (self.status(), Json(self.envelope())).into_response()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuccessEnvelope<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<PageMeta>,
}

impl<T> SuccessEnvelope<T> {
    pub fn data(data: T) -> Self {
        Self {
            success: true,
            data,
            meta: None,
        }
    }

    pub fn paged(data: T, meta: PageMeta) -> Self {
        Self {
            success: true,
            data,
            meta: Some(meta),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageMeta {
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenPairResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeResponse {
    pub tenant_id: String,
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionResponse {
    pub id: String,
    pub device_id: Option<String>,
    pub status: String,
    pub issued_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceResponse {
    pub id: String,
    pub label: Option<String>,
    pub user_agent: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub revoked_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use serde_json::json;

    use super::{AppError, ErrorCode, MeResponse, PageMeta, SuccessEnvelope};

    #[test]
    fn serializes_stable_error_envelope() {
        let value = serde_json::to_value(
            AppError::validation_failed("email is invalid")
                .with_request_id("req-123")
                .with_details(json!({ "field": "email" }))
                .envelope(),
        )
        .expect("error envelope serializes");

        assert_eq!(
            value,
            json!({
                "error": {
                    "code": "validation_failed",
                    "message": "email is invalid",
                    "request_id": "req-123",
                    "details": { "field": "email" }
                }
            })
        );
    }

    #[test]
    fn maps_error_codes_to_http_statuses() {
        assert_eq!(
            ErrorCode::Unauthorized.status(),
            http::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ErrorCode::ServiceUnavailable.status(),
            http::StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn app_error_converts_to_http_response() {
        let response = AppError::not_found("route not found").into_response();

        assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn serializes_success_envelope_with_meta() {
        let value = serde_json::to_value(SuccessEnvelope::paged(
            vec![MeResponse {
                tenant_id: "tenant-1".to_owned(),
                user_id: "user-1".to_owned(),
                session_id: Some("session-1".to_owned()),
                device_id: None,
                roles: vec!["student".to_owned()],
                permissions: vec!["courses:read".to_owned()],
            }],
            PageMeta {
                limit: 50,
                next_cursor: Some("cursor-1".to_owned()),
            },
        ))
        .expect("success envelope serializes");

        assert_eq!(
            value,
            json!({
                "success": true,
                "data": [{
                    "tenant_id": "tenant-1",
                    "user_id": "user-1",
                    "session_id": "session-1",
                    "roles": ["student"],
                    "permissions": ["courses:read"]
                }],
                "meta": {
                    "limit": 50,
                    "next_cursor": "cursor-1"
                }
            })
        );
    }
}
