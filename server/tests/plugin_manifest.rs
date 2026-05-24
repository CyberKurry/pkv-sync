use axum::body::Body;
use axum::extract::Extension;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use pkv_sync_server::api::plugin_manifest::PluginAssetOrigin;
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::service::AppState;
use pkv_sync_server::{api, auth};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

async fn app_and_token() -> (Router, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let pool = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&pool).await.unwrap();
    let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    let user = state
        .users
        .create(NewUser {
            username: "cyberkurry".into(),
            password_hash: "h".into(),
            is_admin: false,
        })
        .await
        .unwrap();
    let raw = auth::token::generate();
    state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &auth::token::hash(&raw),
            device_id: "device-plugin-manifest",
            device_name: "test device",
        })
        .await
        .unwrap();
    (api::router().with_state(state), raw, tmp)
}

fn auth_get(uri: &str, raw: &str, host: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {raw}"))
        .header(header::HOST, host)
        .body(Body::empty())
        .unwrap()
}

fn auth_get_with_forwarded(uri: &str, raw: &str, host: &str, proto: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {raw}"))
        .header(header::HOST, host)
        .header("x-forwarded-proto", proto)
        .body(Body::empty())
        .unwrap()
}

async fn response_bytes(resp: axum::response::Response) -> bytes::Bytes {
    axum::body::to_bytes(resp.into_body(), 256 * 1024)
        .await
        .unwrap()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[tokio::test]
async fn plugin_manifest_advertises_downloadable_assets_with_matching_hashes() {
    let (app, raw, _tmp) = app_and_token().await;
    let host = "sync.example.test";

    let manifest_resp = app
        .clone()
        .oneshot(auth_get("/api/plugin-manifest", &raw, host))
        .await
        .unwrap();
    assert_eq!(manifest_resp.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(&response_bytes(manifest_resp).await)
        .expect("manifest response json");
    // The bundled plugin manifest is kept in lockstep with the workspace
    // version by the release process, so the served version must match
    // CARGO_PKG_VERSION. Past releases had this hard-coded which forced an
    // edit in every chore(release) commit (and broke v0.8.4 when missed).
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        body["main_js_url"],
        "http://sync.example.test/api/plugin-assets/main.js"
    );
    assert_eq!(
        body["manifest_json_url"],
        "http://sync.example.test/api/plugin-assets/manifest.json"
    );
    assert_eq!(
        body["styles_css_url"],
        "http://sync.example.test/api/plugin-assets/styles.css"
    );

    let main_resp = app
        .clone()
        .oneshot(auth_get("/api/plugin-assets/main.js", &raw, host))
        .await
        .unwrap();
    assert_eq!(main_resp.status(), StatusCode::OK);
    assert_eq!(
        main_resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/javascript"
    );
    let main_bytes = response_bytes(main_resp).await;
    assert_eq!(body["main_js_sha256"], sha256_hex(&main_bytes));

    let manifest_asset_resp = app
        .clone()
        .oneshot(auth_get("/api/plugin-assets/manifest.json", &raw, host))
        .await
        .unwrap();
    assert_eq!(manifest_asset_resp.status(), StatusCode::OK);
    assert_eq!(
        manifest_asset_resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap(),
        "application/json"
    );
    let manifest_asset_bytes = response_bytes(manifest_asset_resp).await;
    assert_eq!(
        body["manifest_json_sha256"],
        sha256_hex(&manifest_asset_bytes)
    );

    let styles_resp = app
        .oneshot(auth_get("/api/plugin-assets/styles.css", &raw, host))
        .await
        .unwrap();
    assert_eq!(styles_resp.status(), StatusCode::OK);
    assert_eq!(
        styles_resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/css"
    );
    let styles_bytes = response_bytes(styles_resp).await;
    assert_eq!(body["styles_css_sha256"], sha256_hex(&styles_bytes));
}

#[tokio::test]
async fn plugin_manifest_uses_configured_public_host_over_request_headers() {
    let (app, raw, _tmp) = app_and_token().await;
    let app = app.layer(Extension(PluginAssetOrigin::from_public_host(Some(
        "sync.example.test".into(),
    ))));

    let manifest_resp = app
        .oneshot(auth_get_with_forwarded(
            "/api/plugin-manifest",
            &raw,
            "attacker.example.test",
            "http",
        ))
        .await
        .unwrap();
    assert_eq!(manifest_resp.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(&response_bytes(manifest_resp).await)
        .expect("manifest response json");
    assert_eq!(
        body["main_js_url"],
        "https://sync.example.test/api/plugin-assets/main.js"
    );
    assert_eq!(
        body["manifest_json_url"],
        "https://sync.example.test/api/plugin-assets/manifest.json"
    );
    assert_eq!(
        body["styles_css_url"],
        "https://sync.example.test/api/plugin-assets/styles.css"
    );
}

#[tokio::test]
async fn plugin_manifest_ignores_untrusted_forwarded_proto_without_public_host() {
    let (app, raw, _tmp) = app_and_token().await;

    let manifest_resp = app
        .oneshot(auth_get_with_forwarded(
            "/api/plugin-manifest",
            &raw,
            "sync.example.test",
            "https",
        ))
        .await
        .unwrap();
    assert_eq!(manifest_resp.status(), StatusCode::OK);

    let body: serde_json::Value = serde_json::from_slice(&response_bytes(manifest_resp).await)
        .expect("manifest response json");
    assert_eq!(
        body["main_js_url"],
        "http://sync.example.test/api/plugin-assets/main.js"
    );
}
