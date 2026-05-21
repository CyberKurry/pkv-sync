use crate::api::error::ApiError;
use crate::auth::AdminUser;
use crate::service::AppState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderValue};
use axum::response::Response;

pub async fn metrics(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Result<Response, ApiError> {
    if !state.runtime_cfg.snapshot().await.enable_metrics {
        return Err(ApiError::not_found("metrics disabled"));
    }
    state
        .refresh_metrics_gauges()
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?;

    let mut response = Response::new(Body::from(state.metrics.encode()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    Ok(response)
}
