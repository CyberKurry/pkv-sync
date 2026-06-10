# AI 工具的 MCP 接入

[English](./mcp-howto.md) | 简体中文 | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

文档版本：v1.2.1。

PKV Sync 可以通过 MCP server 暴露笔记库内容。服务端在返回文件内容前会解析 blob pointer，可以通过显式读写工具写入文件，并且要求使用普通的 PKV Sync bearer 设备 token。

## 工具

- `list_vaults`：列出已认证用户可访问的笔记库。
- `list_files {vault_id, at?}`：列出 HEAD 的路径；设置 `at` 时，列出该 commit SHA 下的路径。
- `read_file {vault_id, path}`：读取 HEAD 下的文件。
- `read_file_at_commit {vault_id, path, commit}`：读取指定 commit 下的文件。
- `search {vault_id, query, at?, limit?}`：在文本文件中执行大小写不敏感的子串搜索。`at` 将范围限定到某个历史 commit；`limit` 限制返回的匹配数量。
- `link_graph {vault_id, at?, path_prefix?, limit?}`：返回笔记库的 wikilink 和 Markdown 链接图。响应包含每个文件节点的 `outlinks` 和计算出的 `inlinks`、孤立页面、带有 `missing` 或 `ambiguous` 原因的断链，以及 `truncated` 标志。
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`：列出自 `since_commit` 以来新增、修改、删除或重命名的文件。响应包含 `from_commit`、当前 `to_commit`、`changes` 和 `truncated`；如果 `since_commit` 不是 HEAD 的祖先，工具会返回 `unrelated_commit`，以便客户端重新读取笔记库。
- `write_file {vault_id, path, content, parent_commit}`：以 `parent_commit` 进行乐观并发控制，创建或更新文本文件。
- `delete_file {vault_id, path, parent_commit}`：以 `parent_commit` 进行乐观并发控制，删除文件。
- `write_files {vault_id, parent_commit, writes?, deletes?}`：在一个 commit 中原子地创建、更新和／或删除多个文本文件。`writes[]` 包含 `{path, content}` 对象；`deletes[]` 包含路径。
- `move_file {vault_id, parent_commit, from, to}`：在一个 commit 中移动或重命名文本文件，并保留 git rename 历史。目标路径不能已经存在。

所有 MCP 读取工具都会遵守当前的 SyncPathFilter。被内置隐藏路径规则或运行时 exclude glob 拒绝的路径，不会被列出、搜索、读取、纳入链接图，也不会作为变更报告。

## stdio transport

本地 AI 工具需要启动命令时，使用 stdio。stdio 模式限定到一个笔记库。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

也可以直接传入 token：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

当客户端连接到一个已经运行的本地或内网 MCP 端点时，使用 HTTP。PKV Sync 提供两种 HTTP 部署模式：

- **嵌入模式**：在 `config.toml` 中设置 `[mcp].embed_in_serve = true`，然后 `pkvsyncd serve` 会在主服务端口挂载 `/mcp`。
- **独立模式**：运行单独的 MCP 进程，适合专用监听地址、隔离 MCP，或独立扩缩容：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

端点路径始终是 `/mcp`；嵌入模式使用主服务 origin，独立模式使用单独的监听地址：

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

每个请求都必须包含：

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

部署密钥来自与主 PKV Sync 服务相同的配置文件。缺少或错误的部署密钥会在 bearer token 认证前收到 HTTP `404`。

MCP HTTP 使用固定窗口限流，每 60 秒最多 120 次请求。超限时，服务端返回 HTTP `429`，并返回 code 为 `-32029` 的 JSON-RPC error。
失败的 MCP bearer-token 认证也会在进程内限流，stdio 和 HTTP transport 合计每 60 秒最多 30 次失败尝试。

POST 承载 JSON-RPC 工具调用并返回 JSON 响应。GET 携带 `Accept: text/event-stream` 时订阅 `vault_changed` notification。事件 id 使用 `<vault-id>:<commit-sha>`，客户端重连时可作为 `Last-Event-ID` 传回，以 replay 断线期间错过的 commit。Replay 有上限；如果服务端无法覆盖错过的历史，会发送 `lagged`，客户端应通过同步 API 刷新。

除非放在可信网络控制之后，否则请将 HTTP 绑定到 loopback。bearer token 会授予该用户所有笔记库的读写访问权限。

## 读取和搜索上限

`search` 最多扫描 5000 个可见 tree 文件，最多返回 500 条匹配，并在生产环境搜索文本累计达到 256 MiB 后停止。`link_graph` 最多扫描 5000 个可见文本文件，并使用相同的生产环境文本预算。`changes_since` 最多返回 5000 条可见变更项。`read_file` 和 `read_file_at_commit` 会在返回前解析 blob pointer；超过 64 MiB 的二进制/blob 响应会被拒绝，而不是被 base64 展开进 JSON。

## 写入工具

PKV Sync 在读取工具之外提供四个 MCP 写入工具：

- `write_file(vault_id, path, content, parent_commit)`：创建或更新文本文件。
- `delete_file(vault_id, path, parent_commit)`：删除文件。
- `write_files(vault_id, parent_commit, writes[], deletes[])`：在一个 commit 中原子地创建、更新和删除多个文本文件。如果任一路径无效、文件超过 `max_file_size`、批次为空（`empty_batch`），或批次超过 100 个变更（`batch_too_large`），服务端不会提交任何内容。陈旧的 `parent_commit` 会返回常规 `Conflict` 响应。
- `move_file(vault_id, parent_commit, from, to)`：在单个 commit 中移动或重命名一个文本文件。它会拒绝已存在的目标（`target_exists`）、二进制／blob-pointer 源文件（`unsupported_binary_move`），以及缺失或隐藏的源文件（`not_found`）。

### 乐观并发控制

每次写入都必须提供 `parent_commit`，也就是客户端认为当前笔记库 HEAD 所在的 commit hash。如果客户端上次读取后笔记库已经前进，服务端会返回 `{ "conflict": true, "current_head": "..." }`，并且不会写入。客户端需要重新读取、必要时合并，再用新的 `parent_commit` 重试。

### 限流

写入工具按 `(token, vault)` 组合限流，每分钟最多 60 次写入。`write_files` 整个批次只消耗一次限流记录。读取工具和 SSE 订阅不受这个写入额度影响。

1.2.1 的加固让写入校验保持 fail-closed：`writes[]` 和 `deletes[]` 中归一化后重复的路径会被拒绝，隐藏或排除路径不会泄漏目标存在性，无效的 `move_file` 来源会在消耗写入额度前被拒绝。MCP 鉴权错误保持泛化，Streamable HTTP JSON 请求体上限为 100 MiB。

### 审计记录

每次成功写入、批量写入、移动或删除都会在活动日志中记录为 `mcp_write` 或 `mcp_delete`，details 中包含路径摘要、commit 和 size。管理员可以在活动页查看 AI 驱动的改动。

### 注意：写入会进入 git 历史

AI 驱动的写入会成为笔记库 git 历史中的 commit。你可以通过普通 git 操作回滚，但无法让已经提交的改动“从未发生”；这种可审计性是有意设计。

## 客户端提示

- Claude Code、Codex CLI、Cherry Studio、OpenCode，以及通过桥接使用 MCP 的客户端，都可以通过启动 `pkvsyncd mcp` 使用 stdio 模式。
- 支持 Streamable HTTP 的客户端可以指向 `/mcp`，并在每个请求上发送 bearer auth 和部署密钥。
- 服务端是无状态的，不要求也不返回 `Mcp-Session-Id`。
