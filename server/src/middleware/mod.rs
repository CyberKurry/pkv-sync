pub mod deployment_key;
pub mod real_ip;
pub mod request_id;
pub mod ua_filter;

/// CORS `Access-Control-Allow-Headers` value used by the SSE events endpoint
/// and by the `cors_aware_reject` helper in both `deployment_key` and
/// `ua_filter` middlewares. Keeping this in one place avoids the three
/// call-sites drifting out of sync.
pub const SSE_CORS_ALLOW_HEADERS: &str =
    "authorization, accept, cache-control, user-agent, x-pkvsync-deployment-key, last-event-id";
