pub mod deployment_key;
pub mod rate_limit;
pub mod real_ip;
pub mod request_id;
pub mod ua_filter;

use axum::http::header::HeaderName;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

/// CORS `Access-Control-Allow-Headers` value used by the SSE events endpoint
/// and by the `cors_aware_reject` helper in both `deployment_key` and
/// `ua_filter` middlewares. Keeping this in one place avoids the three
/// call-sites drifting out of sync.
pub const SSE_CORS_ALLOW_HEADERS: &str =
    "authorization, accept, cache-control, user-agent, x-pkvsync-plugin, x-pkvsync-deployment-key, last-event-id";

pub const SSE_PLUGIN_HEADER: &str = "x-pkvsync-plugin";

/// If a rejection happens on the browser-facing SSE endpoint, include the same
/// CORS headers as successful SSE responses so the plugin receives the useful
/// status code instead of a generic browser CORS failure.
pub(crate) fn cors_aware_reject<B>(req: &axum::http::Request<B>, status: StatusCode) -> Response {
    let mut resp = status.into_response();
    if req.uri().path().ends_with("/events") && req.headers().get(header::ORIGIN).is_some() {
        let h = resp.headers_mut();
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
        h.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, OPTIONS".parse().unwrap(),
        );
        h.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            SSE_CORS_ALLOW_HEADERS.parse().unwrap(),
        );
    }
    resp
}

pub fn sse_cors_allow_header_names() -> Vec<HeaderName> {
    SSE_CORS_ALLOW_HEADERS
        .split(',')
        .map(str::trim)
        .map(|name| HeaderName::from_bytes(name.as_bytes()).expect("valid SSE CORS header name"))
        .collect()
}
