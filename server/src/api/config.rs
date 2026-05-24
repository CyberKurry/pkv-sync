use crate::service::AppState;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ConfigResponse {
    pub server_name: String,
    pub version: &'static str,
    pub registration: &'static str,
    pub max_file_size: u64,
    pub supported_text_extensions: Vec<String>,
    pub capabilities: ServerCapabilities,
    /// Push debounce that clients should use; runtime-tuned for sub-second SSE.
    pub push_debounce_ms: u32,
    /// Inline content cap for SSE event payload; clients use it to know whether
    /// to expect inline content vs ref-only for a given file size.
    pub inline_content_max_bytes: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerCapabilities {
    pub history: bool,
    pub diff: bool,
    pub sse: bool,
    pub git_smart_http: bool,
}

fn response(
    server_name: String,
    registration: &'static str,
    max_file_size: u64,
    text_extensions: Vec<String>,
    capabilities: ServerCapabilities,
    push_debounce_ms: u32,
    inline_content_max_bytes: u32,
) -> ConfigResponse {
    ConfigResponse {
        server_name,
        version: env!("CARGO_PKG_VERSION"),
        registration,
        max_file_size,
        supported_text_extensions: text_extensions,
        capabilities,
        push_debounce_ms,
        inline_content_max_bytes,
    }
}

pub async fn config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let cfg = state.runtime_cfg.snapshot().await;
    Json(response(
        cfg.server_name,
        cfg.registration_mode.as_str(),
        cfg.max_file_size,
        cfg.text_extensions,
        ServerCapabilities {
            history: cfg.enable_history_ui,
            diff: cfg.enable_diff_endpoint,
            sse: true,
            // Capability flag must reflect both the admin toggle and whether
            // the `git` binary is actually available on the server. Otherwise
            // a client sees git_smart_http: true and then hits 503 at request
            // time.
            git_smart_http: cfg.enable_git_smart_http && state.git_available,
        },
        cfg.push_debounce_ms,
        cfg.inline_content_max_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::{RegistrationMode, RuntimeConfigRepo};
    use crate::service::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn setup_app(mode: RegistrationMode) -> Router {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "default".into(), true)
            .await
            .unwrap();
        state
            .runtime_cfg_repo
            .set_registration_mode(mode, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;
        Router::new()
            .route("/api/config", get(config))
            .with_state(state)
    }

    #[tokio::test]
    async fn returns_disabled_by_default() {
        let app = setup_app(RegistrationMode::Disabled).await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(v["registration"], "disabled");
        assert_eq!(v["max_file_size"], 100 * 1024 * 1024);
        assert!(v["supported_text_extensions"].is_array());
        assert_eq!(v["capabilities"]["history"], true);
        assert_eq!(v["capabilities"]["diff"], true);
    }

    #[tokio::test]
    async fn returns_invite_only_when_set() {
        let app = setup_app(RegistrationMode::InviteOnly).await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["registration"], "invite_only");
    }
}
