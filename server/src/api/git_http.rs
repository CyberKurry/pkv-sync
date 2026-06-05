//! Git smart HTTP protocol handlers (read-only).
//!
//! Exposes each vault as a read-only Git repository over HTTPS using the
//! standard Git smart HTTP transport. Only `git-upload-pack` (clone/fetch)
//! is supported — no push over HTTP.

use crate::api::error::ApiError;
use crate::auth::git_basic;
use crate::auth::token;
use crate::auth::AuthenticatedUser;
use crate::db::repos::{TokenRepo, UserRepo};
use crate::middleware::real_ip::ClientIp;
use crate::service::{vault, AppState};
use axum::body::{Body, Bytes};
use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_stream::wrappers::ReceiverStream;

// ---------------------------------------------------------------------------
// Request query structs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct InfoRefsQuery {
    pub service: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /git/:vault_id/info/refs?service=git-upload-pack`
///
/// Returns the ref advertisement that Git clients use to discover what the
/// server has before negotiating a pack.
pub async fn info_refs(
    State(state): State<AppState>,
    Path(vault_id): Path<String>,
    Query(query): Query<InfoRefsQuery>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    check_enabled(&state).await?;
    validate_vault_id(&vault_id)?;
    let user = authenticate_basic(&state, &headers, client_ip.to_string()).await?;
    let _vault = vault::ensure_user_vault(&state, &user.user_id, &vault_id).await?;

    let service = query.service.as_deref().unwrap_or("");
    if service != "git-upload-pack" {
        return Err(ApiError::bad_request(
            "invalid_service",
            "only git-upload-pack is supported",
        ));
    }

    let repo_path = state.vault_root().join(&vault_id);
    if !repo_path.exists() {
        return Err(ApiError::not_found("vault repository not found"));
    }

    let output = tokio::process::Command::new("git")
        .arg("upload-pack")
        .arg("--stateless-rpc")
        .arg("--advertise-refs")
        .arg(&repo_path)
        .output()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to spawn git upload-pack");
            ApiError::internal("failed to run git")
        })?;

    if !output.status.success() {
        tracing::warn!(
            exit = output.status.code(),
            "git upload-pack --advertise-refs failed"
        );
        return Err(ApiError::internal("git upload-pack failed"));
    }

    // Build pkt-line header: "# service=git-upload-pack\n" wrapped in pkt-line + flush
    let service_line = b"# service=git-upload-pack\n";
    let pkt_header = pkt_line(service_line);
    let flush = b"0000";

    let mut body = Vec::with_capacity(pkt_header.len() + flush.len() + output.stdout.len());
    body.extend_from_slice(&pkt_header);
    body.extend_from_slice(flush);
    body.extend_from_slice(&output.stdout);

    Ok((
        StatusCode::OK,
        [
            (
                "content-type",
                "application/x-git-upload-pack-advertisement",
            ),
            ("cache-control", "no-cache"),
        ],
        body,
    )
        .into_response())
}

/// `POST /git/:vault_id/git-upload-pack`
///
/// Stateless-RPC endpoint for the actual pack negotiation. The client sends
/// its wants/haves and the server responds with a packfile.
/// Upper bound on a single `git-upload-pack` request body. Real-world
/// negotiation requests are tens of KB even for very large repos because the
/// body only contains `want`/`have` ref lists, not pack data. 10 MiB is a wide
/// margin; anything over it is almost certainly hostile or malformed and we
/// reject before allocating it.
const MAX_UPLOAD_PACK_BODY_BYTES: usize = 10 * 1024 * 1024;

pub async fn upload_pack(
    State(state): State<AppState>,
    Path(vault_id): Path<String>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    check_enabled(&state).await?;
    validate_vault_id(&vault_id)?;
    let user = authenticate_basic(&state, &headers, client_ip.to_string()).await?;
    let _vault = vault::ensure_user_vault(&state, &user.user_id, &vault_id).await?;
    if body.len() > MAX_UPLOAD_PACK_BODY_BYTES {
        return Err(ApiError::bad_request(
            "upload_pack_body_too_large",
            format!(
                "git-upload-pack request body exceeds {} bytes",
                MAX_UPLOAD_PACK_BODY_BYTES
            ),
        ));
    }

    let repo_path = state.vault_root().join(&vault_id);
    if !repo_path.exists() {
        return Err(ApiError::not_found("vault repository not found"));
    }

    let mut child = tokio::process::Command::new("git")
        .arg("upload-pack")
        .arg("--stateless-rpc")
        .arg(&repo_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to spawn git upload-pack");
            ApiError::internal("failed to run git")
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        let write_result = tokio::spawn(async move { stdin.write_all(&body).await }).await;
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "failed to write to git stdin");
            }
            Err(e) => {
                tracing::warn!(error = %e, "stdin write task panicked");
            }
        }
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ApiError::internal("git upload-pack stdout unavailable"))?;
    let stderr = child.stderr.take();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        let mut stdout = stdout;
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_end(&mut buf).await;
            }
            buf
        });
        let mut buf = vec![0_u8; 16 * 1024];
        let mut should_kill = false;
        loop {
            tokio::select! {
                read = stdout.read(&mut buf) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            if tx.send(Ok(Bytes::copy_from_slice(&buf[..n]))).await.is_err() {
                                should_kill = true;
                                break;
                            }
                        }
                        Err(err) => {
                            let _ = tx.send(Err(err)).await;
                            should_kill = true;
                            break;
                        }
                    }
                }
                _ = tx.closed() => {
                    should_kill = true;
                    break;
                }
            }
        }
        if should_kill {
            let _ = child.start_kill();
        }
        let wait_result = child.wait().await;
        let stderr = stderr_task.await.unwrap_or_default();
        match wait_result {
            Ok(status) if status.success() => {}
            Ok(status) => {
                tracing::warn!(
                    exit = status.code(),
                    stderr = %String::from_utf8_lossy(&stderr),
                    "git upload-pack --stateless-rpc failed"
                );
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to wait for git upload-pack");
            }
        }
    });

    Ok((
        StatusCode::OK,
        [("content-type", "application/x-git-upload-pack-result")],
        Body::from_stream(ReceiverStream::new(rx)),
    )
        .into_response())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check that the git smart HTTP feature is enabled both at the binary level
/// (git binary available) and the runtime config level.
async fn check_enabled(state: &AppState) -> Result<(), ApiError> {
    if !state.git_available {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "git_unavailable",
            "git binary not found on server",
        ));
    }
    let cfg = state.runtime_cfg.snapshot().await;
    if !cfg.enable_git_smart_http {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "git_disabled",
            "git smart HTTP is not enabled",
        ));
    }
    Ok(())
}

fn validate_vault_id(vault_id: &str) -> Result<(), ApiError> {
    let is_simple_uuid =
        vault_id.len() == 32 && vault_id.as_bytes().iter().all(u8::is_ascii_hexdigit);
    if is_simple_uuid {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "invalid_vault_id",
            "invalid vault id",
        ))
    }
}

/// Authenticate a request using the Basic auth header.
///
/// Unlike the `AuthenticatedUser` extractor (which uses Bearer tokens), Git
/// clients send credentials via HTTP Basic auth. The username is ignored;
/// only the password (the device token) matters.
async fn authenticate_basic(
    state: &AppState,
    headers: &HeaderMap,
    failure_key: String,
) -> Result<AuthenticatedUser, ApiError> {
    let reservation = match state.auth_failure_limiter.try_acquire(&failure_key) {
        Ok(reservation) => reservation,
        Err(wait) => {
            return Err(ApiError::too_many(format!(
                "too many failed authentication attempts, retry in {}s",
                wait.as_secs().max(1)
            )));
        }
    };

    match authenticate_basic_inner(state, headers).await {
        Ok(user) => {
            reservation.success();
            Ok(user)
        }
        Err(err) => {
            if err.status == StatusCode::UNAUTHORIZED {
                reservation.failure();
            } else {
                reservation.release();
            }
            Err(err)
        }
    }
}

async fn authenticate_basic_inner(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedUser, ApiError> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| ApiError::unauthorized("missing authorization"))?;
    let token_raw = git_basic::extract_token_from_basic(auth_header)
        .ok_or_else(|| ApiError::unauthorized("invalid basic auth"))?;
    if !token::looks_valid(&token_raw) {
        return Err(ApiError::unauthorized("invalid token format"));
    }
    let h = token::hash(&token_raw);
    let (row, user_id) = state
        .tokens
        .find_by_hash(&h)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invalid or revoked token"))?;
    let user = state
        .users
        .find_by_id(&user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user no longer exists"))?;
    if !user.is_active {
        return Err(ApiError::unauthorized("invalid or revoked token"));
    }
    let _ = state
        .tokens
        .touch_used(&row.id, chrono::Utc::now().timestamp())
        .await;
    Ok(AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        token_id: row.id,
        device_id: row.device_id,
    })
}

/// Encode `data` as a Git pkt-line: 4-hex-digit length prefix (including the
/// 4 bytes of the prefix itself) followed by the data.
fn pkt_line(data: &[u8]) -> Vec<u8> {
    let len = 4 + data.len();
    let mut out = Vec::with_capacity(len);
    let hex = b"0123456789abcdef";
    out.push(hex[(len >> 12) & 0xf]);
    out.push(hex[(len >> 8) & 0xf]);
    out.push(hex[(len >> 4) & 0xf]);
    out.push(hex[len & 0xf]);
    out.extend_from_slice(data);
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkt_line_encodes_service_header() {
        let encoded = pkt_line(b"# service=git-upload-pack\n");
        // Length = 4 + 26 = 30 = 0x001e
        let expected_prefix = b"001e# service=git-upload-pack\n";
        assert_eq!(&encoded, expected_prefix);
    }

    #[test]
    fn pkt_line_handles_empty_data() {
        let encoded = pkt_line(b"");
        assert_eq!(&encoded, b"0004");
    }

    #[test]
    fn pkt_line_handles_short_data() {
        let encoded = pkt_line(b"A");
        assert_eq!(&encoded, b"0005A");
    }

    #[test]
    fn pkt_line_does_not_allocate_prefix_string() {
        let source = include_str!("git_http.rs");
        let fn_start = source.find("fn pkt_line").expect("pkt_line exists");
        let next_marker = source[fn_start..]
            .find(
                "\n// ---------------------------------------------------------------------------",
            )
            .map(|idx| fn_start + idx)
            .expect("test marker follows pkt_line");
        let implementation = &source[fn_start..next_marker];

        assert!(
            !implementation.contains("format!"),
            "pkt_line should write the fixed hex prefix without format! allocation"
        );
    }

    #[test]
    fn validate_vault_id_uses_ascii_bytes_for_hex_check() {
        let source = include_str!("git_http.rs");
        let fn_start = source
            .find("fn validate_vault_id")
            .expect("validate_vault_id implementation exists");
        let next_doc = source[fn_start + 1..]
            .find("\n///")
            .map(|idx| fn_start + 1 + idx)
            .expect("following docs exist");
        let implementation = &source[fn_start..next_doc];

        assert!(implementation.contains("as_bytes()"));
        assert!(!implementation.contains(".chars()"));
    }
}
