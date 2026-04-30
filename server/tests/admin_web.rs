use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, Response, StatusCode};
use axum::Router;
use ipnet::IpNet;
use pkv_sync_server::auth::{password, LoginRateLimiter};
use pkv_sync_server::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewUser, UserRepo};
use pkv_sync_server::server;
use pkv_sync_server::service::AppState;
use std::net::SocketAddr;
use std::time::Duration;
use tower::ServiceExt;

async fn app() -> Router {
    let data_dir = tempfile::tempdir().unwrap().keep();
    let db_path = data_dir.join("metadata.db");
    let pool = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&pool).await.unwrap();
    let state = AppState::new(pool, data_dir.clone(), "test".into())
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
            public_host: None,
        },
        storage: StorageConfig { data_dir, db_path },
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

async fn read_body(resp: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), 32 * 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn set_form_origin(req: &mut Request<Body>) {
    req.headers_mut()
        .insert(header::ORIGIN, "http://127.0.0.1:6710".parse().unwrap());
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
    let after_resp = app.oneshot(after_req).await.unwrap();
    let after_body = read_body(after_resp).await;
    assert!(!after_body.contains(">main<"));
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
    assert!(body.contains("Run Blob GC"));
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
            "server_name=Test&registration_mode=disabled&login_failure_threshold=1&login_window_seconds=60&login_lock_seconds=60",
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
