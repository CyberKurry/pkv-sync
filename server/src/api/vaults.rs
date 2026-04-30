use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::db::repos::VaultRepo;
use crate::service::sync::{self, UploadCheckReq};
use crate::service::{vault as vault_service, AppState};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::collections::HashMap;

pub fn router() -> Router<AppState> {
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
        .route("/api/vaults/:id/files/*path", get(read_file))
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
    Ok(Json(serde_json::to_value(v).unwrap()))
}

async fn create(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<CreateVaultReq>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let v = vault_service::create_vault(&state, &user.user_id, &req.name).await?;
    Ok((StatusCode::CREATED, Json(serde_json::to_value(v).unwrap())))
}

async fn remove(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let ok = state.vaults.delete_for_user(&user.user_id, &id).await?;
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
    let body = axum::body::to_bytes(body, max_file_size as usize)
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
    headers: HeaderMap,
    Json(req): Json<sync::PushReq>,
) -> Result<Json<sync::PushResp>, ApiError> {
    let if_match = headers.get("if-match").and_then(|h| h.to_str().ok());
    let idem = headers.get("idempotency-key").and_then(|h| h.to_str().ok());
    Ok(Json(
        sync::push(&state, &user, &id, if_match, idem, req).await?,
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
    Query(q): Query<HashMap<String, String>>,
) -> Result<Json<sync::PullResp>, ApiError> {
    Ok(Json(
        sync::pull(
            &state,
            &user.user_id,
            &id,
            q.get("since").map(String::as_str),
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
    let limit = q
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(50)
        .min(200);
    Ok(Json(
        crate::service::history::commits(&state, &user.user_id, &id, limit).await?,
    ))
}

async fn commit_detail(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path((id, commit)): Path<(String, String)>,
) -> Result<Json<crate::service::history::CommitDetail>, ApiError> {
    Ok(Json(
        crate::service::history::commit_detail(&state, &user.user_id, &id, &commit).await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::AppState;
    use crate::storage::blob::LocalFsBlobStore;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    async fn setup() -> (Router, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into())
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
                device_name: "d",
            })
            .await
            .unwrap();
        (router().with_state(state), raw)
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
        assert_eq!(body["changed_files"][0], "note.md");
    }
}
