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
use crate::service::{vault, AppState};
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

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
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    check_enabled(&state).await?;
    let user = authenticate_basic(&state, &headers).await?;
    let _vault = vault::ensure_user_vault(&state, &user.user_id, &vault_id).await?;

    let service = query.service.as_deref().unwrap_or("");
    if service != "git-upload-pack" {
        return Err(ApiError::bad_request(
            "invalid_service",
            "only git-upload-pack is supported",
        ));
    }

    let repo_path = state.default_vault_root().join(&vault_id);
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
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    check_enabled(&state).await?;
    let user = authenticate_basic(&state, &headers).await?;
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

    let repo_path = state.default_vault_root().join(&vault_id);
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
        .spawn()
        .map_err(|e| {
            tracing::error!(error = %e, "failed to spawn git upload-pack");
            ApiError::internal("failed to run git")
        })?;

    // Write request body to stdin
    if let Some(mut stdin) = child.stdin.take() {
        // Use a spawn to avoid blocking the async runtime on the stdin write
        let write_result = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(&body).await
        })
        .await;
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "failed to write to git stdin");
            }
            Err(e) => {
                tracing::warn!(error = %e, "stdin write task panicked");
            }
        }
        // stdin is dropped here, which closes the pipe and signals EOF to git
    }

    let output = child.wait_with_output().await.map_err(|e| {
        tracing::error!(error = %e, "failed to wait for git upload-pack");
        ApiError::internal("git upload-pack failed")
    })?;

    if !output.status.success() {
        tracing::warn!(
            exit = output.status.code(),
            stderr = %String::from_utf8_lossy(&output.stderr),
            "git upload-pack --stateless-rpc failed"
        );
        return Err(ApiError::internal("git upload-pack failed"));
    }

    Ok((
        StatusCode::OK,
        [("content-type", "application/x-git-upload-pack-result")],
        output.stdout,
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

/// Authenticate a request using the Basic auth header.
///
/// Unlike the `AuthenticatedUser` extractor (which uses Bearer tokens), Git
/// clients send credentials via HTTP Basic auth. The username is ignored;
/// only the password (the device token) matters.
async fn authenticate_basic(
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
        return Err(ApiError::forbidden("account disabled"));
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
    out.extend_from_slice(format!("{len:04x}").as_bytes());
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
}
