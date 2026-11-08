pub mod csrf;
pub mod handlers;
pub mod i18n;
pub(crate) mod password;
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

pub async fn admin_js() -> impl IntoResponse {
    javascript(include_str!("../../static/admin.js"))
}

pub async fn admin_markdown_js() -> impl IntoResponse {
    javascript(include_str!("../../static/admin-markdown.js"))
}

pub async fn marked_js() -> impl IntoResponse {
    javascript(include_str!("../../static/vendor/marked.umd.js"))
}

pub async fn purify_js() -> impl IntoResponse {
    javascript(include_str!("../../static/vendor/purify.min.js"))
}

fn javascript(source: &'static str) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        source,
    )
}

pub async fn admin_icons() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        include_str!("../../static/lucide-icons.svg"),
    )
}

#[cfg(test)]
mod tests {
    fn admin_password_helper_body() -> &'static str {
        let source = include_str!("password.rs");
        let start = source
            .find("pub(crate) async fn hash_admin_password")
            .expect("admin password helper is async");
        let tests_start = source.find("#[cfg(test)]").unwrap_or(source.len());
        &source[start..tests_start]
    }

    #[test]
    fn admin_password_hashing_is_offloaded_from_async_handlers() {
        let body = admin_password_helper_body();

        assert!(body.contains("spawn_blocking"));
    }

    #[test]
    fn admin_password_hashing_does_not_double_validate_strength() {
        let body = admin_password_helper_body();

        assert!(!body.contains("validate_strong"));
    }
}
