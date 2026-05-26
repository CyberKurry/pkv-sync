use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, Response, StatusCode};
use axum::Router;
use ipnet::IpNet;
use pkv_sync_server::admin::i18n::AdminText;
use pkv_sync_server::auth::{password, LoginRateLimiter};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewUser, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::{vault, vault_settings, AppState};
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

async fn app_with_state() -> (Router, AppState) {
    let data_dir = tempfile::tempdir().unwrap().keep();
    let db_path = data_dir.join("metadata.db");
    let pool = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&pool).await.unwrap();
    let state = AppState::new(pool, data_dir.clone(), "test".into(), false)
        .await
        .unwrap();
    state
        .users
        .create(NewUser {
            username: "admin".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: true,
        })
        .await
        .unwrap();
    let cfg = Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:6710".parse().unwrap(),
            deployment_key: "k_test_admin_vault_settings".into(),
            public_host: Some("127.0.0.1:6710".into()),
        },
        storage: StorageConfig { data_dir, db_path },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
        update_check: pkv_sync_server::config::UpdateCheckConfig {
            enabled: false,
            ..Default::default()
        },
    };
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    (server::build_app(state.clone(), &cfg, limiter), state)
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

async fn read_body(resp: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), 32 * 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn set_form_origin(req: &mut Request<Body>) {
    req.headers_mut()
        .insert(header::ORIGIN, "https://127.0.0.1:6710".parse().unwrap());
}

async fn login_cookie(app: &Router) -> String {
    let mut login_req = request(
        Method::POST,
        "/admin/login",
        Body::from("username=admin&password=passw0rd%21%21"),
    );
    login_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    let login_resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(login_resp.status(), StatusCode::SEE_OTHER);
    login_resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn with_session(session_cookie: &str, method: Method, uri: &str, body: Body) -> Request<Body> {
    let mut req = request(method, uri, body);
    req.headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    req
}

fn form_body(extra_sync_globs: &str, apply_starter: bool) -> String {
    let encoded = extra_sync_globs
        .replace('%', "%25")
        .replace('\n', "%0A")
        .replace('/', "%2F")
        .replace('*', "%2A")
        .replace(' ', "+");
    if apply_starter {
        format!("extra_sync_globs={encoded}&apply_starter=1")
    } else {
        format!("extra_sync_globs={encoded}")
    }
}

#[tokio::test]
async fn get_vault_settings_renders_current_extra_sync_globs() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin = state
        .users
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap();
    let vault = vault::create_vault(&state, &admin.id, "main")
        .await
        .unwrap();
    vault_settings::save(
        &state,
        &vault.id,
        &vault_settings::VaultSettings {
            extra_sync_globs: vec!["notes/**".into(), ".obsidian/app.json".into()],
        },
    )
    .await
    .unwrap();

    let req = with_session(
        &session_cookie,
        Method::GET,
        &format!("/admin/vaults/{}/settings", vault.id),
        Body::empty(),
    );
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert!(body.contains(AdminText::en().vault_settings));
    assert!(body.contains("main"));
    assert!(body.contains("notes/**"));
    assert!(body.contains(".obsidian/app.json"));
    assert!(body.contains(&format!("/admin/vaults/{}/settings", vault.id)));
}

#[tokio::test]
async fn post_vault_settings_updates_extra_sync_globs_and_redirects() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin = state
        .users
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap();
    let vault = vault::create_vault(&state, &admin.id, "main")
        .await
        .unwrap();

    let mut req = with_session(
        &session_cookie,
        Method::POST,
        &format!("/admin/vaults/{}/settings", vault.id),
        Body::from(form_body("notes/**\n.obsidian/snippets/**", false)),
    );
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    set_form_origin(&mut req);
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        &format!("/admin/vaults/{}/settings", vault.id)
    );
    let settings = vault_settings::load(&state, &vault.id).await.unwrap();
    assert_eq!(
        settings.extra_sync_globs,
        vec!["notes/**".to_string(), ".obsidian/snippets/**".to_string()]
    );
}

#[tokio::test]
async fn post_vault_settings_apply_starter_writes_exact_starter_allowlist() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin = state
        .users
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap();
    let vault = vault::create_vault(&state, &admin.id, "main")
        .await
        .unwrap();
    vault_settings::save(
        &state,
        &vault.id,
        &vault_settings::VaultSettings {
            extra_sync_globs: vec!["custom/**".into()],
        },
    )
    .await
    .unwrap();

    let mut req = with_session(
        &session_cookie,
        Method::POST,
        &format!("/admin/vaults/{}/settings", vault.id),
        Body::from(form_body("custom/**", true)),
    );
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    set_form_origin(&mut req);
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let settings = vault_settings::load(&state, &vault.id).await.unwrap();
    assert_eq!(
        settings.extra_sync_globs,
        vec![
            ".obsidian/themes/**".to_string(),
            ".obsidian/snippets/**".to_string(),
            ".obsidian/hotkeys.json".to_string(),
            ".obsidian/app.json".to_string(),
            ".obsidian/appearance.json".to_string(),
            ".obsidian/community-plugins.json".to_string(),
            ".obsidian/core-plugins.json".to_string(),
        ]
    );
}

#[tokio::test]
async fn post_vault_settings_rejects_invalid_glob_without_saving() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin = state
        .users
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap();
    let vault = vault::create_vault(&state, &admin.id, "main")
        .await
        .unwrap();

    let mut req = with_session(
        &session_cookie,
        Method::POST,
        &format!("/admin/vaults/{}/settings", vault.id),
        Body::from(form_body("[", false)),
    );
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    set_form_origin(&mut req);
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let settings = vault_settings::load(&state, &vault.id).await.unwrap();
    assert_eq!(
        settings.extra_sync_globs,
        vault_settings::starter_extra_sync_globs()
    );
}
