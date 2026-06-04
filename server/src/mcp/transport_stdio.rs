use crate::auth::{token, AuthenticatedUser};
use crate::db::repos::{TokenRepo, UserRepo, VaultRepo};
use crate::mcp::tools;
use crate::service::AppState;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct StdioSession {
    state: AppState,
    user: AuthenticatedUser,
    vault_id: String,
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
        state
            .vaults
            .find_for_user(&user.user_id, &vault_id)
            .await?
            .ok_or_else(|| anyhow!("vault not found"))?;
        Ok(Self {
            state,
            user,
            vault_id,
        })
    }

    pub async fn handle_jsonrpc(&self, request: Value) -> Value {
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
    if let Err(wait) = state.mcp_auth_limiter.check(auth_limit_key) {
        return Err(anyhow!(
            "rate_limited: mcp auth rate limit exceeded; retry in {}s",
            wait.as_secs().max(1)
        ));
    }
    match authenticate_token_inner(state, token_raw).await {
        Ok(user) => {
            state.mcp_auth_limiter.record_success(auth_limit_key);
            Ok(user)
        }
        Err(err) => {
            state.mcp_auth_limiter.record_failure(auth_limit_key);
            Err(err)
        }
    }
}

async fn authenticate_token_inner(state: &AppState, token_raw: &str) -> Result<AuthenticatedUser> {
    if !token::looks_valid(token_raw) {
        return Err(anyhow!("invalid token format"));
    }
    let token_hash = token::hash(token_raw);
    let (row, user_id) = state
        .tokens
        .find_by_hash(&token_hash)
        .await?
        .ok_or_else(|| anyhow!("invalid or revoked token"))?;
    let user = state
        .users
        .find_by_id(&user_id)
        .await?
        .ok_or_else(|| anyhow!("user no longer exists"))?;
    if !user.is_active {
        return Err(anyhow!("account disabled"));
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
                    Err(err) => jsonrpc_error(id, -32000, &err.to_string()),
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
        "write_file" => serde_json::to_value(
            tools::write_file(state, user, serde_json::from_value(arguments)?).await?,
        )?,
        "delete_file" => serde_json::to_value(
            tools::delete_file(state, user, serde_json::from_value(arguments)?).await?,
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
