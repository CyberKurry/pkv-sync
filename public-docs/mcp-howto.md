# MCP access for AI tools

English | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

Document version: v1.7.0.

PKV Sync can expose vault contents through an MCP server. The server resolves
blob pointers before returning file content, can write through explicit
read-write tools, and requires a normal PKV Sync bearer device token.

## Tools

- `list_vaults`: list vaults available to the authenticated user.
- `list_files {vault_id, at?}`: list paths at HEAD or, when `at` is set, at that commit SHA.
- `read_file {vault_id, path}`: read a file at HEAD. The response carries `sha256`, the hash the write guards expect.
- `read_file_at_commit {vault_id, path, commit}`: read a file at a specific commit (also with `sha256`).
- `search {vault_id, query, at?, limit?, path_prefix?, frontmatter?, after?}`: case-insensitive substring search over text files. `at` scopes to a historical commit; `limit` caps the returned matches (a hard cap — a page never overshoots it); `path_prefix` restricts the scan to one subtree before any file is read; `frontmatter` filters on top-level frontmatter (`{"kind":"wiki"}` matches a trimmed whole scalar, `{"sources":true}` requires the key to exist, and files the filter drops still count against the content budget); `after` is an opaque cursor — pass the previous response's `next_after` back verbatim and the pages tile the result exactly, with nothing skipped or repeated. A cursor this server did not issue is rejected as `invalid_cursor`.
- `link_graph {vault_id, at?, path_prefix?, limit?, kind?}`: return the vault's wikilink and Markdown link graph. The response includes per-file nodes with `outlinks` and computed `inlinks`, orphaned pages, broken links with `missing` or `ambiguous` reasons, and a `truncated` flag. `kind` keeps only files whose top-level frontmatter `kind:` matches, while link targets still resolve against every visible text file — so a wiki page citing a source page is not reported as broken, and orphans are computed inside the selected layer.
- `stale_pages {vault_id, path_prefix?, at?, limit?}`: report which wiki pages have sources that changed after the page's own `source_commit` anchor. Each entry lists `changed_sources`, `removed_sources`, `renamed` (`from`/`to`), `unresolved_sources` and `last_reconciled`. The comparison is a git tree diff, so it answers "the evidence moved" — never "this claim is wrong", which stays the agent's judgement. Three honesty rules: a source the server cannot check (hidden, excluded, or nonexistent) is reported in `unresolved_sources` rather than dropped; a page that declares `sources:` without a `source_commit` is listed in `missing_source_commit` instead of being measured against a guess; and a page whose diff was clipped at the listing cap comes back with `verified: false` even when nothing is listed, so an incomplete scan cannot read as clean. Requires the `sources:`/`source_commit:` frontmatter convention documented in the LLM Wiki workflow guide.
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
negotiate through `initialize`, and the server echoes the requested version
when it knows it — including `2026-07-28` — and otherwise answers with the
newest legacy version (`2025-11-25`). Modern stdio clients can also call
`subscriptions/listen` for the scoped vault; the session then prints
`notifications/resources/updated` lines until the client sends a
`notifications/cancelled` notification carrying the listen's request id (or
exits). JSON-RPC batches are not supported on either transport: a batch line
(a JSON array) is answered with a single `-32600 "batch requests are not
supported"` error line instead of silence. If the session's Token cannot be
revalidated for about 30 seconds — for example while the server's database is
unreachable — the session treats it as revoked and prints one
`notifications/resources/dropped` per active listen, then stops pushing.

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
attempts per 60 seconds per transport instance: the embedded HTTP endpoint
shares the server process's limiter, while each `pkvsyncd mcp` process builds
its own state and therefore its own limiter.

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
the supported list (`2026-07-28`). Invalid params in a modern request — for
example a malformed `_meta` or non-object tool `arguments` — return HTTP
`400` with code `-32602`. Unknown methods return HTTP
`404` with `-32601`, and notification-only messages (no `id`) are answered
with HTTP `202` and no body, except `notifications/cancelled`, which returns
`405`: cancellation over Streamable HTTP is done by closing the SSE
response stream. JSON-RPC batches are not supported on either transport:
an array body on `POST /mcp` is answered with code `-32600`
(`batch requests are not supported`) inside a `200` JSON-RPC envelope — the
same error stdio answers a batch line with — rather than a `202` that
pretends to have accepted it. The envelope is always the legacy one, because
an array cannot carry `params._meta` and therefore never classifies as a
modern request.

Modern results carry `resultType`, and `tools/list` and `server/discover`
additionally carry `ttlMs`/`cacheScope` cache hints. `server/discover`
reports `supportedVersions` so a client can pick its era before connecting. (modern only; legacy `initialize` negotiation stays outside this list)
Legacy `initialize` clients are still served: the server echoes a requested
legacy version, echoes `2026-07-28` verbatim when that modern version is
requested, and falls back to the newest legacy version (`2025-11-25`) for an
unknown one; answers use legacy envelopes.

## Change notifications

- **Legacy clients**: GET `/mcp` with `Accept: text/event-stream` subscribes
  to `vault_changed` notifications. Each event's id is the bare commit SHA,
  and it can be sent back as `Last-Event-ID` to replay missed commits for
  that vault. Replay is capped; if the server cannot cover the missed
  history, it emits `lagged` and the client should refresh from the sync API.
  A resume position the server cannot verify — unreadable or unrecoverable
  history — is answered the same way, never with a silent live stream.
  A value containing `:` is not accepted as a resume position, and when the
  token can see more than one vault only the resuming vault's position is
  known, so the stream also emits `lagged` for the others. Live-broadcast
  `lagged` events carry the affected vault id in the event data;
  replay-level `lagged` events carry empty data.
- **Modern clients**: POST a `subscriptions/listen` request with
  `params.notifications.resourceSubscriptions` set to
  `["pkv://vault/<id>"]` URIs (only vaults the token may see are accepted).
  Duplicate URIs in `resourceSubscriptions` are deduplicated (first
  occurrence wins) in the acknowledgment.
  The response is an SSE stream that opens with a
  `notifications/subscriptions/acknowledged` event echoing the accepted
  subscriptions, then emits `notifications/resources/updated` events whose
  params carry `uri`, `commit`, `event`, and `_meta.subscriptionId`. If the
  server's event buffer overflows (a burst of more than 64 commits while the
  stream is slow), the listen stream emits a
  `notifications/resources/dropped` notification carrying the affected
  `uri` and the same `_meta` subscription id; the client should re-read
  that vault's state. Cancel
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
budget, but reports `truncated: true` instead of failing. Link extraction is
also capped per file (10000 links) and in total for broken links (20000);
hitting either cap sets `truncated: true`. `changes_since`
returns at most 5000 visible change entries. `read_file` and
`read_file_at_commit` resolve blob pointers before responding; binary content is
returned base64-encoded with `encoding: "base64"`, and binary/blob responses
above 64 MiB are rejected instead of being base64-expanded into JSON.

## Write tools

PKV Sync exposes four MCP write tools alongside the read tools:

- `write_file(vault_id, path, content, parent_commit, dry_run?, expected_hashes?)`: create or update a text file.
- `delete_file(vault_id, path, parent_commit, dry_run?, expected_hashes?)`: delete a file.
- `write_files(vault_id, parent_commit, writes[], deletes[], dry_run?, expected_hashes?)`: create, update, and delete multiple text files atomically in one commit. If any path is invalid, a file exceeds `max_file_size`, the batch is empty (`empty_batch`), or the batch exceeds 100 changes (`batch_too_large`), nothing is committed. A stale `parent_commit` returns the normal `Conflict` response.
- `move_file(vault_id, parent_commit, from, to, dry_run?, expected_hashes?)`: move or rename one text file in a single commit. It refuses existing targets (`target_exists`), binary/blob-pointer sources (`unsupported_binary_move`), and missing or hidden sources (`not_found`).

### Dry runs and content guards

Two optional arguments make an agent's write reviewable before it happens and
safe against concurrent human edits.

`dry_run: true` validates everything the real call would (parent currency, path
visibility, size and batch caps, the guards below) and answers
`{"dry_run": true, "would_write": [{"path", "change", "old_path?", "size_bytes"}], "parent_commit"}` without committing, recording activity, or spending write quota. A move is reported as one `rename` entry. It is not a promise: checks that live below the commit path (file-kind and exclude-glob enforcement) do not run, so a dry run can pass where the real write is refused.

`expected_hashes: {"<path>": "<sha256>"}` guards individual files — the hash
`read_file` returned when the agent read them. If any guarded path no longer
matches, the whole call is refused with
`content_diverged: path=<p> expected=<sha> actual=<sha|"null">` (a binary
target is refused outright, since the caller cannot have hashed bytes it never
received) and nothing is committed. The map is additive, so the `deletes[]`
shape is unchanged for existing clients. `expected_hashes` is checked after
`parent_commit`: an out-of-date parent still answers `Conflict`, because the
client has to re-read anyway.

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
activity log as `mcp_write` or `mcp_delete`, with the tool that made the
change, a path summary, the commit, and the size in the details. The admin
activity page shows the tool next to the device (agent) name and can filter by
`?agent=<device name>` and `?tool=<tool name>`, so "which agent deleted this"
and "what has this agent been doing" are both answerable. A dry run records
nothing, because nothing happened.

### Caveat: writes enter git history

AI-driven writes are commits in the vault git history. You can roll back with
normal git operations, but there is no way to make a committed change "never
have happened"; that audit trail is intentional.

## Registry

The server is listed in the official MCP Registry as
`io.github.cyberkurry/pkv-sync` (`mcp/server.json` in this repository), which
points at the published container image and configures the session with
`PKV_TOKEN` and `PKV_VAULT_ID`. Self-hosted servers can also be wired up by
hand with the commands above.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, and bridge-based MCP clients
  can use stdio mode by launching `pkvsyncd mcp`.
- Clients that support Streamable HTTP can point at `/mcp` and send bearer auth
  plus the deployment key on every request.
- Both transports are dual-era: legacy `initialize` clients keep working, and
  modern 2026-07-28 clients get per-request version handling, `server/discover`,
  and `subscriptions/listen` push.
- The server is stateless; it does not require or return `Mcp-Session-Id`.
