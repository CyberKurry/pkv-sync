# AI 工具的 MCP 接入

[English](./mcp-howto.md) | 简体中文 | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

PKV Sync 可以通过 MCP server 暴露笔记库内容。服务端返回文件内容前会解析 blob pointer，也可以通过显式读写工具写入文件，并且必须使用普通 PKV Sync bearer 设备 token。

## 工具

- `list_vaults`：列出当前用户可访问的笔记库。
- `list_files {vault_id, at?}`：列出 HEAD 或在指定 `at` commit SHA 下的路径。
- `read_file {vault_id, path}`：读取 HEAD 下的文件。
- `read_file_at_commit {vault_id, path, commit}`：读取指定 commit 下的文件。
- `search {vault_id, query, at?, limit?}`：在文本文件中执行大小写不敏感的子串搜索。`at` 将搜索范围限定到某个历史 commit；`limit` 限制返回匹配数量。
- `write_file {vault_id, path, content, parent_commit}`：以 `parent_commit` 进行乐观并发控制，创建或更新文本文件。
- `delete_file {vault_id, path, parent_commit}`：以 `parent_commit` 进行乐观并发控制，删除文件。

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

当客户端连接一个已经运行的本地或内网 MCP 端点时，使用 HTTP。PKV Sync 提供两种 HTTP 部署模式：

- **内嵌模式**：在 `config.toml` 中设置 `[mcp].embed_in_serve = true`，`pkvsyncd serve` 会在主服务端口挂载 `/mcp`。
- **独立模式**：运行单独的 MCP 进程，适合专用监听地址、隔离 MCP 或独立扩缩容：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

端点路径始终是 `/mcp`；内嵌模式使用主服务 origin，独立模式使用单独的监听地址：

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

每个请求都必须包含：

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

部署密钥来自与主 PKV Sync 服务相同的配置文件。缺少或错误的部署密钥会在 bearer token 认证前直接返回 HTTP `404`。

HTTP transport 使用固定窗口限流，每 60 秒最多 120 次请求。超限时返回 HTTP `429`，JSON-RPC error code 为 `-32029`。失败的 MCP bearer token 认证也会在进程内限流，stdio 和 HTTP transport 合计每 60 秒最多 30 次失败尝试。

POST 承载 JSON-RPC 工具调用并返回 JSON 响应。GET 携带 `Accept: text/event-stream` 时订阅 `vault_changed` notification。事件 id 使用 `<vault-id>:<commit-sha>`，客户端重连时可作为 `Last-Event-ID` 传回，以 replay 断线期间错过的 commit。Replay 有上限；如果服务端无法覆盖错过的历史，会发送 `lagged`，客户端应通过同步 API 刷新。

除非放在可信网络控制之后，否则请把 HTTP 绑定到 loopback。bearer token 会授予该用户所有笔记库的读写访问权限。

## 读取和搜索上限

`search` 最多扫描 5000 个 tree 文件，最多返回 500 条匹配，并在生产环境搜索文本累计达到 256 MiB 后停止。`read_file` 和 `read_file_at_commit` 会在返回前解析 blob pointer；超过 64 MiB 的二进制/blob 响应会被拒绝，而不是被 base64 展开进 JSON。

## 写入工具

PKV Sync 在读取工具之外提供两个 MCP 写入工具：

- `write_file(vault_id, path, content, parent_commit)`：创建或更新文本文件。
- `delete_file(vault_id, path, parent_commit)`：删除文件。

### 乐观并发控制

每次写入都必须提供 `parent_commit`，也就是客户端认为当前笔记库 HEAD 所在的 commit hash。如果客户端上次读取后笔记库已经前进，服务端会返回 `{ "conflict": true, "current_head": "..." }`，并且不会写入。客户端需要重新读取、必要时合并，再用新的 `parent_commit` 重试。

### 限流

写入工具按 `(token, vault)` 组合限流，每分钟最多 60 次写入。读取工具和 SSE 订阅不受这个写入额度影响。

### 审计记录

每次成功写入或删除都会在活动日志中记录为 `mcp_write` 或 `mcp_delete`，details 中包含 path、commit 和 size。管理员可以在活动页查看 AI 驱动的改动。

### 注意：写入会进入 git 历史

AI 驱动的写入会成为笔记库 git 历史中的 commit。你可以通过普通 git 操作回滚，但无法让已经提交的改动“从未发生”；这种可审计性是有意设计。

## 客户端提示

- Claude Code、Codex CLI、Cherry Studio、OpenCode，以及通过桥接使用 MCP 的客户端，都可以通过启动 `pkvsyncd mcp` 使用 stdio 模式。
- 支持 Streamable HTTP 的客户端可以指向 `/mcp`，并在每个请求上发送 bearer auth 和部署密钥。
- 服务端是无状态的，不要求也不返回 `Mcp-Session-Id`。
