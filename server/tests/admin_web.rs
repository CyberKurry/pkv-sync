use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, Response, StatusCode};
use axum::Router;
use ipnet::IpNet;
use pkv_sync_server::auth::token;
use pkv_sync_server::auth::{password, LoginRateLimiter};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{
    NewActivity, NewToken, NewUser, RuntimeConfigRepo, SyncActivityRepo, TokenRepo, UserRepo,
};
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
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
            deployment_key: "k_test_admin_web".into(),
            // CSRF is fail-closed when public_host is unset,
            // so admin POST tests must provide one. Use the same host the
            // test requests will use so same_origin() succeeds for valid
            // submissions.
            public_host: Some("127.0.0.1:6710".into()),
        },
        storage: StorageConfig { data_dir, db_path },
        network: NetworkConfig {
            trusted_proxies: vec!["127.0.0.1/32".parse::<IpNet>().unwrap()],
        },
        logging: LoggingConfig::default(),
    };
    let limiter = LoginRateLimiter::new(10, Duration::from_secs(900), Duration::from_secs(900));
    (server::build_app(state.clone(), &cfg, limiter), state)
}

async fn app() -> Router {
    app_with_state().await.0
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
    // Test cfg sets public_host, which forces AdminCookiePolicy.secure = true,
    // so CSRF expects an https:// expected origin. Match it here.
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

async fn first_admin_user_id(app: &Router, session_cookie: &str) -> String {
    let mut users_req = request(Method::GET, "/admin/users", Body::empty());
    users_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let users_resp = app.clone().oneshot(users_req).await.unwrap();
    assert_eq!(users_resp.status(), StatusCode::OK);
    let body = read_body(users_resp).await;
    let marker = "/admin/users/";
    let start = body.find(marker).expect("user detail link") + marker.len();
    let end = body[start..]
        .find('"')
        .map(|idx| start + idx)
        .expect("end of user detail link");
    body[start..end].to_string()
}

#[tokio::test]
async fn login_page_renders_without_api_headers() {
    let resp = app()
        .await
        .oneshot(request(Method::GET, "/admin/login", Body::empty()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert!(body.contains("PKV Sync Admin"));
}

#[tokio::test]
async fn lucide_icon_sprite_is_served_without_session() {
    let resp = app()
        .await
        .oneshot(request(
            Method::GET,
            "/admin/static/lucide-icons.svg",
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "image/svg+xml; charset=utf-8"
    );
    let body = read_body(resp).await;
    assert!(body.contains("Icons from Lucide Icons"));
    assert!(body.contains("id=\"gauge\""));
}

#[tokio::test]
async fn login_page_follows_accept_language() {
    let mut req = request(Method::GET, "/admin/login", Body::empty());
    req.headers_mut()
        .insert(header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9".parse().unwrap());
    let resp = app().await.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert!(body.contains("登录"));
    assert!(body.contains("用户名"));
}

#[tokio::test]
async fn language_switch_sets_cookie() {
    let resp = app()
        .await
        .oneshot(request(
            Method::GET,
            "/admin/language/zh-CN?next=/admin/login",
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("set-cookie")
        .to_str()
        .unwrap();
    assert!(cookie.contains("pkv_admin_lang=zh-CN"));
    assert_eq!(
        resp.headers().get(header::LOCATION).unwrap(),
        "/admin/login"
    );
}

#[tokio::test]
async fn language_switch_rejects_dot_segment_next() {
    let resp = app()
        .await
        .oneshot(request(
            Method::GET,
            "/admin/language/zh-CN?next=/admin/../api/config",
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/admin");
}

#[tokio::test]
async fn admin_can_create_device_token_and_plaintext_is_one_time() {
    let app = app().await;
    let session_cookie = login_cookie(&app).await;
    let user_id = first_admin_user_id(&app, &session_cookie).await;

    let mut create_req = request(
        Method::POST,
        &format!("/admin/users/{user_id}/tokens"),
        Body::from("device_name=desktop"),
    );
    create_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    create_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut create_req);
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = read_body(create_resp).await;
    assert!(create_body.contains("desktop"));
    assert!(create_body.contains("pks_"));

    let mut detail_req = request(
        Method::GET,
        &format!("/admin/users/{user_id}"),
        Body::empty(),
    );
    detail_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let detail_resp = app.oneshot(detail_req).await.unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let detail_body = read_body(detail_resp).await;
    assert!(detail_body.contains("desktop"));
    assert!(!detail_body.contains("pks_"));
}

#[tokio::test]
async fn admin_can_manage_device_tokens_from_devices_page() {
    let app = app().await;
    let session_cookie = login_cookie(&app).await;
    let user_id = first_admin_user_id(&app, &session_cookie).await;

    let mut create_req = request(
        Method::POST,
        "/admin/devices",
        Body::from(format!("user_id={user_id}&device_name=MacBook+Pro")),
    );
    create_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    create_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut create_req);
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let create_body = read_body(create_resp).await;
    assert!(create_body.contains("MacBook Pro"));
    assert!(create_body.contains("pks_"));

    let marker = "/admin/devices/";
    let start = create_body.find(marker).expect("device revoke action") + marker.len();
    let end = create_body[start..]
        .find("/revoke")
        .map(|idx| start + idx)
        .expect("end of token id");
    let token_id = &create_body[start..end];

    let mut revoke_req = request(
        Method::POST,
        &format!("/admin/devices/{token_id}/revoke"),
        Body::empty(),
    );
    revoke_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut revoke_req);
    let revoke_resp = app.oneshot(revoke_req).await.unwrap();
    assert_eq!(revoke_resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn admin_can_manage_vaults() {
    let app = app().await;
    let session_cookie = login_cookie(&app).await;
    let user_id = first_admin_user_id(&app, &session_cookie).await;

    let mut create_req = request(
        Method::POST,
        "/admin/vaults",
        Body::from(format!("user_id={user_id}&name=main")),
    );
    create_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    create_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut create_req);
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::SEE_OTHER);

    let mut vaults_req = request(Method::GET, "/admin/vaults", Body::empty());
    vaults_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let vaults_resp = app.clone().oneshot(vaults_req).await.unwrap();
    assert_eq!(vaults_resp.status(), StatusCode::OK);
    let vaults_body = read_body(vaults_resp).await;
    assert!(vaults_body.contains("main"));
    assert!(vaults_body.contains("admin"));

    let marker = "/admin/vaults/";
    let start = vaults_body.find(marker).expect("vault action link") + marker.len();
    let end = vaults_body[start..]
        .find('/')
        .map(|idx| start + idx)
        .expect("end of vault id");
    let vault_id = &vaults_body[start..end];

    let mut reconcile_req = request(
        Method::POST,
        &format!("/admin/vaults/{vault_id}/reconcile"),
        Body::empty(),
    );
    reconcile_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut reconcile_req);
    let reconcile_resp = app.clone().oneshot(reconcile_req).await.unwrap();
    assert_eq!(reconcile_resp.status(), StatusCode::SEE_OTHER);

    let mut delete_req = request(
        Method::POST,
        &format!("/admin/vaults/{vault_id}/delete"),
        Body::empty(),
    );
    delete_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut delete_req);
    let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::SEE_OTHER);

    let mut after_req = request(Method::GET, "/admin/vaults", Body::empty());
    after_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let after_resp = app.clone().oneshot(after_req).await.unwrap();
    let after_body = read_body(after_resp).await;
    assert!(!after_body.contains(">main<"));

    let mut activity_req = request(Method::GET, "/admin/activity", Body::empty());
    activity_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let activity_resp = app.oneshot(activity_req).await.unwrap();
    assert_eq!(activity_resp.status(), StatusCode::OK);
    let activity_body = read_body(activity_resp).await;
    assert!(activity_body.contains("create_vault"));
    assert!(activity_body.contains("delete_vault"));
    assert!(activity_body.contains(&format!("<code>{vault_id}</code>")));
}

#[tokio::test]
async fn admin_can_browse_vault_files_history_and_diff_read_only() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin_id = first_admin_user_id(&app, &session_cookie).await;
    let vault = pkv_sync_server::service::vault::create_vault(&state, &admin_id, "main")
        .await
        .unwrap();
    let store = Git2VaultStore::new(state.default_vault_root());
    let c1 = store
        .commit_changes(
            &vault.id,
            None,
            &[FileChange::Upsert {
                path: "note.md".into(),
                file: StoredFile::Text {
                    bytes: b"hello\n".to_vec(),
                },
            }],
            "sync: laptop",
        )
        .await
        .unwrap();
    let c2 = store
        .commit_changes(
            &vault.id,
            Some(&c1),
            &[FileChange::Upsert {
                path: "note.md".into(),
                file: StoredFile::Text {
                    bytes: b"hello\nworld\n".to_vec(),
                },
            }],
            "sync: laptop",
        )
        .await
        .unwrap();
    let _c3 = store
        .commit_changes(
            &vault.id,
            Some(&c2),
            &[FileChange::Upsert {
                path: "logs/history".into(),
                file: StoredFile::Text {
                    bytes: b"edge history content\n".to_vec(),
                },
            }],
            "sync: laptop",
        )
        .await
        .unwrap();

    let mut files_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/files", vault.id),
        Body::empty(),
    );
    files_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let files_resp = app.clone().oneshot(files_req).await.unwrap();
    assert_eq!(files_resp.status(), StatusCode::OK);
    let files_body = read_body(files_resp).await;
    assert!(files_body.contains("note.md"));
    assert!(files_body.contains(&format!("/admin/vaults/{}/files/note", vault.id)));

    let mut view_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/files/note.md", vault.id),
        Body::empty(),
    );
    view_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let view_resp = app.clone().oneshot(view_req).await.unwrap();
    assert_eq!(view_resp.status(), StatusCode::OK);
    let view_body = read_body(view_resp).await;
    assert!(view_body.contains("hello"));
    assert!(view_body.contains("world"));
    assert!(view_body.contains("History"));
    assert!(view_body.contains("Diff with previous"));

    let mut history_named_file_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/files/logs/history", vault.id),
        Body::empty(),
    );
    history_named_file_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let history_named_file_resp = app.clone().oneshot(history_named_file_req).await.unwrap();
    assert_eq!(history_named_file_resp.status(), StatusCode::OK);
    let history_named_file_body = read_body(history_named_file_resp).await;
    assert!(history_named_file_body.contains("edge history content"));

    let mut history_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/history/note.md", vault.id),
        Body::empty(),
    );
    history_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let history_resp = app.clone().oneshot(history_req).await.unwrap();
    assert_eq!(history_resp.status(), StatusCode::OK);
    let history_body = read_body(history_resp).await;
    assert!(history_body.contains(&c2[..7]));
    assert!(history_body.contains("View at this commit"));
    assert!(history_body.contains("Diff with previous"));
    assert!(!history_body.contains("Restore"));
    assert!(!history_body.contains("Rollback"));

    let mut diff_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/diff?path=note.md&to={}", vault.id, c2),
        Body::empty(),
    );
    diff_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let diff_resp = app.oneshot(diff_req).await.unwrap();
    assert_eq!(diff_resp.status(), StatusCode::OK);
    let diff_body = read_body(diff_resp).await;
    assert!(diff_body.contains("diff-split"));
    assert!(diff_body.contains("diff-right-cell diff-add"));
    assert!(diff_body.contains(">world<"));
    assert!(diff_body.contains("diff-add"));
    assert!(!diff_body.contains("Restore"));
    assert!(!diff_body.contains("Rollback"));

    let actions: Vec<(String,)> =
        sqlx::query_as("SELECT action FROM sync_activity WHERE vault_id = ? ORDER BY id")
            .bind(&vault.id)
            .fetch_all(&state.pool)
            .await
            .unwrap();
    let actions: Vec<String> = actions.into_iter().map(|(action,)| action).collect();
    assert!(actions.contains(&"view_commit".to_string()));
    assert!(actions.contains(&"view_history".to_string()));
    assert!(actions.contains(&"view_diff".to_string()));
}

#[tokio::test]
async fn admin_history_and_diff_routes_follow_runtime_flags() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin_id = first_admin_user_id(&app, &session_cookie).await;
    let vault = pkv_sync_server::service::vault::create_vault(&state, &admin_id, "main")
        .await
        .unwrap();
    let store = Git2VaultStore::new(state.default_vault_root());
    let c1 = store
        .commit_changes(
            &vault.id,
            None,
            &[FileChange::Upsert {
                path: "note.md".into(),
                file: StoredFile::Text {
                    bytes: b"hello\n".to_vec(),
                },
            }],
            "sync: laptop",
        )
        .await
        .unwrap();

    state
        .runtime_cfg_repo
        .set_history_flags(false, false, None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;

    let mut vaults_req = request(Method::GET, "/admin/vaults", Body::empty());
    vaults_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let vaults_resp = app.clone().oneshot(vaults_req).await.unwrap();
    assert_eq!(vaults_resp.status(), StatusCode::OK);
    let vaults_body = read_body(vaults_resp).await;
    assert!(!vaults_body.contains("Browse files"));

    for uri in [
        format!("/admin/vaults/{}/files", vault.id),
        format!("/admin/vaults/{}/files/note.md", vault.id),
        format!("/admin/vaults/{}/history/note.md", vault.id),
        format!("/admin/vaults/{}/diff?path=note.md&to={}", vault.id, c1),
    ] {
        let mut req = request(Method::GET, &uri, Body::empty());
        req.headers_mut()
            .insert(header::COOKIE, session_cookie.parse().unwrap());
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri}");
    }

    state
        .runtime_cfg_repo
        .set_history_flags(true, false, None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;

    let mut file_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/files/note.md", vault.id),
        Body::empty(),
    );
    file_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let file_resp = app.clone().oneshot(file_req).await.unwrap();
    assert_eq!(file_resp.status(), StatusCode::OK);
    let file_body = read_body(file_resp).await;
    assert!(file_body.contains("History"));
    assert!(!file_body.contains("Diff with previous"));

    let mut diff_req = request(
        Method::GET,
        &format!("/admin/vaults/{}/diff?path=note.md&to={}", vault.id, c1),
        Body::empty(),
    );
    diff_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let diff_resp = app.clone().oneshot(diff_req).await.unwrap();
    assert_eq!(diff_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn api_routes_still_require_plugin_headers() {
    let resp = app()
        .await
        .oneshot(request(Method::GET, "/api/config", Body::empty()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dashboard_requires_session() {
    let resp = app()
        .await
        .oneshot(request(Method::GET, "/admin", Body::empty()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_success_sets_cookie_and_allows_dashboard() {
    let app = app().await;
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
    let session_cookie = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let mut dashboard_req = request(Method::GET, "/admin", Body::empty());
    dashboard_req.headers_mut().insert(
        header::COOKIE,
        session_cookie.parse().expect("cookie header"),
    );
    let dashboard_resp = app.oneshot(dashboard_req).await.unwrap();
    assert_eq!(dashboard_resp.status(), StatusCode::OK);
    let body = read_body(dashboard_resp).await;
    assert!(body.contains("Dashboard"));
    assert!(body.contains("Sync Status"));
}

#[tokio::test]
async fn dashboard_header_does_not_render_inert_search() {
    let app = app().await;
    let session_cookie = login_cookie(&app).await;

    let mut dashboard_req = request(Method::GET, "/admin", Body::empty());
    dashboard_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let dashboard_resp = app.oneshot(dashboard_req).await.unwrap();
    assert_eq!(dashboard_resp.status(), StatusCode::OK);
    let body = read_body(dashboard_resp).await;

    assert!(!body.contains("Search..."));
    assert!(!body.contains("aria-hidden=\"true\">\n  <svg class=\"admin-icon\" aria-hidden=\"true\"><use href=\"/admin/static/lucide-icons.svg#search\""));
}

#[tokio::test]
async fn invites_page_accepts_human_expiry_and_shows_created_invite() {
    let app = app().await;
    let session_cookie = login_cookie(&app).await;
    let expires = (chrono::Utc::now() + chrono::Duration::days(3))
        .format("%Y-%m-%dT%H:%M")
        .to_string();

    let mut create_req = request(
        Method::POST,
        "/admin/invites",
        Body::from(format!("expires_at={}", expires.replace(':', "%3A"))),
    );
    create_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    create_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut create_req);
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::SEE_OTHER);

    let mut invites_req = request(Method::GET, "/admin/invites", Body::empty());
    invites_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let invites_resp = app.oneshot(invites_req).await.unwrap();
    assert_eq!(invites_resp.status(), StatusCode::OK);
    let body = read_body(invites_resp).await;

    assert!(body.contains("inv_"));
    assert!(body.contains("Pending"));
    assert!(body.contains("type=\"datetime-local\""));
}

#[tokio::test]
async fn users_page_search_and_status_filter_are_applied() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let bob = state
        .users
        .create(NewUser {
            username: "bob".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let alice = state
        .users
        .create(NewUser {
            username: "alice".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    state.users.set_active(&alice.id, false).await.unwrap();

    let mut req = request(
        Method::GET,
        "/admin/users?q=bo&status=active",
        Body::empty(),
    );
    req.headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;

    assert!(body.contains(&format!("/admin/users/{}", bob.id)));
    assert!(!body.contains(&format!("/admin/users/{}", alice.id)));
    assert!(body.contains("name=\"q\""));
    assert!(body.contains("value=\"bo\""));
    assert!(body.contains("<option value=\"active\" selected>Active</option>"));
}

#[tokio::test]
async fn settings_update_applies_live_login_limiter() {
    let app = app().await;
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
    let session_cookie = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let mut settings_req = request(
        Method::POST,
        "/admin/settings",
        Body::from(
            "server_name=Test&timezone=Asia%2FShanghai&registration_mode=disabled&login_failure_threshold=1&login_window_seconds=60&login_lock_seconds=60&extra_exclude_globs=&sse_heartbeat_seconds=30&push_debounce_ms=250&inline_content_max_bytes=8192",
        ),
    );
    settings_req.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    settings_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut settings_req);
    let settings_resp = app.clone().oneshot(settings_req).await.unwrap();
    assert_eq!(settings_resp.status(), StatusCode::SEE_OTHER);

    let mut bad_login = request(
        Method::POST,
        "/admin/login",
        Body::from("username=admin&password=wrongpass"),
    );
    bad_login.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    let bad_resp = app.clone().oneshot(bad_login).await.unwrap();
    assert_eq!(bad_resp.status(), StatusCode::UNAUTHORIZED);

    let mut good_login = request(
        Method::POST,
        "/admin/login",
        Body::from("username=admin&password=passw0rd%21%21"),
    );
    good_login.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/x-www-form-urlencoded".parse().unwrap(),
    );
    let locked_resp = app.oneshot(good_login).await.unwrap();
    assert_eq!(locked_resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn protected_admin_post_requires_same_origin() {
    let app = app().await;
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
    let session_cookie = login_resp
        .headers()
        .get(header::SET_COOKIE)
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let mut missing_origin = request(Method::POST, "/admin/gc", Body::empty());
    missing_origin
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let missing_origin_resp = app.clone().oneshot(missing_origin).await.unwrap();
    assert_eq!(missing_origin_resp.status(), StatusCode::FORBIDDEN);

    let mut same_origin = request(Method::POST, "/admin/gc", Body::empty());
    same_origin
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut same_origin);
    let same_origin_resp = app.oneshot(same_origin).await.unwrap();
    assert_eq!(same_origin_resp.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn activity_page_filters_by_user_and_action() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin_id = first_admin_user_id(&app, &session_cookie).await;
    let bob = state
        .users
        .create(NewUser {
            username: "bob".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    state
        .activities
        .insert(NewActivity {
            user_id: &admin_id,
            vault_id: None,
            token_id: None,
            action: "push",
            commit_hash: None,
            client_ip: Some("127.0.0.1"),
            user_agent: Some("PKVSync-Plugin/0.1.0"),
            details: None,
        })
        .await
        .unwrap();
    state
        .activities
        .insert(NewActivity {
            user_id: &bob.id,
            vault_id: None,
            token_id: None,
            action: "pull",
            commit_hash: None,
            client_ip: Some("127.0.0.2"),
            user_agent: Some("PKVSync-Plugin/0.1.0"),
            details: None,
        })
        .await
        .unwrap();

    let mut req = request(
        Method::GET,
        &format!("/admin/activity?user_id={}&action=pull", bob.id),
        Body::empty(),
    );
    req.headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert!(body.contains("<td><strong>bob</strong></td>"));
    assert!(body.contains("<span class=\"pill pill-blue\">pull</span>"));
    assert!(!body.contains("<td><strong>admin</strong></td>"));
    assert!(!body.contains("<span class=\"pill pill-blue\">push</span>"));
    assert!(body.contains(&format!(
        "<option value=\"{}\" selected>bob</option>",
        bob.id
    )));
    assert!(body.contains("<option value=\"pull\" selected>Pull</option>"));
}

#[tokio::test]
async fn activity_page_masks_client_ips_and_limits_recent_rows() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin_id = first_admin_user_id(&app, &session_cookie).await;

    for idx in 0..31 {
        let ip = match idx {
            30 => "203.0.113.42",
            29 => "2001:db8:85a3::8a2e:370:7334",
            _ => "198.51.100.9",
        };
        let user_agent = format!("PKVSync-Plugin/limited-{idx:02}");
        sqlx::query(
            "INSERT INTO sync_activity
             (user_id, action, client_ip, user_agent, timestamp)
             VALUES (?, 'push', ?, ?, ?)",
        )
        .bind(&admin_id)
        .bind(ip)
        .bind(&user_agent)
        .bind(1_700_000_000_i64 + idx)
        .execute(&state.pool)
        .await
        .unwrap();
    }

    let mut req = request(Method::GET, "/admin/activity", Body::empty());
    req.headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;

    assert_eq!(body.matches("PKVSync-Plugin/limited-").count(), 30);
    assert!(body.contains("PKVSync-Plugin/limited-30"));
    assert!(body.contains("PKVSync-Plugin/limited-29"));
    assert!(!body.contains("PKVSync-Plugin/limited-00"));
    assert!(body.contains("203.*.*.42"));
    assert!(!body.contains("203.0.113.42"));
    assert!(body.contains("2001:db8:*:*:*:*:370:7334"));
    assert!(!body.contains("2001:db8:85a3::8a2e:370:7334"));
}

#[tokio::test]
async fn user_detail_token_revoke_requires_token_to_belong_to_path_user() {
    let (app, state) = app_with_state().await;
    let session_cookie = login_cookie(&app).await;
    let admin_id = first_admin_user_id(&app, &session_cookie).await;
    let bob = state
        .users
        .create(NewUser {
            username: "bob".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let admin_token = state
        .tokens
        .create(NewToken {
            user_id: &admin_id,
            token_hash: &token::hash(&token::generate()),
            device_id: "admin-device",
            device_name: "Admin Device",
        })
        .await
        .unwrap();

    let mut revoke_req = request(
        Method::POST,
        &format!("/admin/users/{}/tokens/{}/revoke", bob.id, admin_token.id),
        Body::empty(),
    );
    revoke_req
        .headers_mut()
        .insert(header::COOKIE, session_cookie.parse().unwrap());
    set_form_origin(&mut revoke_req);
    let resp = app.oneshot(revoke_req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let tokens = state.tokens.list_for_user(&admin_id).await.unwrap();
    let still_live = tokens.iter().find(|t| t.id == admin_token.id).unwrap();
    assert!(still_live.revoked_at.is_none());
}
