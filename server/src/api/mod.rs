use crate::middleware::rate_limit;
use crate::service::AppState;
use axum::Router;

pub mod admin;
pub mod auth;
pub mod config;
pub mod error;
pub mod git_http;
pub mod health;
pub mod me;
pub mod metrics;
pub mod plugin_manifest;
pub mod vault_settings;
pub mod vaults;

pub fn router() -> Router<AppState> {
    let git_routes = Router::new()
        .route(
            "/git/:vault_id/info/refs",
            axum::routing::get(git_http::info_refs),
        )
        .route(
            "/git/:vault_id/git-upload-pack",
            axum::routing::post(git_http::upload_pack),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            rate_limit::RequestRateLimiter::git_http(),
            rate_limit::git_http_middleware,
        ));

    Router::new()
        .route("/metrics", axum::routing::get(metrics::metrics))
        .route("/api/health", axum::routing::get(health::health))
        .route("/api/config", axum::routing::get(config::config))
        .merge(auth::router())
        .merge(me::router())
        .merge(plugin_manifest::router())
        .merge(vault_settings::router())
        .merge(vaults::router())
        .merge(admin::router())
        .merge(git_routes)
}
