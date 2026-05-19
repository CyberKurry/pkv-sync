pub mod deployment_key;
pub mod real_ip;
pub mod request_id;
pub mod ua_filter;

use axum::http::header::HeaderName;

/// CORS `Access-Control-Allow-Headers` value used by the SSE events endpoint
/// and by the `cors_aware_reject` helper in both `deployment_key` and
/// `ua_filter` middlewares. Keeping this in one place avoids the three
/// call-sites drifting out of sync.
pub const SSE_CORS_ALLOW_HEADERS: &str =
    "authorization, accept, cache-control, user-agent, x-pkvsync-plugin, x-pkvsync-deployment-key, last-event-id";

pub const SSE_PLUGIN_HEADER: &str = "x-pkvsync-plugin";

pub fn sse_cors_allow_header_names() -> Vec<HeaderName> {
    SSE_CORS_ALLOW_HEADERS
        .split(',')
        .map(str::trim)
        .map(|name| HeaderName::from_bytes(name.as_bytes()).expect("valid SSE CORS header name"))
        .collect()
}
