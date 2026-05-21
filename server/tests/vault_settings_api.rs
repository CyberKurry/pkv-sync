use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo, VaultSettingsRepo};
use pkv_sync_server::service::{vault, vault_settings, AppState};
use pkv_sync_server::{api, auth};
use tower::ServiceExt;

fn expected_starter_extra_sync_globs() -> Vec<String> {
    [
        ".obsidian/themes/**",
        ".obsidian/snippets/**",
        ".obsidian/hotkeys.json",
        ".obsidian/app.json",
        ".obsidian/appearance.json",
        ".obsidian/community-plugins.json",
        ".obsidian/core-plugins.json",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

async fn state_and_user() -> (AppState, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
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
    (state, user.id, tmp)
}

async fn create_user_with_token(state: &AppState, username: &str) -> (String, String) {
    let user = state
        .users
        .create(NewUser {
            username: username.into(),
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
            device_id: &format!("device-{username}"),
            device_name: "test device",
        })
        .await
        .unwrap();
    (user.id, raw)
}

async fn app_and_users() -> (Router, AppState, String, String, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    let (owner_id, owner_token) = create_user_with_token(&state, "owner").await;
    let (_other_id, other_token) = create_user_with_token(&state, "other").await;
    let app = api::router().with_state(state.clone());
    (app, state, owner_id, owner_token, other_token, tmp)
}

fn auth_request(method: &str, uri: impl Into<String>, raw: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri.into())
        .header("authorization", format!("Bearer {raw}"))
        .body(Body::empty())
        .unwrap()
}

fn auth_json_request(
    method: &str,
    uri: impl Into<String>,
    raw: &str,
    body: serde_json::Value,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri.into())
        .header("authorization", format!("Bearer {raw}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn new_vault_creation_loads_starter_allowlist() {
    let (state, user_id, _tmp) = state_and_user().await;

    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    let settings = vault_settings::load(&state, &vault.id).await.unwrap();

    assert_eq!(
        settings.extra_sync_globs,
        expected_starter_extra_sync_globs()
    );
}

#[tokio::test]
async fn get_vault_settings_returns_current_settings_for_owner() {
    let (app, state, owner_id, owner_token, _other_token, _tmp) = app_and_users().await;
    let vault = vault::create_vault(&state, &owner_id, "main")
        .await
        .unwrap();

    let response = app
        .oneshot(auth_request(
            "GET",
            format!("/api/vaults/{}/settings", vault.id),
            &owner_token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        body["extra_sync_globs"],
        serde_json::json!(expected_starter_extra_sync_globs())
    );
}

#[tokio::test]
async fn put_vault_settings_saves_settings_and_returns_no_content() {
    let (app, state, owner_id, owner_token, _other_token, _tmp) = app_and_users().await;
    let vault = vault::create_vault(&state, &owner_id, "main")
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(auth_json_request(
            "PUT",
            format!("/api/vaults/{}/settings", vault.id),
            &owner_token,
            serde_json::json!({"extra_sync_globs":["notes/**",".obsidian/plugins/foo/**"]}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap()
            .len(),
        0
    );

    let response = app
        .oneshot(auth_request(
            "GET",
            format!("/api/vaults/{}/settings", vault.id),
            &owner_token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        body["extra_sync_globs"],
        serde_json::json!(["notes/**", ".obsidian/plugins/foo/**"])
    );
}

#[tokio::test]
async fn vault_settings_routes_hide_cross_user_vaults() {
    let (app, state, owner_id, _owner_token, other_token, _tmp) = app_and_users().await;
    let vault = vault::create_vault(&state, &owner_id, "main")
        .await
        .unwrap();

    let get_response = app
        .clone()
        .oneshot(auth_request(
            "GET",
            format!("/api/vaults/{}/settings", vault.id),
            &other_token,
        ))
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);

    let put_response = app
        .oneshot(auth_json_request(
            "PUT",
            format!("/api/vaults/{}/settings", vault.id),
            &other_token,
            serde_json::json!({"extra_sync_globs":["notes/**"]}),
        ))
        .await
        .unwrap();
    assert_eq!(put_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn put_vault_settings_rejects_invalid_glob() {
    let (app, state, owner_id, owner_token, _other_token, _tmp) = app_and_users().await;
    let vault = vault::create_vault(&state, &owner_id, "main")
        .await
        .unwrap();

    let response = app
        .oneshot(auth_json_request(
            "PUT",
            format!("/api/vaults/{}/settings", vault.id),
            &owner_token,
            serde_json::json!({"extra_sync_globs":["["]}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(body["error"]["code"], "invalid_glob");
}

#[tokio::test]
async fn vault_settings_set_and_load_round_trips() {
    let (state, user_id, _tmp) = state_and_user().await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();

    state
        .vault_settings
        .set(&vault.id, "extra_sync_globs", r#"["notes/**"]"#)
        .await
        .unwrap();

    let loaded = state.vault_settings.load_for_vault(&vault.id).await.unwrap();
    assert_eq!(
        loaded.get("extra_sync_globs"),
        Some(&r#"["notes/**"]"#.to_string())
    );
}

#[tokio::test]
async fn vault_delete_cascades_settings_cleanup() {
    let (state, user_id, _tmp) = state_and_user().await;
    let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
    vault_settings::save(
        &state,
        &vault.id,
        &vault_settings::VaultSettings {
            extra_sync_globs: vec!["notes/**".into()],
        },
    )
    .await
    .unwrap();

    assert!(state
        .vault_settings
        .load_for_vault(&vault.id)
        .await
        .unwrap()
        .contains_key("extra_sync_globs"));
    assert!(vault::delete_vault_for_user(&state, &user_id, &vault.id)
        .await
        .unwrap());

    assert!(state
        .vault_settings
        .load_for_vault(&vault.id)
        .await
        .unwrap()
        .is_empty());
}
