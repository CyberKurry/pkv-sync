use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized".into(),
            message: msg.into(),
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden".into(),
            message: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found".into(),
            message: msg.into(),
        }
    }

    pub fn bad_request(code: &str, msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: code.into(),
            message: msg.into(),
        }
    }

    pub fn conflict(code: &str, msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: code.into(),
            message: msg.into(),
        }
    }

    pub fn too_many(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "rate_limited".into(),
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        let msg = msg.into();
        tracing::error!(error = %msg, "internal server error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal".into(),
            message: "internal server error".into(),
        }
    }

    /// Construct an `ApiError` with an arbitrary status code, code, and message.
    pub fn new(status: StatusCode, code: &str, msg: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": ErrorBody { code: self.code, message: self.message },
        });
        (self.status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn internal_error_response_hides_original_message() {
        let secret = "argon2 failed at /var/lib/pkv-sync/vaults/main.git";

        let response = ApiError::internal(secret).into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body_text = String::from_utf8(body.to_vec()).unwrap();
        let body: serde_json::Value = serde_json::from_str(&body_text).unwrap();
        assert_eq!(body["error"]["code"], "internal");
        assert_eq!(body["error"]["message"], "internal server error");
        assert!(!body_text.contains(secret));
        assert!(!body_text.contains("/var/lib/pkv-sync"));
    }
}
