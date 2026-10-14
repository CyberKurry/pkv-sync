use crate::auth::{token, AuthenticatedUser};
use crate::db::repos::{TokenRepo, UserRepo, VaultRepo};
use crate::mcp::auth::{mcp_token_validity, TokenValidity, TokenValidityCache};
use crate::mcp::protocol::Era;
use crate::mcp::tools;
use crate::service::events::VaultEvent;
use crate::service::AppState;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub(crate) const GENERIC_MCP_AUTH_ERROR: &str = "invalid or revoked token";
pub(crate) const GENERIC_MCP_INTERNAL_ERROR: &str = "internal server error";
/// Marker for a non-object `tools/call` `arguments` value; mapped to
/// `-32602 invalid params` by both era dispatchers (L-5).
const INVALID_TOOL_ARGUMENTS: &str = "invalid params: arguments must be an object";

const LISTEN_DRAIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// How long a stdio session may keep its listen subscriptions while the token
/// check cannot reach the database (R3-8), expressed in drain ticks: the run
/// loop drives `drain_listens` once per `LISTEN_DRAIN_INTERVAL`, so 600 ticks
/// is about 30 seconds.
///
/// R2-3 stopped one database hiccup from permanently silencing a *valid*
/// session by keeping the subscriptions and retrying. Kept unbounded, that
/// same tolerance became an open-ended fail-open: any `sqlx::Error` — down to
/// a self-healing `database is locked` — let a session that is in fact
/// revoked keep every subscription alive for as long as the error lasted.
/// Counting ticks rather than reading a clock makes the bound independent of
/// machine speed and directly testable; a stalled run loop stretches the
/// wall-clock equivalent, which is harmless because nothing is being served
/// while it is stalled.
const TRANSIENT_TOKEN_MAX_TICKS: u32 = 600;

pub struct StdioSession {
    state: AppState,
    user: AuthenticatedUser,
    vault_id: String,
    token_hash: String,
    validity_cache: Mutex<TokenValidityCache>,
    listens: Mutex<HashMap<String, ListenEntry>>,
    /// One-shot report latch for a transient token-revalidation outage: the
    /// drain tick runs every 50ms, so a database outage must not log on every
    /// tick. Set on the transition into a transient streak, cleared the next
    /// time the token check answers.
    transient_token_check_reported: std::sync::atomic::AtomicBool,
    /// `Transient` answers seen back to back since the last real answer
    /// (R3-8), bounded by `transient_token_max_ticks`.
    transient_token_ticks: std::sync::atomic::AtomicU32,
    /// One-shot latch for the "tolerance exceeded" report. The terminal path
    /// stays reachable on every later tick while the store is still
    /// unreachable, and `transient_token_check_reported` is already spent by
    /// then, so without this the warning would repeat every 50ms.
    transient_token_tolerance_reported: std::sync::atomic::AtomicBool,
    /// Tolerance for `transient_token_ticks`; overridable in tests.
    transient_token_max_ticks: std::sync::atomic::AtomicU32,
}

struct ListenEntry {
    vault_id: String,
    id: Value,
    receiver: tokio::sync::broadcast::Receiver<VaultEvent>,
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
        // The limiter runs before the token lookup, so the only available
        // key material is the vault id plus a per-process factor. In
        // practice this limiter is inert on the stdio path: `pkvsyncd mcp`
        // builds a fresh AppState (and thus a fresh limiter instance) per
        // process, and `authenticate` runs exactly once per process, so
        // this key sees a single `try_acquire` no matter what it contains.
        // The vault dimension is kept so a future in-process reuse of the
        // limiter across sessions keys correctly, and the pid suffix keeps
        // process instances from sharing a bucket if that ever happens.
        let auth_limit_key = format!("mcp-auth:stdio:{vault_id}:{}", std::process::id());
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
            listens: Mutex::new(HashMap::new()),
            transient_token_check_reported: std::sync::atomic::AtomicBool::new(false),
            transient_token_ticks: std::sync::atomic::AtomicU32::new(0),
            transient_token_tolerance_reported: std::sync::atomic::AtomicBool::new(false),
            transient_token_max_ticks: std::sync::atomic::AtomicU32::new(TRANSIENT_TOKEN_MAX_TICKS),
        })
    }

    /// Test seam (R3-8): shrink the transient tolerance so a test can reach
    /// the terminal transition without draining six hundred times.
    pub fn set_transient_token_max_ticks_for_tests(&self, ticks: u32) {
        self.transient_token_max_ticks
            .store(ticks, std::sync::atomic::Ordering::Release);
    }

    /// Recheck the session token (revocation / expiry / disabled account)
    /// against the shared 5s validity cache (R-4). Called by every entry
    /// point that does work on the session's behalf — request handling and
    /// the listen drain — since a registered listen outlives its request
    /// and would otherwise keep pushing to a revoked session until EOF.
    async fn token_still_valid(&self) -> bool {
        matches!(self.token_validity().await, TokenValidity::Valid)
    }

    /// Forgets the current transient streak: the token check produced an
    /// answer, so the next outage opens its own tolerance window (R3-8).
    fn clear_transient_token_streak(&self) {
        self.transient_token_ticks
            .store(0, std::sync::atomic::Ordering::Release);
        self.transient_token_check_reported
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.transient_token_tolerance_reported
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Same recheck, but with the transient/terminal distinction the drain
    /// needs (R2-3).
    async fn token_validity(&self) -> TokenValidity {
        let mut validity_cache = self.validity_cache.lock().await;
        mcp_token_validity(
            &self.state,
            &self.token_hash,
            &self.user,
            &mut validity_cache,
        )
        .await
    }

    pub async fn handle_jsonrpc(&self, request: Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        if !self.token_still_valid().await {
            return jsonrpc_error(id, -32000, "invalid or revoked token");
        }
        handle_jsonrpc(
            &self.state,
            &self.user,
            Some(self.vault_id.as_str()),
            request,
        )
        .await
    }

    /// Handle one inbound stdio line. Requests are answered with exactly one
    /// line via `tx`; notifications are answered with silence (the stdio
    /// equivalent of HTTP's 202), except `notifications/cancelled`, which
    /// removes a registered listen.
    pub async fn pump(&self, line: &str, tx: &mut mpsc::Sender<String>) {
        if line.trim().is_empty() {
            return;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(msg) => msg,
            Err(_) => {
                let _ = tx
                    .send(jsonrpc_error(Value::Null, -32700, "parse error").to_string())
                    .await;
                return;
            }
        };
        if msg.is_array() {
            // JSON-RPC batch (an array of requests). Batches are not
            // supported, and silence here would leave a client waiting
            // forever for responses that never come (L-4) — answer the
            // line with a single invalid-request error instead. HTTP answers
            // the same shape with the byte-identical `-32600` line (R3-7),
            // wrapped in the legacy 200 envelope because an array can never
            // carry `params._meta` (R3-6); the two transports agree on the
            // protocol-level answer even though stdio has no status code to
            // lean on.
            let _ = tx
                .send(
                    jsonrpc_error(
                        Value::Null,
                        -32600,
                        "invalid request: batch requests are not supported",
                    )
                    .to_string(),
                )
                .await;
            return;
        }
        if msg.get("id").is_none() {
            if msg.get("method").and_then(Value::as_str) == Some("notifications/cancelled") {
                if let Some(rid) = msg.pointer("/params/requestId") {
                    self.listens.lock().await.remove(&rid.to_string());
                }
            }
            return;
        }
        let response = match crate::mcp::protocol::classify_request(&msg) {
            Ok(Era::Legacy) => Some(self.handle_jsonrpc(msg).await),
            Ok(Era::Modern) => self.handle_modern(msg, tx).await,
            Err(err) => Some(crate::mcp::protocol::error_response(
                msg.get("id").unwrap_or(&Value::Null),
                err,
            )),
        };
        if let Some(response) = response {
            let _ = tx.send(response.to_string()).await;
        }
    }

    async fn handle_modern(&self, msg: Value, tx: &mut mpsc::Sender<String>) -> Option<Value> {
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        if !self.token_still_valid().await {
            return Some(jsonrpc_error(id, -32000, "invalid or revoked token"));
        }
        if msg.get("method").and_then(Value::as_str) == Some("subscriptions/listen") {
            self.listen(id, msg, tx).await;
            return None;
        }
        Some(
            handle_modern_jsonrpc(&self.state, &self.user, Some(self.vault_id.as_str()), msg).await,
        )
    }

    async fn listen(&self, id: Value, msg: Value, tx: &mut mpsc::Sender<String>) {
        let filter_json = msg
            .pointer("/params/notifications")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut lr = crate::mcp::listen_filter::parse_listen_request(&filter_json);
        let scoped_uri = format!("pkv://vault/{}", self.vault_id);
        // stdio sessions are scoped to one vault: accept only that URI and
        // drop everything else the client asked for.
        lr.accepted_uris.retain(|uri| uri == &scoped_uri);
        if !lr.accepted_uris.is_empty() {
            self.listens.lock().await.insert(
                id.to_string(),
                ListenEntry {
                    vault_id: self.vault_id.clone(),
                    id: id.clone(),
                    receiver: self.state.events.subscribe(&self.vault_id),
                },
            );
        }
        let ack = crate::mcp::listen_filter::acknowledged(&id, &lr);
        let _ = tx
            .send(serde_json::to_string(&ack).unwrap_or_default())
            .await;
    }

    /// Drain every registered listen receiver once and write a
    /// `notifications/resources/updated` line per pending event. `run` polls
    /// this on a fixed interval; calling it directly is how tests observe
    /// stream output without a real stdin.
    pub async fn drain_listens(&self, tx: &mut mpsc::Sender<String>) {
        // A listen outlives the request that registered it, so revalidate
        // the token here too (HTTP rechecks per frame; stdio previously
        // rechecked only on requests, letting a revoked session keep
        // receiving pushes until EOF). One check per drain covers every
        // entry; the 5s validity cache keeps the cost negligible on the
        // 50ms tick.
        let terminal = match self.token_validity().await {
            TokenValidity::Valid => {
                self.clear_transient_token_streak();
                false
            }
            TokenValidity::Revoked => true,
            TokenValidity::Transient => {
                // The database did not answer, so nothing is known about the
                // token: keep every listen and retry on the next tick.
                // Clearing here (pre-R2-3) turned one database hiccup into a
                // permanently silent session.
                let ticks = self
                    .transient_token_ticks
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                    + 1;
                if ticks
                    <= self
                        .transient_token_max_ticks
                        .load(std::sync::atomic::Ordering::Acquire)
                {
                    if !self
                        .transient_token_check_reported
                        .swap(true, std::sync::atomic::Ordering::Relaxed)
                    {
                        tracing::warn!(
                            vault_id = %self.vault_id,
                            "token revalidation hit a transient database error; keeping listen subscriptions and retrying"
                        );
                    }
                    return;
                }
                // The tolerance is bounded (R3-8): an outage that outlasts it
                // is indistinguishable from a revocation as far as this
                // session is concerned, and keeping the subscriptions alive
                // indefinitely would let a session that is in fact revoked go
                // on receiving pushes for as long as the error lasts.
                if !self
                    .transient_token_tolerance_reported
                    .swap(true, std::sync::atomic::Ordering::Relaxed)
                {
                    tracing::warn!(
                        vault_id = %self.vault_id,
                        ticks,
                        "token revalidation failed for the whole tolerance window; treating the session as no longer valid"
                    );
                }
                true
            }
        };
        if terminal {
            // Terminal. The subscription cannot be served any more, and
            // silence would leave the client believing it is still
            // subscribed (stdio has no channel close to lean on), so the
            // client gets one `notifications/resources/dropped` per
            // active listen — the same "you missed events, re-read the
            // resource" vocabulary the lagged path already uses — before
            // the registry is cleared. The re-read is expected to fail with
            // a 401 rather than return data, which is exactly the truth the
            // client needs; the alternative (silence) would hide it.
            let mut listens = self.listens.lock().await;
            let lines: Vec<String> = listens
                .values()
                .map(|entry| {
                    serde_json::to_string(&crate::mcp::notifications::resources_dropped(
                        &entry.vault_id,
                        &entry.id,
                    ))
                    .unwrap_or_default()
                })
                .collect();
            let dropped = listens.len();
            listens.clear();
            drop(listens);
            for line in lines {
                let _ = tx.send(line).await;
            }
            if dropped > 0 {
                tracing::warn!(
                    vault_id = %self.vault_id,
                    subscriptions = dropped,
                    "stdio session token is no longer valid; dropped its listen subscriptions"
                );
            }
            return;
        }
        let mut listens = self.listens.lock().await;
        let mut lines = Vec::new();
        let dead: Vec<String> = listens
            .iter_mut()
            .filter_map(|(key, entry)| loop {
                match entry.receiver.try_recv() {
                    Ok(event) => lines.push(
                        serde_json::to_string(&crate::mcp::notifications::resource_updated(
                            &entry.vault_id,
                            event.commit.clone(),
                            event,
                            &entry.id,
                        ))
                        .unwrap_or_default(),
                    ),
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                        // Lagged means the receiver already skipped to the
                        // oldest retained event; stdio has no replay path, so
                        // surface the gap before continuing the drain.
                        lines.push(
                            serde_json::to_string(&crate::mcp::notifications::resources_dropped(
                                &entry.vault_id,
                                &entry.id,
                            ))
                            .unwrap_or_default(),
                        );
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => return None,
                    Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                        return Some(key.clone())
                    }
                }
            })
            .collect();
        for key in dead {
            listens.remove(&key);
        }
        drop(listens);
        for line in lines {
            let _ = tx.send(line).await;
        }
    }
}

pub async fn run(state: AppState, vault_id: String, token_raw: String) -> Result<()> {
    run_on(
        state,
        vault_id,
        token_raw,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

/// Line-oriented MCP stdio loop over any duplex pair. `run` wires real
/// stdio; tests drive both halves of a `tokio::io::duplex` so the EOF
/// cleanup path (stdin close with a listen registered) is exercised for
/// real.
///
/// The outbound channel (64 lines) is consumed by a dedicated writer task,
/// never by this loop. Both producers — request responses from `pump` and
/// listen pushes from `drain_listens` — run inside the select arms below,
/// so if the loop itself received, a drain producing more lines than the
/// channel holds (a Lagged marker plus every retained ring event = 65)
/// would block forever on `tx.send().await` with no consumer left to
/// advance: a permanent session deadlock (CR-1). Decoupling also makes
/// stdin EOF safe: dropping `tx` closes the channel, the writer drains the
/// remaining buffered lines and exits, and awaiting the JoinHandle before
/// returning flushes everything still queued (the N-5 EOF race).
///
/// The writer reports its final outcome (`Result<(), std::io::Error>`) over
/// a oneshot when it ends — after the channel closes and drains, or on the
/// first write/flush error, where it stops draining immediately. The loop
/// selects on that outcome so a mid-session broken pipe ends the run at
/// once, and the EOF path awaits it (with the JoinHandle as backup for a
/// panic/abort) so `run_on` returns Err instead of a silent Ok.
pub async fn run_on<R, W>(
    state: AppState,
    vault_id: String,
    token_raw: String,
    reader: R,
    writer: W,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let session = StdioSession::authenticate(state, vault_id, token_raw).await?;
    let mut lines = tokio::io::BufReader::new(reader).lines();
    let (tx, mut rx) = mpsc::channel::<String>(64);
    let (outcome_tx, mut outcome_rx) =
        tokio::sync::oneshot::channel::<std::result::Result<(), std::io::Error>>();
    let writer_task = tokio::spawn(async move {
        let mut stdout = writer;
        let outcome = loop {
            let Some(line) = rx.recv().await else {
                // Channel closed: every buffered line was written.
                break Ok(());
            };
            if let Err(err) = stdout.write_all(line.as_bytes()).await {
                break Err(err);
            }
            if let Err(err) = stdout.write_all(b"\n").await {
                break Err(err);
            }
            if let Err(err) = stdout.flush().await {
                break Err(err);
            }
        };
        // Dropping `rx` here also makes every later `tx.send` fail fast
        // instead of queueing lines nobody will ever write.
        let _ = outcome_tx.send(outcome);
    });
    let mut tx = tx;
    // 50ms poll: bounded worst-case latency for push lines; the loop only
    // wakes on stdin or this tick — never a hot spin.
    let mut drain_tick = tokio::time::interval(LISTEN_DRAIN_INTERVAL);
    drain_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Ok(Some(line)) = line else {
                    // stdin EOF: dropping tx closes the outbound channel; the
                    // writer task drains every line still buffered in it,
                    // flushes, and reports its outcome below.
                    drop(tx);
                    break;
                };
                session.pump(&line, &mut tx).await;
            }
            _ = drain_tick.tick() => {
                session.drain_listens(&mut tx).await;
            }
            outcome = &mut outcome_rx => {
                // The writer ended mid-session: stdout is broken (or the
                // task panicked and dropped the sender), so nothing further
                // can be delivered — end the run now rather than answer
                // into the void until stdin EOF.
                return match outcome {
                    Ok(Err(err)) => Err(anyhow!("stdio write failed: {err}")),
                    Ok(Ok(())) => Err(anyhow!("stdio writer stopped before stdin EOF")),
                    Err(_) => Err(anyhow!(
                        "stdio writer task ended without reporting an outcome"
                    )),
                };
            }
        }
    }
    // Join first: the handle resolving proves the writer finished, so every
    // queued line has been written (or the failing write has happened); a
    // panic/abort surfaces here as a JoinError. The oneshot then carries
    // the write outcome itself.
    writer_task
        .await
        .map_err(|e| anyhow!("stdio writer task failed: {e}"))?;
    outcome_rx
        .await
        .map_err(|_| anyhow!("stdio writer task ended without reporting an outcome"))?
        .map_err(|err| anyhow!("stdio write failed: {err}"))?;
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
        // The credential rejection path deliberately carries no payload:
        // its wording would distinguish "invalid format" from "revoked",
        // and none of it may reach the caller — only
        // GENERIC_MCP_AUTH_ERROR is returned.
        Err(AuthErr::Credential) => {
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
    /// The internal rejection reason (bad format, revoked, disabled, user
    /// gone) is deliberately never surfaced: the caller only ever returns
    /// `GENERIC_MCP_AUTH_ERROR`, and logging the reason would put
    /// caller-controlled noise in the log. A unit variant — nothing is
    /// carried, so nothing can leak — matching `BasicAuthErr::Credential`
    /// in the git-http transport (R2-9).
    Credential,
    Internal(anyhow::Error),
}

async fn authenticate_token_inner(
    state: &AppState,
    token_raw: &str,
) -> std::result::Result<AuthenticatedUser, AuthErr> {
    if !token::looks_valid(token_raw) {
        return Err(AuthErr::Credential);
    }
    let token_hash = token::hash(token_raw);
    let (row, user_id) = state
        .tokens
        .find_by_hash(&token_hash)
        .await
        .map_err(|err| AuthErr::Internal(anyhow!(err)))?
        .ok_or(AuthErr::Credential)?;
    let user = state
        .users
        .find_by_id(&user_id)
        .await
        .map_err(|err| AuthErr::Internal(anyhow!(err)))?
        .ok_or(AuthErr::Credential)?;
    if !user.is_active {
        return Err(AuthErr::Credential);
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
        "initialize" => {
            let requested = request
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2025-06-18");
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": crate::mcp::protocol::legacy_negotiate_initialize(requested),
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": crate::mcp::protocol::SERVER_NAME,
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })
        }
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
                    Err(err) if err.to_string() == INVALID_TOOL_ARGUMENTS => {
                        jsonrpc_error(id, -32602, INVALID_TOOL_ARGUMENTS)
                    }
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

// Era-dispatched modern methods shared by both transports. Subscriptions are
// transport-specific (HTTP answers over SSE, stdio registers an in-session
// receiver), so transports must handle `subscriptions/listen` before calling
// this.
pub(crate) async fn handle_modern_jsonrpc(
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
        "server/discover" => {
            let result = crate::mcp::protocol::decorate_result(
                json!({
                    "supportedVersions": crate::mcp::protocol::SUPPORTED_VERSIONS,
                    "capabilities": {"tools": {}},
                    "instructions": "PKV Sync vault tools: list/read/search vault files, link graph, changes since, and guarded write tools.",
                }),
                Some((3_600_000, "private")),
            );
            json!({"jsonrpc": "2.0", "id": id, "result": result})
        }
        "tools/list" => {
            let result = crate::mcp::protocol::decorate_result(
                json!({"tools": tools::tool_definitions()}),
                Some((300_000, "private")),
            );
            json!({"jsonrpc": "2.0", "id": id, "result": result})
        }
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match serde_json::from_value::<ToolCallParams>(params) {
                Ok(params) => match call_tool(state, user, vault_scope, params).await {
                    Ok(result) => {
                        // Single source for the modern result envelope: also
                        // adds the SHOULD-level `_meta.serverInfo`.
                        let result = crate::mcp::protocol::decorate_result(result, None);
                        json!({"jsonrpc": "2.0", "id": id, "result": result})
                    }
                    Err(err) if err.to_string() == INVALID_TOOL_ARGUMENTS => {
                        jsonrpc_error(id, -32602, INVALID_TOOL_ARGUMENTS)
                    }
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
    // Non-object `arguments` (e.g. `[1,2]` or `"x"`) must fail here with a
    // recognizable marker, not inside the per-tool `from_value` — that
    // path classifies the failure as an internal error (-32000 plus an
    // ERROR log), misreporting a client mistake as server breakage (L-5).
    // Both dispatch eras map the marker to -32602 invalid params.
    if !params.arguments.is_object() && !params.arguments.is_null() {
        return Err(anyhow!(INVALID_TOOL_ARGUMENTS));
    }
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
        "file_too_large:",
        "invalid_commit:",
        "invalid_path:",
        "not_found:",
        "path_conflict:",
        "path_excluded:",
        "rate_limited:",
        "target_exists:",
        "unrelated_commit:",
        "unknown tool:",
        "unsupported_binary_move:",
        "wrong_file_kind:",
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
