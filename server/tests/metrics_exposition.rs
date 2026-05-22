use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, HeaderValue, Method, Request, StatusCode};
use axum::Router;
use ipnet::IpNet;
use pkv_sync_server::auth::LoginRateLimiter;
use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::{sync, vault, AppState};
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

async fn test_state() -> AppState {
    let tmp = tempfile::tempdir().unwrap();
    let db = pool::connect(&tmp.path().join("metadata.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    AppState::new(db, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap()
}

async fn test_app() -> (Router, AppState, String) {
    let data_dir = tempfile::tempdir().unwrap().keep();
    let db_path = data_dir.join("metadata.db");
    let db = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&db).await.unwrap();
    let state = AppState::new(db, data_dir.clone(), "test".into(), true)
        .await
        .unwrap();
    let key = "k_metrics_test".to_string();
    let cfg = Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:6710".parse().unwrap(),
            deployment_key: key.clone(),
            public_host: None,
        },
        storage: StorageConfig { data_dir, db_path },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
    };
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    (server::build_app(state.clone(), &cfg, limiter), state, key)
}

async fn create_token(state: &AppState, username: &str, is_admin: bool) -> String {
    let user = state
        .users
        .create(NewUser {
            username: username.into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin,
        })
        .await
        .unwrap();
    let raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: &format!("{username}-device"),
            device_name: username,
        })
        .await
        .unwrap();
    raw
}

fn request(uri: &str, key: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::USER_AGENT, "PKVSync-Plugin/0.5.0");
    if let Some(key) = key {
        builder = builder.header("x-pkvsync-deployment-key", key);
    }
    let mut req = builder.body(Body::empty()).unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));
    req
}

fn authenticated_request(uri: &str, key: &str, raw: &str) -> Request<Body> {
    let mut req = request(uri, Some(key));
    req.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {raw}")).unwrap(),
    );
    req
}

#[tokio::test]
async fn metrics_registry_encodes_expected_metric_names() {
    let state = test_state().await;
    let encoded = String::from_utf8(state.metrics.encode()).unwrap();

    for name in [
        "pkv_http_requests_total",
        "pkv_http_request_duration_seconds",
        "pkv_push_changes_total",
        "pkv_pull_files_total",
        "pkv_sse_subscribers",
        "pkv_active_tokens",
        "pkv_vaults_total",
        "pkv_blobs_total",
        "pkv_blob_gc_last_run_unix_seconds",
        "pkv_blob_gc_freed_bytes_total",
        "pkv_git_repo_size_bytes",
        "pkv_auto_merge_clean_total",
        "pkv_auto_merge_conflict_total",
    ] {
        assert!(encoded.contains(name), "missing metric {name} in {encoded}");
    }
}

#[tokio::test]
async fn metrics_endpoint_is_disabled_by_default() {
    let (app, state, key) = test_app().await;
    let admin_raw = create_token(&state, "metrics-disabled-admin", true).await;

    let resp = app
        .oneshot(authenticated_request("/metrics", &key, &admin_raw))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_endpoint_requires_deployment_key() {
    let (app, state, _key) = test_app().await;
    state
        .runtime_cfg_repo
        .set_enable_metrics(true, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;

    let resp = app.oneshot(request("/metrics", None)).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn metrics_endpoint_requires_admin_token_when_enabled() {
    let (app, state, key) = test_app().await;
    state
        .runtime_cfg_repo
        .set_enable_metrics(true, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let user_raw = create_token(&state, "metrics-user", false).await;

    let missing = app
        .clone()
        .oneshot(request("/metrics", Some(&key)))
        .await
        .unwrap();
    let forbidden = app
        .oneshot(authenticated_request("/metrics", &key, &user_raw))
        .await
        .unwrap();

    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_endpoint_scrapes_text_exposition_when_enabled() {
    let (app, state, key) = test_app().await;
    state
        .runtime_cfg_repo
        .set_enable_metrics(true, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let admin_raw = create_token(&state, "metrics-admin", true).await;

    let resp = app
        .oneshot(authenticated_request("/metrics", &key, &admin_raw))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("text/plain; version=0.0.4"),
        "unexpected content type: {content_type}"
    );
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.starts_with("# HELP"), "unexpected exposition: {body}");
    assert!(body.contains("pkv_http_requests_total"));
}

#[tokio::test]
async fn metrics_endpoint_reports_http_request_counters() {
    let (app, state, key) = test_app().await;
    state
        .runtime_cfg_repo
        .set_enable_metrics(true, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let admin_raw = create_token(&state, "metrics-counter-admin", true).await;

    let health = app
        .clone()
        .oneshot(request("/api/health", Some(&key)))
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let resp = app
        .oneshot(authenticated_request("/metrics", &key, &admin_raw))
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();

    assert!(
        body.contains(
            "pkv_http_requests_total{code=\"200\",method=\"GET\",route=\"/api/health\"} 1"
        ),
        "expected health request counter, got: {body}"
    );
}

#[tokio::test]
async fn metrics_endpoint_reports_nonzero_push_counters() {
    let (app, state, key) = test_app().await;
    state
        .runtime_cfg_repo
        .set_enable_metrics(true, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let admin_raw = create_token(&state, "metrics-push-admin", true).await;
    let user = state
        .users
        .create(NewUser {
            username: "alice".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let raw = token::generate();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "device-metrics",
            device_name: "metrics test",
        })
        .await
        .unwrap();
    let auth = AuthenticatedUser {
        user_id: user.id.clone(),
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id: token_row.device_id,
    };
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();

    sync::push(
        &state,
        &auth,
        &vault.id,
        None,
        None,
        sync::PushReq {
            device_name: Some("metrics test".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        },
    )
    .await
    .unwrap();

    let resp = app
        .oneshot(authenticated_request("/metrics", &key, &admin_raw))
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();

    assert!(
        body.contains("pkv_push_changes_total{kind=\"text\"} 1"),
        "expected nonzero text push counter, got: {body}"
    );
}

#[tokio::test]
async fn metrics_endpoint_reports_nonzero_pull_counters() {
    let (app, state, key) = test_app().await;
    state
        .runtime_cfg_repo
        .set_enable_metrics(true, None)
        .await
        .unwrap();
    let cfg = state.runtime_cfg_repo.load().await.unwrap();
    state.runtime_cfg.replace(cfg).await;
    let admin_raw = create_token(&state, "metrics-pull-admin", true).await;
    let user = state
        .users
        .create(NewUser {
            username: "bob".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let raw = token::generate();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "device-metrics-pull",
            device_name: "metrics pull test",
        })
        .await
        .unwrap();
    let auth = AuthenticatedUser {
        user_id: user.id.clone(),
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id: token_row.device_id,
    };
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
    sync::push(
        &state,
        &auth,
        &vault.id,
        None,
        None,
        sync::PushReq {
            device_name: Some("metrics pull test".into()),
            changes: vec![sync::PushChange::Text {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        },
    )
    .await
    .unwrap();

    let pulled = sync::pull(&state, &user.id, &vault.id, None).await.unwrap();
    assert_eq!(pulled.added.len(), 1);

    let resp = app
        .oneshot(authenticated_request("/metrics", &key, &admin_raw))
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();

    assert!(
        body.contains("pkv_pull_files_total{bucket=\"added\"} 1"),
        "expected nonzero added pull counter, got: {body}"
    );
}
