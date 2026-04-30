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
}

fn response(
    server_name: String,
    registration: &'static str,
    max_file_size: u64,
    text_extensions: Vec<String>,
) -> ConfigResponse {
    ConfigResponse {
        server_name,
        version: env!("CARGO_PKG_VERSION"),
        registration,
        max_file_size,
        supported_text_extensions: text_extensions,
    }
}

pub async fn config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let cfg = state.runtime_cfg.snapshot().await;
    Json(response(
        cfg.server_name,
        cfg.registration_mode.as_str(),
        cfg.max_file_size,
        cfg.text_extensions,
    ))
}

pub async fn public_config(
    State(cfg): State<crate::db::repos::RuntimeConfig>,
) -> Json<ConfigResponse> {
    Json(response(
        cfg.server_name,
        "disabled",
        cfg.max_file_size,
        cfg.text_extensions,
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
        let state = AppState::new(pool, tmp.path().to_path_buf(), "default".into())
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
