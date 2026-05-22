use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, Response, StatusCode};
use axum::Router;
use ipnet::IpNet;
use pkv_sync_server::auth::LoginRateLimiter;
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewUser, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

const STRONG_PASSWORD: &str = "ThisIsAReallyGoodPassw0rd";

async fn fresh_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let pool = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&pool).await.unwrap();
    let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), false)
        .await
        .unwrap();
    (state, tmp)
}

fn app_with_state(state: AppState, data_dir: std::path::PathBuf) -> Router {
    let cfg = Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:6710".parse().unwrap(),
            deployment_key: "k_test_setup".into(),
            public_host: Some("127.0.0.1:6710".into()),
        },
        storage: StorageConfig {
            data_dir,
            db_path: std::path::PathBuf::from("metadata.db"),
        },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
    };
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    server::build_app(state, &cfg, limiter)
}

fn request(method: Method, uri: &str, body: Body) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:6710")
        .body(body)
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));
    req
}

fn form_request(uri: &str, body: impl Into<String>) -> Request<Body> {
    let mut req = request(Method::POST, uri, Body::from(body.into()));
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    req.headers_mut()
        .insert(header::ORIGIN, "https://127.0.0.1:6710".parse().unwrap());
    req
}

async fn read_body(resp: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), 32 * 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn setup_wizard_creates_first_admin_and_seals() {
    let (state, tmp) = fresh_state().await;
    let app = app_with_state(state.clone(), tmp.path().to_path_buf());

    let setup = app
        .clone()
        .oneshot(request(Method::GET, "/setup", Body::empty()))
        .await
        .unwrap();
    assert_eq!(setup.status(), StatusCode::OK);
    let body = read_body(setup).await;
    assert!(body.contains("Initial Setup"));

    let create = app
        .clone()
        .oneshot(form_request(
            "/setup",
            format!("username=newadmin&password={STRONG_PASSWORD}&confirm={STRONG_PASSWORD}"),
        ))
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        create.headers().get(header::LOCATION).unwrap(),
        "/admin/login?setup=complete&u=newadmin"
    );
    assert_eq!(state.users.count_admins().await.unwrap(), 1);
    assert!(!state.is_setup_pending().await);

    let sealed = app
        .clone()
        .oneshot(request(Method::GET, "/setup", Body::empty()))
        .await
        .unwrap();
    assert_eq!(sealed.status(), StatusCode::NOT_FOUND);

    let login = app
        .oneshot(form_request(
            "/admin/login",
            format!("username=newadmin&password={STRONG_PASSWORD}"),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn api_returns_setup_required_before_deployment_key_when_no_admin_exists() {
    let (state, tmp) = fresh_state().await;
    let app = app_with_state(state, tmp.path().to_path_buf());

    let resp = app
        .oneshot(request(Method::GET, "/api/config", Body::empty()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = read_body(resp).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"]["code"], "setup_required");
}

#[tokio::test]
async fn admin_routes_point_pending_setup_to_wizard() {
    let (state, tmp) = fresh_state().await;
    let app = app_with_state(state.clone(), tmp.path().to_path_buf());

    let admin = app
        .clone()
        .oneshot(request(Method::GET, "/admin", Body::empty()))
        .await
        .unwrap();
    assert_eq!(admin.status(), StatusCode::SEE_OTHER);
    assert_eq!(admin.headers().get(header::LOCATION).unwrap(), "/setup");

    let login = app
        .oneshot(form_request(
            "/admin/login",
            "username=admin&password=anything",
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(state.users.count_admins().await.unwrap(), 0);
    let body = read_body(login).await;
    assert!(body.contains("first-run setup"));
    assert!(body.contains("href=\"/setup\""));
}

#[tokio::test]
async fn setup_post_requires_same_origin() {
    let (state, tmp) = fresh_state().await;
    let app = app_with_state(state, tmp.path().to_path_buf());
    let mut req = request(
        Method::POST,
        "/setup",
        Body::from(format!(
            "username=newadmin&password={STRONG_PASSWORD}&confirm={STRONG_PASSWORD}"
        )),
    );
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );

    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn setup_rejects_weak_password_without_creating_admin() {
    let (state, tmp) = fresh_state().await;
    let app = app_with_state(state.clone(), tmp.path().to_path_buf());

    let resp = app
        .oneshot(form_request(
            "/setup",
            "username=newadmin&password=weakpass&confirm=weakpass",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(state.users.count_admins().await.unwrap(), 0);
    let body = read_body(resp).await;
    assert!(body.contains("12 characters"));
}

#[tokio::test]
async fn existing_admin_seals_setup_immediately() {
    let (state, tmp) = fresh_state().await;
    state
        .users
        .create(NewUser {
            username: "admin".into(),
            password_hash: pkv_sync_server::auth::password::hash(STRONG_PASSWORD).unwrap(),
            is_admin: true,
        })
        .await
        .unwrap();
    state.mark_setup_complete().await;
    let app = app_with_state(state.clone(), tmp.path().to_path_buf());

    let setup = app
        .oneshot(request(Method::GET, "/setup", Body::empty()))
        .await
        .unwrap();

    assert_eq!(setup.status(), StatusCode::NOT_FOUND);
    assert!(!state.is_setup_pending().await);
}
