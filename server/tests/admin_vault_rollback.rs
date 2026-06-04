use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, Response, StatusCode};
use axum::Router;
use ipnet::IpNet;
use pkv_sync_server::auth::{password, token, AuthenticatedUser, LoginRateLimiter};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::sync::{push, PushChange, PushReq};
use pkv_sync_server::service::{vault, AppState};
use pkv_sync_server::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};
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
            deployment_key: "k_test_admin_vault_rollback".into(),
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
        mcp: Default::default(),
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

async fn login_csrf(app: &Router) -> (String, String) {
    let page_resp = app
        .clone()
        .oneshot(request(Method::GET, "/admin/login", Body::empty()))
        .await
        .unwrap();
    assert_eq!(page_resp.status(), StatusCode::OK);
    let csrf_cookie = page_resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("login csrf cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let body = read_body(page_resp).await;
    let marker = "name=\"login_csrf\" type=\"hidden\" value=\"";
    let start = body.find(marker).expect("login csrf hidden input") + marker.len();
    let end = body[start..].find('"').expect("login csrf value end");
    (body[start..start + end].to_string(), csrf_cookie)
}

async fn login_cookie(app: &Router) -> String {
    let (csrf, csrf_cookie) = login_csrf(app).await;
    let mut login_req = request(
        Method::POST,
        "/admin/login",
        Body::from(format!(
            "username=admin&password=passw0rd%21%21&login_csrf={csrf}"
        )),
    );
    login_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    login_req
        .headers_mut()
        .insert(header::COOKIE, csrf_cookie.parse().unwrap());
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

async fn create_auth_user(state: &AppState, username: &str, is_admin: bool) -> AuthenticatedUser {
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
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: &format!("device-{username}"),
            device_name: username,
        })
        .await
        .unwrap();
    AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin,
        token_id: token_row.id,
        device_id: token_row.device_id,
    }
}

async fn push_text(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    parent: Option<&str>,
    content: &str,
) -> String {
    push(
        state,
        user,
        vault_id,
        parent,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "note.md".into(),
                content: content.into(),
            }],
        },
    )
    .await
    .unwrap()
    .new_commit
}

#[tokio::test]
async fn admin_post_rollback_moves_head_records_activity_and_redirects_to_history() {
    let (app, state) = app_with_state().await;
    let owner = create_auth_user(&state, "owner", false).await;
    let vault = vault::create_vault(&state, &owner.user_id, "main")
        .await
        .unwrap();
    let first = push_text(&state, &owner, &vault.id, None, "v1").await;
    let second = push_text(&state, &owner, &vault.id, Some(&first), "v2").await;
    let session_cookie = login_cookie(&app).await;

    let history_req = with_session(
        &session_cookie,
        Method::GET,
        &format!("/admin/vaults/{}/history/note.md", vault.id),
        Body::empty(),
    );
    let history_resp = app.clone().oneshot(history_req).await.unwrap();
    assert_eq!(history_resp.status(), StatusCode::OK);
    let history_body = read_body(history_resp).await;
    assert!(history_body.contains(&format!("action=\"/admin/vaults/{}/rollback\"", vault.id)));
    assert!(history_body.contains(&format!("name=\"commit\" value=\"{first}\"")));
    assert!(history_body.contains("Rollback"));

    let mut rollback_req = with_session(
        &session_cookie,
        Method::POST,
        &format!("/admin/vaults/{}/rollback", vault.id),
        Body::from(format!("commit={first}&path=note.md")),
    );
    rollback_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    set_form_origin(&mut rollback_req);
    let rollback_resp = app.clone().oneshot(rollback_req).await.unwrap();

    assert_eq!(rollback_resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        rollback_resp.headers().get(header::LOCATION).unwrap(),
        &format!("/admin/vaults/{}/history/note.md", vault.id)
    );
    let git = Git2VaultStore::new(state.default_vault_root());
    assert_eq!(
        git.head(&vault.id).await.unwrap().as_deref(),
        Some(first.as_str())
    );
    let file = git
        .read_file(&vault.id, "note.md", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        file,
        StoredFile::Text {
            bytes: b"v1".to_vec()
        }
    );

    let (action, commit_hash, token_id, details): (String, String, Option<String>, String) =
        sqlx::query_as(
            "SELECT action, commit_hash, token_id, details
         FROM sync_activity WHERE vault_id = ? AND action = 'vault_rollback'",
        )
        .bind(&vault.id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
    let (admin_web_tokens,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM tokens WHERE device_id LIKE 'admin-web-%'")
            .fetch_one(&state.pool)
            .await
            .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(action, "vault_rollback");
    assert_eq!(commit_hash, first);
    assert!(token_id.is_none());
    assert_eq!(admin_web_tokens, 0);
    assert_eq!(details["from_commit"], second);
    assert_eq!(details["to_commit"], first);
}

#[tokio::test]
async fn rollback_post_rejects_missing_or_non_admin_session() {
    let (app, state) = app_with_state().await;
    let owner = create_auth_user(&state, "owner", false).await;
    let vault = vault::create_vault(&state, &owner.user_id, "main")
        .await
        .unwrap();
    let first = push_text(&state, &owner, &vault.id, None, "v1").await;

    let mut missing_session_req = request(
        Method::POST,
        &format!("/admin/vaults/{}/rollback", vault.id),
        Body::from(format!("commit={first}&path=note.md")),
    );
    missing_session_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    set_form_origin(&mut missing_session_req);
    let missing_session_resp = app.clone().oneshot(missing_session_req).await.unwrap();
    assert_eq!(missing_session_resp.status(), StatusCode::UNAUTHORIZED);

    let non_admin = state
        .users
        .create(NewUser {
            username: "plain".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let session_id = pkv_sync_server::admin::session::create_session(&state, &non_admin.id)
        .await
        .unwrap();
    let mut non_admin_req = request(
        Method::POST,
        &format!("/admin/vaults/{}/rollback", vault.id),
        Body::from(format!("commit={first}&path=note.md")),
    );
    non_admin_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    non_admin_req.headers_mut().insert(
        header::COOKIE,
        format!(
            "{}={session_id}",
            pkv_sync_server::admin::session::COOKIE_NAME
        )
        .parse()
        .unwrap(),
    );
    set_form_origin(&mut non_admin_req);
    let non_admin_resp = app.clone().oneshot(non_admin_req).await.unwrap();
    assert_eq!(non_admin_resp.status(), StatusCode::UNAUTHORIZED);

    let git = Git2VaultStore::new(state.default_vault_root());
    assert_eq!(
        git.head(&vault.id).await.unwrap().as_deref(),
        Some(first.as_str())
    );
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sync_activity WHERE vault_id = ? AND action = 'vault_rollback'",
    )
    .bind(&vault.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}
