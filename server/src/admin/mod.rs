pub mod csrf;
pub mod handlers;
pub mod i18n;
pub mod session;
pub mod system;
pub mod templates;

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;

pub async fn admin_css() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/admin.css"),
    )
}
