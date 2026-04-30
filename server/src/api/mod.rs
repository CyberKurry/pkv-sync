use crate::service::AppState;
use axum::Router;

pub mod admin;
pub mod auth;
pub mod config;
pub mod error;
pub mod health;
pub mod me;
pub mod vaults;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", axum::routing::get(health::health))
        .route("/api/config", axum::routing::get(config::config))
        .merge(auth::router())
        .merge(me::router())
        .merge(vaults::router())
        .merge(admin::router())
}
