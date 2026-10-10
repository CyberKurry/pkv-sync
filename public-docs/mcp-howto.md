# MCP access for AI tools

English | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

Document version: v1.5.1.

PKV Sync can expose vault contents through an MCP server. The server resolves
blob pointers before returning file content, can write through explicit
read-write tools, and requires a normal PKV Sync bearer device token.

## Tools

- `list_vaults`: list vaults available to the authenticated user.
- `list_files {vault_id, at?}`: list paths at HEAD or, when `at` is set, at that commit SHA.
- `read_file {vault_id, path}`: read a file at HEAD.
- `read_file_at_commit {vault_id, path, commit}`: read a file at a specific commit.
- `search {vault_id, query, at?, limit?}`: case-insensitive substring search over text files. `at` scopes to a historical commit; `limit` caps the number of returned matches.
- `link_graph {vault_id, at?, path_prefix?, limit?}`: return the vault's wikilink and Markdown link graph. The response includes per-file nodes with `outlinks` and computed `inlinks`, orphaned pages, broken links with `missing` or `ambiguous` reasons, and a `truncated` flag.
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`: list files added, modified, deleted, or renamed since `since_commit`. The response includes `from_commit`, current `to_commit`, `changes`, and `truncated`; if `since_commit` is not an ancestor of HEAD, the tool returns `unrelated_commit` so the client can re-read the vault.
- `write_file {vault_id, path, content, parent_commit}`: create or update a text file with optimistic concurrency on `parent_commit`.
- `delete_file {vault_id, path, parent_commit}`: delete a file with optimistic concurrency on `parent_commit`.
- `write_files {vault_id, parent_commit, writes?, deletes?}`: atomically create, update, and/or delete multiple text files in one commit. `writes[]` contains `{path, content}` objects; `deletes[]` contains paths.
- `move_file {vault_id, parent_commit, from, to}`: move or rename a text file in one commit while preserving git rename history. The target must not already exist.

Paths rejected by the active `SyncPathFilter` are not listed, searched, read, or
included in link graphs or change reports. Read tools (`list_files`, `read_file`,
`read_file_at_commit`, `search`) accept hidden paths that a per-vault allowlist
re-admits, and can return generated `.conflict-*` sidecars. `link_graph`,
`changes_since`, and every write tool apply a stricter policy and reject hidden
paths outright, so an agent cannot address hidden files for actions.

## stdio transport

Use stdio for local AI tools that launch a command. stdio mode is scoped to one
vault.

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

You can also pass the token directly:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

stdio speaks both protocol eras. Modern 2026-07-28 clients send per-request
`_meta` protocol fields and receive modern result envelopes; legacy clients
negotiate through `initialize`, and the server echoes the requested legacy
version. Modern stdio clients can also call `subscriptions/listen` for the
scoped vault; the session then prints `notifications/resources/updated`
lines until the client sends a `notifications/cancelled` notification
carrying the listen's request id (or exits).

## Streamable HTTP transport

Use HTTP when the client connects to an already running local or internal MCP
endpoint. MCP HTTP is embedded-only: `[mcp].embed_in_serve` defaults to `true`,
so `pkvsyncd serve` mounts `/mcp` on the main server port. Set
`[mcp].embed_in_serve = false` to disable the embedded endpoint. The standalone
`pkvsyncd mcp --transport http --bind` process was removed in this version;
point existing standalone deployments at the main server's `/mcp`.

The endpoint path is always `/mcp`, on the main server origin:

```text
POST http://127.0.0.1:6710/mcp
GET  http://127.0.0.1:6710/mcp
```

Every request must include:

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

The deployment key is read from the same config file as the main PKV Sync
server. Missing or wrong deployment keys receive HTTP `404` before bearer-token
authentication.

MCP HTTP is fixed-window rate limited at 120 requests per 60 seconds. When the
limit is exceeded, the server responds with HTTP `429` and a JSON-RPC error
using code `-63001`. Failed MCP bearer-token authentication responds with
HTTP `401` and code `-63002`, and is also rate limited in-process at 30
attempts per 60 seconds across stdio and HTTP transports.

The MCP specification's `Origin`-header DNS-rebinding check is intentionally
not implemented on `/mcp`: the endpoint requires the non-ambient deployment
key and bearer device token (browsers cannot attach either to a cross-origin
request), admin session cookies are rejected, and no CORS policy exposes the
endpoint to browser origins. If a deployment disables any of these
assumptions, it should front `/mcp` with its own `Origin` checks.

## MCP 2026-07-28 (modern) clients

The server serves two protocol eras on the same endpoint. Modern clients send
per-request protocol fields in `params._meta`
(`io.modelcontextprotocol/protocolVersion` plus
`io.modelcontextprotocol/clientCapabilities`) and must repeat the version in
an `MCP-Protocol-Version` header, the method in an `Mcp-Method` header, and —
when the request carries `params.name` — the name in an `Mcp-Name` header
(base64-encoded as `=?base64?...?=` when it is not plain ASCII). A mismatch
between any header and the body returns HTTP `400` with code `-32020`.
Requesting an unsupported version returns HTTP `400` with code `-32022` and
the supported list (`2026-07-28`, `2025-06-18`). Unknown methods return HTTP
`404` with `-32601`, and notification-only messages (no `id`) are answered
with HTTP `202` and no body.

Modern results carry `resultType`, and `tools/list` and `server/discover`
additionally carry `ttlMs`/`cacheScope` cache hints. `server/discover`
reports `supportedVersions` so a client can pick its era before connecting.
Legacy `initialize` clients are still served: the server echoes the requested
legacy version and answers with legacy envelopes.

## Change notifications

- **Legacy clients**: GET `/mcp` with `Accept: text/event-stream` subscribes
  to `vault_changed` notifications. Each event's id is the bare commit SHA,
  and it can be sent back as `Last-Event-ID` to replay missed commits for
  that vault. Replay is capped; if the server cannot cover the missed
  history, it emits `lagged` and the client should refresh from the sync API.
  A value containing `:` is not accepted as a resume position, and when the
  token can see more than one vault only the resuming vault's position is
  known, so the stream also emits `lagged` for the others.
- **Modern clients**: POST a `subscriptions/listen` request with
  `params.notifications.resourceSubscriptions` set to
  `["pkv://vault/<id>"]` URIs (only vaults the token may see are accepted).
  The response is an SSE stream that opens with a
  `notifications/subscriptions/acknowledged` event echoing the accepted
  subscriptions, then emits `notifications/resources/updated` events whose
  params carry `uri`, `commit`, `event`, and `_meta.subscriptionId`. Cancel
  by closing the stream. The modern stream is not resumable: there is no
  `Last-Event-ID` replay, so after a reconnect the client listens again and
  diffs what it reads.

Bind HTTP to loopback unless you put it behind trusted network controls. A
bearer token gives read and write access to every vault owned by that user.

## Read and search limits

`search` scans at most 5000 visible tree files — a vault with more than 5000
visible files makes the call fail with `too many files to search` rather than
being partially scanned. It returns at most 500 matches (default 100), and
aborts with an error once it has read more than 256 MiB of text in production.
`link_graph` scans at most 5000 visible text files with the same production text
budget, but reports `truncated: true` instead of failing. `changes_since`
returns at most 5000 visible change entries. `read_file` and
`read_file_at_commit` resolve blob pointers before responding; binary content is
returned base64-encoded with `encoding: "base64"`, and binary/blob responses
above 64 MiB are rejected instead of being base64-expanded into JSON.

## Write tools

PKV Sync exposes four MCP write tools alongside the read tools:

- `write_file(vault_id, path, content, parent_commit)`: create or update a text file.
- `delete_file(vault_id, path, parent_commit)`: delete a file.
- `write_files(vault_id, parent_commit, writes[], deletes[])`: create, update, and delete multiple text files atomically in one commit. If any path is invalid, a file exceeds `max_file_size`, the batch is empty (`empty_batch`), or the batch exceeds 100 changes (`batch_too_large`), nothing is committed. A stale `parent_commit` returns the normal `Conflict` response.
- `move_file(vault_id, parent_commit, from, to)`: move or rename one text file in a single commit. It refuses existing targets (`target_exists`), binary/blob-pointer sources (`unsupported_binary_move`), and missing or hidden sources (`not_found`).

### Optimistic concurrency control

Every write requires `parent_commit`: the commit hash the client believes is
the current vault head. If the vault has advanced since the client last read,
the server returns `{ "conflict": true, "current_head": "..." }` without
writing. The client must re-read, merge if needed, and retry with the new
`parent_commit`.

### Rate limit

Write tools are rate-limited per `(token, vault)` pair at 60 writes per minute.
`write_files` spends one rate-limit record for the whole batch. Read tools and
SSE subscriptions are unaffected by this write quota.

The 1.2.1 hardening keeps write validation fail-closed: duplicate normalized
paths across `writes[]` and `deletes[]` are rejected, hidden or excluded paths
do not leak target existence, and invalid `move_file` sources are rejected
before consuming write quota. MCP auth errors stay generic, and Streamable HTTP
JSON bodies are capped at 100 MiB.

### Audit trail

Every successful write, batch write, move, or delete is recorded in the
activity log as `mcp_write` or `mcp_delete`, with a path summary, commit, and
size in the details. Admins can review AI-driven changes from the activity
page.

### Caveat: writes enter git history

AI-driven writes are commits in the vault git history. You can roll back with
normal git operations, but there is no way to make a committed change "never
have happened"; that audit trail is intentional.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, and bridge-based MCP clients
  can use stdio mode by launching `pkvsyncd mcp`.
- Clients that support Streamable HTTP can point at `/mcp` and send bearer auth
  plus the deployment key on every request.
- Both transports are dual-era: legacy `initialize` clients keep working, and
  modern 2026-07-28 clients get per-request version handling, `server/discover`,
  and `subscriptions/listen` push.
- The server is stateless; it does not require or return `Mcp-Session-Id`.
