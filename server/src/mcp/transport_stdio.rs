use crate::auth::{token, AuthenticatedUser};
use crate::db::repos::{TokenRepo, UserRepo, VaultRepo};
use crate::mcp::auth::{mcp_token_still_valid, TokenValidityCache};
use crate::mcp::tools;
use crate::service::AppState;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

pub(crate) const GENERIC_MCP_AUTH_ERROR: &str = "invalid or revoked token";
pub(crate) const GENERIC_MCP_INTERNAL_ERROR: &str = "internal server error";

pub struct StdioSession {
    state: AppState,
    user: AuthenticatedUser,
    vault_id: String,
    token_hash: String,
    validity_cache: Mutex<TokenValidityCache>,
}

impl std::fmt::Debug for StdioSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioSession")
            .field("user_id", &self.user.user_id)
            .field("vault_id", &self.vault_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

impl StdioSession {
    pub async fn authenticate(
        state: AppState,
        vault_id: String,
        token_raw: String,
    ) -> Result<Self> {
        let auth_limit_key = format!("mcp-auth:stdio:{vault_id}");
        let user = authenticate_token(&state, &token_raw, &auth_limit_key).await?;
        let token_hash = token::hash(&token_raw);
        state
            .vaults
            .find_for_user(&user.user_id, &vault_id)
            .await?
            .ok_or_else(|| anyhow!("vault not found"))?;
        Ok(Self {
            state,
            user,
            vault_id,
            token_hash,
            validity_cache: Mutex::new(TokenValidityCache::default()),
        })
    }

    pub async fn handle_jsonrpc(&self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let mut validity_cache = self.validity_cache.lock().await;
        if !mcp_token_still_valid(
            &self.state,
            &self.token_hash,
            &self.user,
            &mut validity_cache,
        )
        .await
        {
            return jsonrpc_error(id, -32000, "invalid or revoked token");
        }
        drop(validity_cache);
        handle_jsonrpc(
            &self.state,
            &self.user,
            Some(self.vault_id.as_str()),
            request,
        )
        .await
    }
}

pub async fn run(state: AppState, vault_id: String, token_raw: String) -> Result<()> {
    let session = StdioSession::authenticate(state, vault_id, token_raw).await?;
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(request) => session.handle_jsonrpc(request).await,
            Err(_) => jsonrpc_error(Value::Null, -32700, "parse error"),
        };
        stdout.write_all(response.to_string().as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    Ok(())
}

pub(crate) async fn authenticate_token(
    state: &AppState,
    token_raw: &str,
    auth_limit_key: &str,
) -> Result<AuthenticatedUser> {
    let reservation = match state.mcp_auth_limiter.try_acquire(auth_limit_key) {
        Ok(reservation) => reservation,
        Err(wait) => {
            return Err(anyhow!(
                "rate_limited: mcp auth rate limit exceeded; retry in {}s",
                wait.as_secs().max(1)
            ));
        }
    };
    match authenticate_token_inner(state, token_raw).await {
        Ok(user) => {
            reservation.success();
            Ok(user)
        }
        Err(AuthErr::Credential(err)) => {
            let _ = err;
            reservation.failure();
            Err(anyhow!(GENERIC_MCP_AUTH_ERROR))
        }
        Err(AuthErr::Internal(err)) => {
            reservation.release();
            tracing::error!(error = %err, "mcp token authentication failed internally");
            Err(anyhow!(GENERIC_MCP_AUTH_ERROR))
        }
    }
}

enum AuthErr {
    Credential(anyhow::Error),
    Internal(anyhow::Error),
}

async fn authenticate_token_inner(
    state: &AppState,
    token_raw: &str,
) -> std::result::Result<AuthenticatedUser, AuthErr> {
    if !token::looks_valid(token_raw) {
        return Err(AuthErr::Credential(anyhow!("invalid token format")));
    }
    let token_hash = token::hash(token_raw);
    let (row, user_id) = state
        .tokens
        .find_by_hash(&token_hash)
        .await
        .map_err(|err| AuthErr::Internal(anyhow!(err)))?
        .ok_or_else(|| AuthErr::Credential(anyhow!("invalid or revoked token")))?;
    let user = state
        .users
        .find_by_id(&user_id)
        .await
        .map_err(|err| AuthErr::Internal(anyhow!(err)))?
        .ok_or_else(|| AuthErr::Credential(anyhow!("user no longer exists")))?;
    if !user.is_active {
        return Err(AuthErr::Credential(anyhow!("account disabled")));
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

pub(crate) async fn handle_jsonrpc(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_scope: Option<&str>,
    request: Value,
) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return jsonrpc_error(id, -32600, "invalid request");
    };

    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "cyberkurry pkv sync",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": tools::tool_definitions()
            }
        }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match serde_json::from_value::<ToolCallParams>(params) {
                Ok(params) => match call_tool(state, user, vault_scope, params).await {
                    Ok(result) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": result
                    }),
                    Err(err) => {
                        let message = mcp_tool_error_public_message(&err);
                        jsonrpc_error(id, -32000, &message)
                    }
                },
                Err(err) => jsonrpc_error(id, -32602, &format!("invalid params: {err}")),
            }
        }
        _ => jsonrpc_error(id, -32601, "method not found"),
    }
}

async fn call_tool(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_scope: Option<&str>,
    params: ToolCallParams,
) -> Result<Value> {
    let arguments = match params.arguments {
        Value::Null => json!({}),
        other => other,
    };
    if let Some(vault_id) = scoped_argument_vault(&arguments) {
        if let Some(scope) = vault_scope {
            if vault_id != scope {
                return Err(anyhow!("vault not available in this MCP session"));
            }
        }
    }

    let structured = match params.name.as_str() {
        "list_vaults" => {
            let mut output = tools::list_vaults(state, &user.user_id).await?;
            if let Some(scope) = vault_scope {
                output.vaults.retain(|vault| vault.id == scope);
            }
            serde_json::to_value(output)?
        }
        "list_files" => serde_json::to_value(
            tools::list_files(state, &user.user_id, serde_json::from_value(arguments)?).await?,
        )?,
        "read_file" => serde_json::to_value(
            tools::read_file(state, &user.user_id, serde_json::from_value(arguments)?).await?,
        )?,
        "read_file_at_commit" => serde_json::to_value(
            tools::read_file_at_commit(state, &user.user_id, serde_json::from_value(arguments)?)
                .await?,
        )?,
        "search" => serde_json::to_value(
            tools::search(state, &user.user_id, serde_json::from_value(arguments)?).await?,
        )?,
        "link_graph" => serde_json::to_value(
            tools::link_graph(state, &user.user_id, serde_json::from_value(arguments)?).await?,
        )?,
        "changes_since" => serde_json::to_value(
            tools::changes_since(state, &user.user_id, serde_json::from_value(arguments)?).await?,
        )?,
        "write_file" => serde_json::to_value(
            tools::write_file(state, user, serde_json::from_value(arguments)?).await?,
        )?,
        "delete_file" => serde_json::to_value(
            tools::delete_file(state, user, serde_json::from_value(arguments)?).await?,
        )?,
        "write_files" => serde_json::to_value(
            tools::write_files(state, user, serde_json::from_value(arguments)?).await?,
        )?,
        "move_file" => serde_json::to_value(
            tools::move_file(state, user, serde_json::from_value(arguments)?).await?,
        )?,
        _ => return Err(anyhow!("unknown tool: {}", params.name)),
    };

    Ok(json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(&structured)?
        }],
        "structuredContent": structured,
        "isError": false
    }))
}

pub(crate) fn mcp_tool_error_public_message(error: &anyhow::Error) -> Cow<'_, str> {
    let message = error.to_string();
    if is_public_mcp_tool_error(&message) {
        Cow::Owned(message)
    } else {
        tracing::error!(error = %error, "mcp tool call failed internally");
        Cow::Borrowed(GENERIC_MCP_INTERNAL_ERROR)
    }
}

fn is_public_mcp_tool_error(message: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "batch_too_large:",
        "conflicting_path:",
        "duplicate_path:",
        "empty_batch:",
        "invalid_commit:",
        "invalid_path:",
        "not_found:",
        "path_excluded:",
        "rate_limited:",
        "target_exists:",
        "unrelated_commit:",
        "unsupported_binary_move:",
        "unknown tool:",
    ];
    const EXACT: &[&str] = &[
        "blob not referenced by vault",
        "file exceeds MCP response limit",
        "file not found",
        "search content budget exceeded",
        "vault not available in this MCP session",
        "vault not found",
    ];

    PREFIXES.iter().any(|prefix| message.starts_with(prefix))
        || EXACT.contains(&message)
        || message.starts_with("blob not found: ")
        || message.starts_with("file exceeds max_file_size of ")
        || message.starts_with("too many files to search: ")
}

fn scoped_argument_vault(arguments: &Value) -> Option<&str> {
    arguments
        .as_object()
        .and_then(|object| object.get("vault_id"))
        .and_then(Value::as_str)
}

pub(crate) fn jsonrpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool;
    use crate::db::repos::NewUser;
    use std::time::Duration;

    async fn test_state() -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap()
    }

    fn set_single_attempt_limit(state: &AppState) {
        state
            .mcp_auth_limiter
            .update_config(1, Duration::from_secs(60), Duration::from_secs(60));
    }

    fn count_occurrences(haystack: &str, needle: &str) -> usize {
        haystack.match_indices(needle).count()
    }

    #[test]
    fn mcp_transports_share_token_validity_check() {
        let http_source = include_str!("transport_http.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let stdio_source = include_str!("transport_stdio.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();

        assert_eq!(
            count_occurrences(http_source, "mcp_token_still_valid("),
            3,
            "HTTP SSE stream must call the shared MCP token validity helper"
        );
        assert_eq!(
            count_occurrences(stdio_source, "mcp_token_still_valid("),
            1,
            "stdio sessions must call the shared MCP token validity helper"
        );
        assert!(
            !http_source.contains("find_by_hash(token_hash)"),
            "HTTP transport must not reimplement token lookup"
        );
        assert!(
            !stdio_source.contains("find_by_hash(token_hash)"),
            "stdio transport must not reimplement token lookup"
        );
    }

    #[tokio::test]
    async fn authenticate_token_releases_limiter_neutrally_on_internal_error() {
        let state = test_state().await;
        set_single_attempt_limit(&state);
        state.pool.close().await;

        let err = authenticate_token(&state, &token::generate(), "internal-error-key")
            .await
            .expect_err("closed database should fail token lookup internally");
        assert!(
            !err.to_string().contains("rate_limited"),
            "first attempt should report database access failure, got {err}"
        );

        state
            .mcp_auth_limiter
            .try_acquire("internal-error-key")
            .expect("internal auth errors must release the limiter reservation neutrally");
    }

    #[tokio::test]
    async fn authenticate_token_charges_limiter_on_bad_token() {
        let state = test_state().await;
        set_single_attempt_limit(&state);

        let err = authenticate_token(&state, &token::generate(), "bad-token-key")
            .await
            .expect_err("unknown but well-formed token should be rejected");
        assert_eq!(err.to_string(), "invalid or revoked token");

        assert!(
            state.mcp_auth_limiter.try_acquire("bad-token-key").is_err(),
            "credential failures must still consume the failure budget"
        );
    }

    #[tokio::test]
    async fn tool_internal_errors_use_generic_jsonrpc_message() {
        let state = test_state().await;
        let user = state
            .users
            .create(NewUser {
                username: "mcp-internal".into(),
                password_hash: "hash".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let auth_user = AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: user.is_admin,
            token_id: "token-internal".into(),
            device_id: "device-internal".into(),
        };
        state.pool.close().await;

        let response = handle_jsonrpc(
            &state,
            &auth_user,
            None,
            json!({
                "jsonrpc": "2.0",
                "id": "internal",
                "method": "tools/call",
                "params": {
                    "name": "list_vaults",
                    "arguments": {}
                }
            }),
        )
        .await;

        assert_eq!(response["error"]["code"], -32000);
        assert_eq!(response["error"]["message"], "internal server error");
        let body = response.to_string();
        for leaked in ["database", "pool", "closed", "sqlite"] {
            assert!(
                !body.to_ascii_lowercase().contains(leaked),
                "response leaked {leaked}: {body}"
            );
        }
    }
}
