# MCP access for AI tools

English | [简体中文](./mcp-howto.zh-CN.md)

PKV Sync can expose vault contents through a read-only MCP server. The MCP
server never writes files, resolves blob pointers before returning file
content, and requires a normal PKV Sync bearer device token.

## Tools

- `list_vaults`: list vaults available to the authenticated user.
- `list_files`: list paths at HEAD or a specific commit.
- `read_file`: read a file at HEAD.
- `read_file_at_commit`: read a file at a specific commit.
- `search`: case-insensitive substring search over text files.

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
endpoint.

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

The endpoint is:

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

Every request must include:

```text
Authorization: Bearer pks_xxx
```

POST carries JSON-RPC tool calls and returns JSON responses. GET with
`Accept: text/event-stream` subscribes to `vault_changed` notifications. Event
ids use `<vault-id>:<commit-sha>` and can be sent back as `Last-Event-ID` to
replay missed commits.

Bind HTTP to loopback unless you put it behind trusted network controls. A
bearer token gives read access to every vault owned by that user.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, and bridge-based MCP clients
  can use stdio mode by launching `pkvsyncd mcp`.
- Clients that support Streamable HTTP can point at `/mcp` and send bearer auth
  on every request.
- The server is stateless; it does not require or return `Mcp-Session-Id`.
