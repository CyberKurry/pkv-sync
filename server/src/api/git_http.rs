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
use std::fmt;
use std::time::Duration;
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
    is_uuid_vault_id(&vault_id)?;
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

    let output = git_upload_pack_command()
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
const UPLOAD_PACK_PROCESS_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub async fn upload_pack(
    State(state): State<AppState>,
    Path(vault_id): Path<String>,
    Extension(ClientIp(client_ip)): Extension<ClientIp>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    check_enabled(&state).await?;
    is_uuid_vault_id(&vault_id)?;
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

    let mut child = git_upload_pack_command()
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
        let write_result =
            tokio::time::timeout(UPLOAD_PACK_PROCESS_TIMEOUT, stdin.write_all(&body)).await;
        match write_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "failed to write to git stdin");
            }
            Err(_) => {
                let _ = child.start_kill();
                return Err(ApiError::internal("git upload-pack timed out"));
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
        let timeout_at = tokio::time::Instant::now() + UPLOAD_PACK_PROCESS_TIMEOUT;
        let timeout = tokio::time::sleep_until(timeout_at);
        tokio::pin!(timeout);
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
                _ = &mut timeout => {
                    let _ = tx.try_send(Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "git upload-pack timed out",
                    )));
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

const GENERIC_GIT_AUTH_ERROR: &str = "invalid or revoked token";

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

/// Validate that `vault_id` is exactly 32 lowercase/uppercase hex chars.
///
/// This is intentionally stricter than the `[A-Za-z0-9_-]` rule used by the
/// storage layer and CLI (`is_valid_vault_id`): the smart-HTTP boundary only
/// ever receives server-generated UUIDs, so we reject anything that is not a
/// bare UUID to avoid exposing internal path structure.
fn is_uuid_vault_id(vault_id: &str) -> Result<(), ApiError> {
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

fn git_upload_pack_command() -> tokio::process::Command {
    let mut command = tokio::process::Command::new("git");
    command
        .arg("-c")
        .arg("uploadpack.hideRefs=refs")
        .arg("-c")
        .arg("uploadpack.hideRefs=!refs/heads/main")
        .arg("upload-pack")
        .arg("--stateless-rpc");
    command
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
        Err(BasicAuthErr::Credential) => {
            reservation.failure();
            Err(ApiError::unauthorized(GENERIC_GIT_AUTH_ERROR))
        }
        Err(BasicAuthErr::Internal(err)) => {
            reservation.release();
            tracing::error!(error = %err, "git http basic authentication failed internally");
            Err(ApiError::unauthorized(GENERIC_GIT_AUTH_ERROR))
        }
    }
}

enum BasicAuthErr {
    Credential,
    Internal(String),
}

impl fmt::Display for BasicAuthErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Credential => f.write_str("credential error"),
            Self::Internal(err) => f.write_str(err),
        }
    }
}

async fn authenticate_basic_inner(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedUser, BasicAuthErr> {
    let auth_header = headers
        .get("authorization")
        .ok_or(BasicAuthErr::Credential)?;
    let token_raw =
        git_basic::extract_token_from_basic(auth_header).ok_or(BasicAuthErr::Credential)?;
    if !token::looks_valid(&token_raw) {
        return Err(BasicAuthErr::Credential);
    }
    let h = token::hash(&token_raw);
    let (row, user_id) = state
        .tokens
        .find_by_hash(&h)
        .await
        .map_err(|err| BasicAuthErr::Internal(err.to_string()))?
        .ok_or(BasicAuthErr::Credential)?;
    let user = state
        .users
        .find_by_id(&user_id)
        .await
        .map_err(|err| BasicAuthErr::Internal(err.to_string()))?
        .ok_or(BasicAuthErr::Credential)?;
    if !user.is_active {
        return Err(BasicAuthErr::Credential);
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
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::storage::git::{FileChange, GitVaultStore, StoredFile};
    use base64::Engine;
    use git2::Oid;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    const GENERIC_AUTH_ERROR: &str = "invalid or revoked token";

    async fn test_state() -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap()
    }

    fn basic_header_for_token(raw: &str) -> HeaderMap {
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("git:{raw}"));
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Basic {encoded}").parse().unwrap());
        headers
    }

    async fn create_user_token(state: &AppState, username: &str) -> (String, String) {
        let user = state
            .users
            .create(NewUser {
                username: username.into(),
                password_hash: "hash".into(),
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
                device_id: "git-http-test",
                device_name: "Git HTTP Test",
            })
            .await
            .unwrap();
        (user.id, raw)
    }

    async fn expect_basic_auth_error(
        state: &AppState,
        headers: &HeaderMap,
        failure_key: &str,
    ) -> ApiError {
        authenticate_basic(state, headers, failure_key.to_string())
            .await
            .expect_err("basic auth should fail")
    }

    async fn enable_git_smart_http(state: &AppState) {
        let mut cfg = state.runtime_cfg.snapshot().await;
        cfg.enable_git_smart_http = true;
        state.runtime_cfg.replace(cfg).await;
    }

    #[tokio::test]
    async fn authenticate_basic_credential_failures_use_generic_message() {
        let state = test_state().await;
        let unknown_token_headers = basic_header_for_token(&token::generate());
        let invalid_format_headers = basic_header_for_token("not-a-token");
        let mut malformed_basic_headers = HeaderMap::new();
        malformed_basic_headers.insert("authorization", "Basic !!!not-base64!!!".parse().unwrap());
        let missing_headers = HeaderMap::new();

        let (deleted_user_id, deleted_user_token) =
            create_user_token(&state, "deleted-git-http").await;
        state.users.delete(&deleted_user_id).await.unwrap();
        let deleted_user_headers = basic_header_for_token(&deleted_user_token);

        let (disabled_user_id, disabled_user_token) =
            create_user_token(&state, "disabled-git-http").await;
        state
            .users
            .set_active(&disabled_user_id, false)
            .await
            .unwrap();
        let disabled_user_headers = basic_header_for_token(&disabled_user_token);

        let cases = [
            ("missing-header", &missing_headers),
            ("malformed-basic", &malformed_basic_headers),
            ("invalid-token-format", &invalid_format_headers),
            ("unknown-token", &unknown_token_headers),
            ("deleted-user", &deleted_user_headers),
            ("disabled-user", &disabled_user_headers),
        ];

        for (name, headers) in cases {
            let err = expect_basic_auth_error(&state, headers, name).await;
            assert_eq!(err.status, StatusCode::UNAUTHORIZED, "{name}");
            assert_eq!(err.code, "unauthorized", "{name}");
            assert_eq!(err.message, GENERIC_AUTH_ERROR, "{name}");
        }
    }

    #[tokio::test]
    async fn authenticate_basic_internal_errors_are_sanitized_and_release_limiter() {
        let state = test_state().await;
        state.auth_failure_limiter.update_config(
            1,
            Duration::from_secs(60),
            Duration::from_secs(60),
        );
        let headers = basic_header_for_token(&token::generate());
        state.pool.close().await;

        let err = expect_basic_auth_error(&state, &headers, "git-internal-error").await;

        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert_eq!(err.code, "unauthorized");
        assert_eq!(err.message, GENERIC_AUTH_ERROR);
        assert!(!err.message.contains("database"));
        assert!(!err.message.contains("pool"));
        state
            .auth_failure_limiter
            .try_acquire("git-internal-error")
            .expect("internal auth errors must release the limiter reservation neutrally");
    }

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

    #[tokio::test]
    async fn info_refs_advertises_only_main_ref() {
        let state = test_state().await;
        enable_git_smart_http(&state).await;
        let (user_id, raw_token) = create_user_token(&state, "git-ref-user").await;
        let vault = vault::create_vault(&state, &user_id, "main").await.unwrap();
        let git = state.git_store();
        let head = git
            .commit_changes(
                &vault.id,
                None,
                &[FileChange::Upsert {
                    path: "note.md".into(),
                    file: StoredFile::Text {
                        bytes: b"hello".to_vec(),
                    },
                }],
                "seed",
            )
            .await
            .unwrap();
        let repo = git2::Repository::open_bare(state.vault_root().join(&vault.id)).unwrap();
        repo.reference(
            "refs/heads/secret",
            Oid::from_str(&head).unwrap(),
            true,
            "test extra ref",
        )
        .unwrap();

        let response = info_refs(
            State(state),
            Path(vault.id),
            Query(InfoRefsQuery {
                service: Some("git-upload-pack".into()),
            }),
            Extension(ClientIp(IpAddr::V4(Ipv4Addr::LOCALHOST))),
            basic_header_for_token(&raw_token),
        )
        .await
        .unwrap();

        let body = axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap();
        let advertised_refs = String::from_utf8_lossy(&body);
        assert!(advertised_refs.contains("refs/heads/main"));
        assert!(!advertised_refs.contains("refs/heads/secret"));
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
    fn is_uuid_vault_id_uses_ascii_bytes_for_hex_check() {
        let source = include_str!("git_http.rs");
        let fn_start = source
            .find("fn is_uuid_vault_id")
            .expect("is_uuid_vault_id implementation exists");
        let next_doc = source[fn_start + 1..]
            .find("\n///")
            .map(|idx| fn_start + 1 + idx)
            .expect("following docs exist");
        let implementation = &source[fn_start..next_doc];

        assert!(implementation.contains("as_bytes()"));
        assert!(!implementation.contains(".chars()"));
    }

    #[test]
    fn upload_pack_has_external_process_timeout() {
        let source = include_str!("git_http.rs");
        let fn_start = source
            .find("pub async fn upload_pack")
            .expect("upload_pack implementation exists");
        let next_marker = source[fn_start..]
            .find(
                "\n// ---------------------------------------------------------------------------",
            )
            .map(|idx| fn_start + idx)
            .expect("helper marker follows upload_pack");
        let implementation = &source[fn_start..next_marker];

        assert!(implementation.contains("UPLOAD_PACK_PROCESS_TIMEOUT"));
        assert!(implementation.contains("tokio::time::sleep_until"));
        assert!(implementation.contains("child.start_kill()"));
    }
}
