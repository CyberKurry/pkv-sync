use crate::service::AppState;
use axum::Router;

pub mod invites;
pub mod system;
pub mod users;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(users::router())
        .merge(invites::router())
        .merge(system::router())
}
