# MCP access for AI tools

English | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

PKV Sync can expose vault contents through an MCP server. The server resolves
blob pointers before returning file content, can write through explicit
read-write tools, and requires a normal PKV Sync bearer device token.

## Tools

- `list_vaults`: list vaults available to the authenticated user.
- `list_files {vault_id, at?}`: list paths at HEAD or, when `at` is set, at that commit SHA.
- `read_file {vault_id, path}`: read a file at HEAD.
- `read_file_at_commit {vault_id, path, commit}`: read a file at a specific commit.
- `search {vault_id, query, at?, limit?}`: case-insensitive substring search over text files. `at` scopes to a historical commit; `limit` caps the number of returned matches.
- `write_file {vault_id, path, content, parent_commit}`: create or update a text file with optimistic concurrency on `parent_commit`.
- `delete_file {vault_id, path, parent_commit}`: delete a file with optimistic concurrency on `parent_commit`.

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

`search` scans at most 5000 tree files, returns at most 500 matches, and stops
after 256 MiB of searched text in production. `read_file` and
`read_file_at_commit` resolve blob pointers before responding; binary/blob
responses above 64 MiB are rejected instead of being base64-expanded into JSON.

## Write tools

PKV Sync exposes two MCP write tools alongside the read tools:

- `write_file(vault_id, path, content, parent_commit)`: create or update a text file.
- `delete_file(vault_id, path, parent_commit)`: delete a file.

### Optimistic concurrency control

Every write requires `parent_commit`: the commit hash the client believes is
the current vault head. If the vault has advanced since the client last read,
the server returns `{ "conflict": true, "current_head": "..." }` without
writing. The client must re-read, merge if needed, and retry with the new
`parent_commit`.

### Rate limit

Write tools are rate-limited per `(token, vault)` pair at 60 writes per minute.
Read tools and SSE subscriptions are unaffected by this write quota.

### Audit trail

Every successful write or delete is recorded in the activity log as
`mcp_write` or `mcp_delete`, with the path, commit, and size in the details.
Admins can review AI-driven changes from the activity page.

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
