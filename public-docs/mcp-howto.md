# MCP access for AI tools

English | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

Document version: v1.4.3.

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

All MCP read tools honor the current SyncPathFilter. Paths rejected by built-in hidden-path rules or runtime exclude globs are not listed, searched, read, included in link graphs, or reported as changes.

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

## Streamable HTTP transport

Use HTTP when the client connects to an already running local or internal MCP
endpoint. PKV Sync offers two HTTP deployment modes:

- **Embedded**: set `[mcp].embed_in_serve = true` in `config.toml`, then
  `pkvsyncd serve` mounts `/mcp` on the main server port.
- **Standalone**: run a separate MCP process, useful for dedicated bind
  addresses, air-gapped MCP, or independent scaling:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

The endpoint path is always `/mcp`; use the main server origin for embedded
mode or the standalone bind address for standalone mode:

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
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
using code `-32029`.
Failed MCP bearer-token authentication is also rate limited in-process at 30
attempts per 60 seconds across stdio and HTTP transports.

POST carries JSON-RPC tool calls and returns JSON responses. GET with
`Accept: text/event-stream` subscribes to `vault_changed` notifications. Event
ids use `<vault-id>:<commit-sha>` and can be sent back as `Last-Event-ID` to
replay missed commits. Replay is capped; if the server cannot cover the missed
history, it emits `lagged` and the client should refresh from the sync API.

Bind HTTP to loopback unless you put it behind trusted network controls. A
bearer token gives read and write access to every vault owned by that user.

## Read and search limits

`search` scans at most 5000 visible tree files, returns at most 500 matches,
and stops after 256 MiB of searched text in production. `link_graph` scans at
most 5000 visible text files and uses the same production text budget.
`changes_since` returns at most 5000 visible change entries. `read_file` and
`read_file_at_commit` resolve blob pointers before responding; binary/blob
responses above 64 MiB are rejected instead of being base64-expanded into JSON.

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
- The server is stateless; it does not require or return `Mcp-Session-Id`.
