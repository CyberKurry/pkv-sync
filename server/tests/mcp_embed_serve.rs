use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use ipnet::IpNet;
use pkv_sync_server::auth::{password, token, LoginRateLimiter};
use pkv_sync_server::config::{
    Config, LoggingConfig, McpConfig, NetworkConfig, ServerConfig, StorageConfig,
};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

const DEPLOYMENT_KEY: &str = "k_mcp_embed_test";

async fn app(embed_in_serve: bool) -> (axum::Router, AppState, String) {
    let data_dir = tempfile::tempdir().unwrap().keep();
    let db_path = data_dir.join("metadata.db");
    let pool = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&pool).await.unwrap();
    let state = AppState::new(pool, data_dir.clone(), "test".into(), true)
        .await
        .unwrap();
    let user = state
        .users
        .create(NewUser {
            username: "mcp-user".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: true,
        })
        .await
        .unwrap();
    let raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "mcp-embed-device",
            device_name: "MCP Embed",
        })
        .await
        .unwrap();
    let cfg = Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:6710".parse().unwrap(),
            deployment_key: DEPLOYMENT_KEY.into(),
            public_host: None,
        },
        storage: StorageConfig { data_dir, db_path },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
        update_check: Default::default(),
        mcp: McpConfig { embed_in_serve },
    };
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    (server::build_app(state.clone(), &cfg, limiter), state, raw)
}

fn with_connect_info(mut request: Request<Body>) -> Request<Body> {
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:12345".parse::<SocketAddr>().unwrap(),
    ));
    request
}

fn mcp_post(raw: &str, deployment_key: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {raw}"));
    if let Some(deployment_key) = deployment_key {
        builder = builder.header("x-pkvsync-deployment-key", deployment_key);
    }
    with_connect_info(
        builder
            .body(Body::from(
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize"
                })
                .to_string(),
            ))
            .unwrap(),
    )
}

#[tokio::test]
async fn serve_router_omits_mcp_when_embed_disabled() {
    let (app, _state, raw) = app(false).await;

    let response = app
        .oneshot(mcp_post(&raw, Some(DEPLOYMENT_KEY)))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn serve_router_mounts_mcp_when_embed_enabled() {
    let (app, _state, raw) = app(true).await;

    let response = app
        .oneshot(mcp_post(&raw, Some(DEPLOYMENT_KEY)))
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::OK);
    assert!(headers.get("mcp-session-id").is_none());
    assert_eq!(json["result"]["serverInfo"]["name"], "cyberkurry pkv sync");
}

#[tokio::test]
async fn embedded_mcp_rejects_missing_deployment_key() {
    let (app, _state, raw) = app(true).await;

    let response = app.oneshot(mcp_post(&raw, None)).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn embedded_mcp_rejects_admin_cookie_without_bearer() {
    let (app, _state, _raw) = app(true).await;
    let request = with_connect_info(
        Request::builder()
            .method(Method::POST)
            .uri("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
            .header(header::COOKIE, "pkv_admin_session=pretend-admin-session")
            .body(Body::from(
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize"
                })
                .to_string(),
            ))
            .unwrap(),
    );

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["message"], "missing bearer token");
}

#[tokio::test]
async fn embedded_mcp_sse_keeps_streamable_http_headers_behavior() {
    let (app, _state, raw) = app(true).await;
    let request = with_connect_info(
        Request::builder()
            .method(Method::GET)
            .uri("/mcp")
            .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
            .header(header::AUTHORIZATION, format!("Bearer {raw}"))
            .header(header::ACCEPT, "text/event-stream")
            .body(Body::empty())
            .unwrap(),
    );

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream")));
}
