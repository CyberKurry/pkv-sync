use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::db::repos::{NewActivity, SyncActivityRepo, VaultRepo};
use crate::middleware::{real_ip::ClientIp, sse_cors_allow_header_names};
use crate::service::sync::{self, UploadCheckReq};
use crate::service::{vault as vault_service, AppState};
use axum::body::Body;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// CORS layer applied only to the SSE event endpoint. The Obsidian plugin
/// has to use the native `fetch()` for SSE (Obsidian's `requestUrl` shim
/// doesn't expose a ReadableStream), and `fetch()` is subject to standard
/// browser CORS rules. Without this layer, the plugin's cross-origin
/// preflight OPTIONS request gets 405 from the router and the entire SSE
/// path falls back to polling (~30s latency). Auth still hangs on the
/// bearer device token and deployment key in headers, which CORS does not
/// weaken — so opening Origin to `*` here is safe.
fn sse_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(sse_cors_allow_header_names())
        .max_age(std::time::Duration::from_secs(86400))
}

pub fn router() -> Router<AppState> {
    // SSE endpoint gets its own sub-router so the CorsLayer wraps the entire
    // routing decision (including OPTIONS preflight). Applying CORS only via
    // .route_layer on a `get()` method router does not work because axum's
    // method router rejects OPTIONS with 405 before delegating to the layer.
    let sse_router = Router::new()
        .route("/api/vaults/:id/events", get(events))
        .layer(sse_cors_layer());
    Router::new()
        .route("/api/vaults", get(list).post(create))
        .route("/api/vaults/:id", delete(remove))
        .route("/api/vaults/:id/upload/check", post(upload_check))
        .route("/api/vaults/:id/upload/blob", post(upload_blob))
        .route("/api/vaults/:id/blobs/:hash", get(download_blob))
        .route("/api/vaults/:id/push", post(push))
        .route("/api/vaults/:id/state", get(state))
        .route("/api/vaults/:id/pull", get(pull))
        .route("/api/vaults/:id/commits", get(commits))
        .route("/api/vaults/:id/commits/:commit", get(commit_detail))
        .route("/api/vaults/:id/history", get(file_history))
        .route("/api/vaults/:id/diff", get(diff))
        .route("/api/vaults/:id/files/*path", get(read_file))
        .merge(sse_router)
}

#[derive(Deserialize)]
struct CreateVaultReq {
    name: String,
}

async fn list(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let v = state.vaults.list_for_user(&user.user_id).await?;
    Ok(Json(
        serde_json::to_value(v).map_err(|e| ApiError::internal(e.to_string()))?,
    ))
}

async fn create(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateVaultReq>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let v = vault_service::create_vault(&state, &user.user_id, &req.name).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(v).map_err(|e| ApiError::internal(e.to_string()))?),
    ))
}

async fn remove(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let ok = vault_service::delete_vault_for_user(&state, &user.user_id, &id).await?;
    if !ok {
        return Err(ApiError::not_found("vault not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn upload_check(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Json(req): Json<UploadCheckReq>,
) -> Result<Json<sync::UploadCheckResp>, ApiError> {
    Ok(Json(
        sync::upload_check(&state, &user.user_id, &id, req.blob_hashes).await?,
    ))
}

async fn upload_blob(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<StatusCode, ApiError> {
    let hash = headers
        .get("content-hash")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("missing_hash", "Content-Hash header required"))?;
    let max_file_size = state.runtime_cfg.snapshot().await.max_file_size;
    let body = axum::body::to_bytes(body, max_body_bytes(max_file_size))
        .await
        .map_err(|_| {
            ApiError::bad_request(
                "file_too_large",
                format!("file exceeds max_file_size of {max_file_size} bytes"),
            )
        })?;
    sync::upload_blob(&state, &user.user_id, &id, hash, body).await?;
    Ok(StatusCode::CREATED)
}

fn max_body_bytes(max_file_size: u64) -> usize {
    max_file_size.try_into().unwrap_or(usize::MAX)
}

async fn download_blob(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((id, hash)): Path<(String, String)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let b = sync::download_blob(&state, &user.user_id, &id, &hash)
        .await?
        .ok_or_else(|| ApiError::not_found("blob missing"))?;
    Ok((
        StatusCode::OK,
        [("content-type", "application/octet-stream")],
        b,
    ))
}

async fn push(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    client_ip: Option<Extension<ClientIp>>,
    headers: HeaderMap,
    Json(req): Json<sync::PushReq>,
) -> Result<Json<sync::PushResp>, ApiError> {
    let if_match = headers.get("if-match").and_then(|h| h.to_str().ok());
    let idem = headers.get("idempotency-key").and_then(|h| h.to_str().ok());
    let client_ip = client_ip.map(|Extension(ClientIp(ip))| ip.to_string());
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    Ok(Json(
        sync::push_with_request_metadata(
            &state,
            &user,
            &id,
            if_match,
            idem,
            sync::RequestMetadata {
                client_ip: client_ip.as_deref(),
                user_agent: user_agent.as_deref(),
            },
            req,
        )
        .await?,
    ))
}

async fn state(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<sync::StateResp>, ApiError> {
    Ok(Json(
        sync::state(
            &state,
            &user.user_id,
            &id,
            q.get("head_since").map(String::as_str),
        )
        .await?,
    ))
}

async fn pull(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    client_ip: Option<Extension<ClientIp>>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<sync::PullResp>, ApiError> {
    let client_ip = client_ip.map(|Extension(ClientIp(ip))| ip.to_string());
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    Ok(Json(
        sync::pull_with_request_metadata(
            &state,
            &user,
            &id,
            q.get("since").map(String::as_str),
            sync::RequestMetadata {
                client_ip: client_ip.as_deref(),
                user_agent: user_agent.as_deref(),
            },
        )
        .await?,
    ))
}

async fn read_file(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((id, path)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let f = sync::read_file(
        &state,
        &user.user_id,
        &id,
        &path,
        q.get("at").map(String::as_str),
    )
    .await?
    .ok_or_else(|| ApiError::not_found("file"))?;
    match f {
        crate::storage::git::StoredFile::Text { bytes } => Ok((
            StatusCode::OK,
            [("content-type", "text/plain; charset=utf-8")],
            bytes,
        )),
        crate::storage::git::StoredFile::BlobPointer { hash, .. } => {
            let b = sync::download_blob(&state, &user.user_id, &id, &hash)
                .await?
                .ok_or_else(|| ApiError::not_found("blob"))?;
            Ok((
                StatusCode::OK,
                [("content-type", "application/octet-stream")],
                b.to_vec(),
            ))
        }
    }
}

async fn commits(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::service::history::CommitSummary>>, ApiError> {
    if q.contains_key("path") && !state.runtime_cfg.snapshot().await.enable_history_ui {
        return Err(ApiError::not_found("history disabled"));
    }
    let limit = q
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
        .min(200);
    Ok(Json(
        crate::service::history::commits(
            &state,
            &user.user_id,
            &id,
            limit,
            q.get("path").map(String::as_str),
        )
        .await?,
    ))
}

async fn commit_detail(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((id, commit)): Path<(String, String)>,
    client_ip: Option<Extension<ClientIp>>,
    headers: HeaderMap,
) -> Result<Json<crate::service::history::CommitDetail>, ApiError> {
    if !state.runtime_cfg.snapshot().await.enable_history_ui {
        return Err(ApiError::not_found("history disabled"));
    }
    let out = crate::service::history::commit_detail(&state, &user.user_id, &id, &commit).await?;
    let (client_ip, user_agent) = request_metadata_parts(client_ip, &headers);
    sync::record_view(
        &state,
        &user,
        &id,
        "view_commit",
        None,
        sync::RequestMetadata {
            client_ip: client_ip.as_deref(),
            user_agent: user_agent.as_deref(),
        },
    )
    .await?;
    Ok(Json(out))
}

async fn file_history(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    client_ip: Option<Extension<ClientIp>>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::service::history::CommitSummary>>, ApiError> {
    if !state.runtime_cfg.snapshot().await.enable_history_ui {
        return Err(ApiError::not_found("history disabled"));
    }
    let path = q
        .get("path")
        .ok_or_else(|| ApiError::bad_request("missing_path", "path required"))?;
    let limit = q
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
        .min(200);
    let out =
        crate::service::history::file_history(&state, &user.user_id, &id, path, limit).await?;
    let (client_ip, user_agent) = request_metadata_parts(client_ip, &headers);
    sync::record_view(
        &state,
        &user,
        &id,
        "view_history",
        Some(path),
        sync::RequestMetadata {
            client_ip: client_ip.as_deref(),
            user_agent: user_agent.as_deref(),
        },
    )
    .await?;
    Ok(Json(out))
}

async fn diff(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    client_ip: Option<Extension<ClientIp>>,
    headers: HeaderMap,
) -> Result<Json<crate::service::diff::UnifiedDiff>, ApiError> {
    if !state.runtime_cfg.snapshot().await.enable_diff_endpoint {
        return Err(ApiError::not_found("diff disabled"));
    }
    let path = q
        .get("path")
        .ok_or_else(|| ApiError::bad_request("missing_path", "path required"))?;
    let to = q
        .get("to")
        .ok_or_else(|| ApiError::bad_request("missing_to", "to required"))?;
    let out = crate::service::diff::unified_diff(
        &state,
        &user.user_id,
        &id,
        q.get("from").map(String::as_str),
        to,
        path,
    )
    .await?;
    let (client_ip, user_agent) = request_metadata_parts(client_ip, &headers);
    sync::record_view(
        &state,
        &user,
        &id,
        "view_diff",
        Some(path),
        sync::RequestMetadata {
            client_ip: client_ip.as_deref(),
            user_agent: user_agent.as_deref(),
        },
    )
    .await?;
    Ok(Json(out))
}

async fn events(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _vault = vault_service::ensure_user_vault(&state, &user.user_id, &id).await?;

    let replay_events = match headers
        .get("last-event-id")
        .and_then(|h| h.to_str().ok())
        .filter(|h| !h.trim().is_empty())
    {
        Some(last_event_id) => crate::service::events::replay_events_after(
            state.default_vault_root(),
            &id,
            last_event_id,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?,
        None => Vec::new(),
    };

    let receiver = state.events.subscribe(&id);
    let replay_stream = tokio_stream::iter(replay_events.into_iter().filter_map(|event| {
        let id = event.commit.clone();
        Some(Ok::<Event, Infallible>(
            Event::default()
                .event("commit")
                .id(id)
                .json_data(&event)
                .ok()?,
        ))
    }));
    let live_stream = BroadcastStream::new(receiver).filter_map(|res| match res {
        Ok(event) => Some(Ok::<Event, Infallible>(
            Event::default()
                .event("commit")
                .id(event.commit.clone())
                .json_data(&event)
                .ok()?,
        )),
        Err(_lagged) => Some(Ok(Event::default().event("lagged").data(""))),
    });
    let stream = replay_stream.chain(live_stream);

    let heartbeat = state
        .runtime_cfg
        .snapshot()
        .await
        .sse_heartbeat_seconds
        .max(10);

    let sse = Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(heartbeat))
            .text(":hb"),
    );

    let mut response = sse.into_response();
    response.headers_mut().insert(
        header::HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));

    let _ = state
        .activities
        .insert(NewActivity {
            user_id: &user.user_id,
            vault_id: Some(&id),
            token_id: Some(&user.token_id),
            action: "sse_subscribed",
            commit_hash: None,
            client_ip: None,
            user_agent: None,
            details: None,
        })
        .await;

    Ok(response)
}

fn request_metadata_parts(
    client_ip: Option<Extension<ClientIp>>,
    headers: &HeaderMap,
) -> (Option<String>, Option<String>) {
    let client_ip = client_ip.map(|Extension(ClientIp(ip))| ip.to_string());
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    (client_ip, user_agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
    use crate::service::AppState;
    use crate::storage::blob::LocalFsBlobStore;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    async fn setup_with_state() -> (Router, AppState, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let h = password::hash("passw0rd!!").unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "alice".into(),
                password_hash: h,
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token::hash(&raw),
                device_id: "device-vaults",
                device_name: "d",
            })
            .await
            .unwrap();
        (router().with_state(state.clone()), state, raw)
    }

    async fn setup() -> (Router, String) {
        let (app, _state, raw) = setup_with_state().await;
        (app, raw)
    }

    async fn create_vault(app: Router, raw: &str, name: &str) -> String {
        let create = app
            .oneshot(req_json(
                "POST",
                "/api/vaults",
                raw,
                serde_json::json!({"name": name}),
            ))
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(create.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        body["id"].as_str().unwrap().to_string()
    }

    async fn push_text(
        app: Router,
        raw: &str,
        vault_id: &str,
        path: &str,
        content: &str,
        if_match: Option<&str>,
    ) -> String {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("/api/vaults/{vault_id}/push"))
            .header("authorization", format!("Bearer {raw}"))
            .header("content-type", "application/json")
            .header(
                "idempotency-key",
                format!("push-{}", uuid::Uuid::new_v4().simple()),
            );
        if let Some(head) = if_match {
            builder = builder.header("if-match", head);
        }
        let resp = app
            .oneshot(
                builder
                    .body(Body::from(
                        serde_json::json!({
                            "device_name": "test",
                            "changes": [{"kind":"text","path":path,"content":content}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        body["new_commit"].as_str().unwrap().to_string()
    }

    async fn upload_blob_bytes(app: Router, raw: &str, vault_id: &str, bytes: Vec<u8>) -> String {
        let hash = LocalFsBlobStore::sha256(&bytes);
        let upload = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/vaults/{vault_id}/upload/blob"))
                    .header("authorization", format!("Bearer {raw}"))
                    .header("content-hash", &hash)
                    .body(Body::from(bytes))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upload.status(), StatusCode::CREATED);
        hash
    }

    async fn push_blob(
        app: Router,
        raw: &str,
        vault_id: &str,
        path: &str,
        hash: &str,
        size: usize,
        if_match: Option<&str>,
    ) -> String {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("/api/vaults/{vault_id}/push"))
            .header("authorization", format!("Bearer {raw}"))
            .header("content-type", "application/json")
            .header(
                "idempotency-key",
                format!("push-{}", uuid::Uuid::new_v4().simple()),
            );
        if let Some(head) = if_match {
            builder = builder.header("if-match", head);
        }
        let resp = app
            .oneshot(
                builder
                    .body(Body::from(
                        serde_json::json!({
                            "device_name": "test",
                            "changes": [{
                                "kind":"blob",
                                "path": path,
                                "blob_hash": hash,
                                "size": size,
                                "mime": "image/png"
                            }]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        body["new_commit"].as_str().unwrap().to_string()
    }

    fn req_json(method: &str, uri: &str, raw: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {raw}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    fn auth_request(method: &str, uri: impl Into<String>, raw: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri.into())
            .header("authorization", format!("Bearer {raw}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn create_list_delete_vault() {
        let (app, raw) = setup().await;
        let create = app
            .clone()
            .oneshot(req_json(
                "POST",
                "/api/vaults",
                &raw,
                serde_json::json!({"name":"main"}),
            ))
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(create.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        let id = body["id"].as_str().unwrap().to_string();

        let list = app
            .clone()
            .oneshot(auth_request("GET", "/api/vaults", &raw))
            .await
            .unwrap();
        assert_eq!(list.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(list.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body.as_array().unwrap().len(), 1);

        let delete = app
            .oneshot(auth_request("DELETE", format!("/api/vaults/{id}"), &raw))
            .await
            .unwrap();
        assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn upload_check_and_blob_upload() {
        let (app, raw) = setup().await;
        let create = app
            .clone()
            .oneshot(req_json(
                "POST",
                "/api/vaults",
                &raw,
                serde_json::json!({"name":"main"}),
            ))
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(create.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        let id = body["id"].as_str().unwrap().to_string();
        let bytes = b"hello blob".to_vec();
        let hash = LocalFsBlobStore::sha256(&bytes);

        let check = app
            .clone()
            .oneshot(req_json(
                "POST",
                &format!("/api/vaults/{id}/upload/check"),
                &raw,
                serde_json::json!({"blob_hashes":[hash]}),
            ))
            .await
            .unwrap();
        assert_eq!(check.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(check.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["missing"].as_array().unwrap().len(), 1);

        let upload = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/vaults/{id}/upload/blob"))
                    .header("authorization", format!("Bearer {raw}"))
                    .header("content-hash", &hash)
                    .body(Body::from(bytes))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upload.status(), StatusCode::CREATED);

        let check = app
            .oneshot(req_json(
                "POST",
                &format!("/api/vaults/{id}/upload/check"),
                &raw,
                serde_json::json!({"blob_hashes":[hash]}),
            ))
            .await
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(check.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["missing"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn push_text_change() {
        let (app, raw) = setup().await;
        let create = app
            .clone()
            .oneshot(req_json(
                "POST",
                "/api/vaults",
                &raw,
                serde_json::json!({"name":"main"}),
            ))
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(create.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        let id = body["id"].as_str().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/vaults/{id}/push"))
                    .header("authorization", format!("Bearer {raw}"))
                    .header("content-type", "application/json")
                    .header("idempotency-key", "push-text-1")
                    .body(Body::from(
                        serde_json::json!({
                            "device_name": "test",
                            "changes": [{"kind":"text","path":"note.md","content":"hello"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        let commit = body["new_commit"].as_str().unwrap().to_string();
        assert_eq!(body["files_changed"], 1);

        let state = app
            .clone()
            .oneshot(auth_request("GET", format!("/api/vaults/{id}/state"), &raw))
            .await
            .unwrap();
        assert_eq!(state.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(state.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["current_head"], commit);

        let pull = app
            .clone()
            .oneshot(auth_request("GET", format!("/api/vaults/{id}/pull"), &raw))
            .await
            .unwrap();
        assert_eq!(pull.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(pull.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["added"][0]["path"], "note.md");
        assert_eq!(body["added"][0]["content_inline"], "hello");

        let file = app
            .clone()
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/files/note.md"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(file.status(), StatusCode::OK);
        let body = axum::body::to_bytes(file.into_body(), 4096).await.unwrap();
        assert_eq!(body.as_ref(), b"hello");

        let commits = app
            .clone()
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/commits"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(commits.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(commits.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body[0]["commit"], commit);

        let detail = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/commits/{commit}"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(detail.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(detail.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(body.get("changed_files").is_none());
        assert_eq!(body["changes"][0]["path"], "note.md");
        assert_eq!(body["changes"][0]["change_type"], "added");
    }

    #[tokio::test]
    async fn commit_detail_returns_parent_diff_changes() {
        let (app, raw) = setup().await;
        let id = create_vault(app.clone(), &raw, "main").await;
        let first = push_text(app.clone(), &raw, &id, "note.md", "hello", None).await;
        let second = push_text(
            app.clone(),
            &raw,
            &id,
            "note.md",
            "hello\nworld\n",
            Some(&first),
        )
        .await;

        let detail = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/commits/{second}"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(detail.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(detail.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(body.get("changed_files").is_none());
        assert_eq!(body["parent"], first);
        assert_eq!(body["changes"][0]["path"], "note.md");
        assert_eq!(body["changes"][0]["change_type"], "modified");
        assert_eq!(body["changes"][0]["binary"], false);
    }

    #[tokio::test]
    async fn file_history_endpoint_tracks_only_requested_path() {
        let (app, raw) = setup().await;
        let id = create_vault(app.clone(), &raw, "main").await;
        let first = push_text(app.clone(), &raw, &id, "note.md", "v1\n", None).await;
        let second = push_text(app.clone(), &raw, &id, "note.md", "v2\n", Some(&first)).await;
        let _third = push_text(app.clone(), &raw, &id, "other.md", "other\n", Some(&second)).await;

        let history = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/history?path=note.md&limit=10"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(history.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(history.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        let rows = body.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["commit"], second);
        assert_eq!(rows[1]["commit"], first);
    }

    #[tokio::test]
    async fn diff_endpoint_returns_unified_patch_for_text_file() {
        let (app, raw) = setup().await;
        let id = create_vault(app.clone(), &raw, "main").await;
        let first = push_text(app.clone(), &raw, &id, "note.md", "hello\n", None).await;
        let second = push_text(
            app.clone(),
            &raw,
            &id,
            "note.md",
            "hello\nworld\n",
            Some(&first),
        )
        .await;

        let diff = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/diff?to={second}&path=note.md"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(diff.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(diff.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["from"], first);
        assert_eq!(body["to"], second);
        assert_eq!(body["path"], "note.md");
        assert_eq!(body["binary"], false);
        assert_eq!(body["truncated"], false);
        assert!(body["patch"].as_str().unwrap().contains("+world"));
    }

    #[tokio::test]
    async fn diff_endpoint_marks_blob_pointer_changes_as_binary() {
        let (app, raw) = setup().await;
        let id = create_vault(app.clone(), &raw, "main").await;
        let first_bytes = vec![1, 2, 3, 4];
        let first_hash = upload_blob_bytes(app.clone(), &raw, &id, first_bytes.clone()).await;
        let first = push_blob(
            app.clone(),
            &raw,
            &id,
            "image.png",
            &first_hash,
            first_bytes.len(),
            None,
        )
        .await;
        let second_bytes = vec![9, 8, 7, 6];
        let second_hash = upload_blob_bytes(app.clone(), &raw, &id, second_bytes.clone()).await;
        let second = push_blob(
            app.clone(),
            &raw,
            &id,
            "image.png",
            &second_hash,
            second_bytes.len(),
            Some(&first),
        )
        .await;

        let diff = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/diff?to={second}&path=image.png"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(diff.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(diff.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["binary"], true);
        assert_eq!(body["patch"], "");
    }

    #[tokio::test]
    async fn history_and_diff_feature_flags_return_404_when_disabled() {
        let (app, state, raw) = setup_with_state().await;
        state
            .runtime_cfg_repo
            .set_history_flags(false, false, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;
        let id = create_vault(app.clone(), &raw, "main").await;
        let head = push_text(app.clone(), &raw, &id, "note.md", "hello", None).await;

        let history = app
            .clone()
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/history?path=note.md"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(history.status(), StatusCode::NOT_FOUND);

        let commits_path = app
            .clone()
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/commits?path=note.md"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(commits_path.status(), StatusCode::NOT_FOUND);

        let commits = app
            .clone()
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/commits"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(commits.status(), StatusCode::OK);

        let diff = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/diff?to={head}&path=note.md"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(diff.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn history_endpoint_records_view_activity() {
        let (app, state, raw) = setup_with_state().await;
        let id = create_vault(app.clone(), &raw, "main").await;
        let _head = push_text(app.clone(), &raw, &id, "note.md", "hello", None).await;

        let history = app
            .oneshot(auth_request(
                "GET",
                format!("/api/vaults/{id}/history?path=note.md"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(history.status(), StatusCode::OK);

        let row: (String, String) = sqlx::query_as(
            "SELECT action, details FROM sync_activity WHERE vault_id = ? AND action = 'view_history'",
        )
        .bind(&id)
        .fetch_one(&state.pool)
        .await
        .unwrap();
        assert_eq!(row.0, "view_history");
        let details: serde_json::Value = serde_json::from_str(&row.1).unwrap();
        assert_eq!(details["path"], "note.md");
    }

    #[tokio::test]
    async fn push_records_request_metadata_from_handler() {
        let (app, state, raw) = setup_with_state().await;
        let create = app
            .clone()
            .oneshot(req_json(
                "POST",
                "/api/vaults",
                &raw,
                serde_json::json!({"name":"main"}),
            ))
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(create.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        let id = body["id"].as_str().unwrap();

        let resp = app
            .clone()
            .layer(axum::extract::Extension(ClientIp(
                "203.0.113.12".parse().unwrap(),
            )))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/vaults/{id}/push"))
                    .header("authorization", format!("Bearer {raw}"))
                    .header("content-type", "application/json")
                    .header("user-agent", "PKVSync-Plugin/0.1.0")
                    .body(Body::from(
                        serde_json::json!({
                            "device_name": "test",
                            "changes": [{"kind":"text","path":"note.md","content":"hello"}]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let row: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT client_ip, user_agent FROM sync_activity WHERE vault_id = ?")
                .bind(id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(row.0.as_deref(), Some("203.0.113.12"));
        assert_eq!(row.1.as_deref(), Some("PKVSync-Plugin/0.1.0"));
    }
}
