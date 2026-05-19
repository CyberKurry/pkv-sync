# AI 工具的 MCP 接入

[English](./mcp-howto.md) | 简体中文

PKV Sync 可以通过只读 MCP server 暴露笔记库内容。MCP server 不会写文件，返回文件内容前会解析 blob pointer，并且必须使用普通 PKV Sync bearer 设备 token。

## 工具

- `list_vaults`：列出当前用户可访问的笔记库。
- `list_files`：列出 HEAD 或指定 commit 下的路径。
- `read_file`：读取 HEAD 下的文件。
- `read_file_at_commit`：读取指定 commit 下的文件。
- `search`：在文本文件中执行大小写不敏感的子串搜索。

## stdio transport

本地 AI 工具需要启动命令时，使用 stdio。stdio 模式只暴露一个笔记库。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

也可以直接传入 token：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

当客户端连接一个已经运行的本地或内网 MCP 端点时，使用 HTTP。

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

端点为：

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

每个请求都必须包含：

```text
Authorization: Bearer pks_xxx
```

POST 承载 JSON-RPC 工具调用并返回 JSON 响应。GET 携带 `Accept: text/event-stream` 时订阅 `vault_changed` notification。事件 id 使用 `<vault-id>:<commit-sha>`，客户端重连时可作为 `Last-Event-ID` 传回，以 replay 断线期间错过的 commit。

除非放在可信网络控制之后，否则请把 HTTP 绑定到 loopback。bearer token 会授予该用户所有笔记库的只读访问权限。

## 客户端提示

- Claude Code、Codex CLI、Cherry Studio、OpenCode，以及通过桥接使用 MCP 的客户端，都可以通过启动 `pkvsyncd mcp` 使用 stdio 模式。
- 支持 Streamable HTTP 的客户端可以指向 `/mcp`，并在每个请求上发送 bearer auth。
- 服务端是无状态的，不要求也不返回 `Mcp-Session-Id`。
