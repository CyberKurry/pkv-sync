# AI 工具的 MCP 接入

[English](./mcp-howto.md) | 简体中文 | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

文档版本：v1.7.0。

PKV Sync 可以通过 MCP server 暴露笔记库内容。服务端在返回文件内容前会解析 blob pointer，可以通过显式读写工具写入文件，并且要求使用普通的 PKV Sync bearer 设备 token。

## 工具

- `list_vaults`：列出已认证用户可访问的笔记库。
- `list_files {vault_id, at?}`：列出 HEAD 的路径；设置 `at` 时，列出该 commit SHA 下的路径。
- `read_file {vault_id, path}`：读取 HEAD 下的文件。响应会带上 `sha256`，也就是写入守卫所期望的哈希。
- `read_file_at_commit {vault_id, path, commit}`：读取指定 commit 下的文件（同样带有 `sha256`）。
- `search {vault_id, query, at?, limit?, path_prefix?, frontmatter?, after?}`：在文本文件中执行大小写不敏感的子串搜索。`at` 将范围限定到某个历史 commit；`limit` 限制返回的匹配数量（硬上限——一页绝不会超出该上限）；`path_prefix` 会在读取任何文件之前把扫描限定到某个子树；`frontmatter` 按顶层 frontmatter 过滤（`{"kind":"wiki"}` 匹配去除首尾空白后的完整标量，`{"sources":true}` 要求该键存在，而被过滤器丢弃的文件仍会计入内容预算）；`after` 是不透明游标——把上一次响应中的 `next_after` 原样传回，各页就能精确铺满结果，既不跳过也不重复。凡不是本服务端签发的游标都会被拒绝并报 `invalid_cursor`。
- `link_graph {vault_id, at?, path_prefix?, limit?, kind?}`：返回笔记库的 wikilink 和 Markdown 链接图。响应包含每个文件节点的 `outlinks` 和计算出的 `inlinks`、孤立页面、带有 `missing` 或 `ambiguous` 原因的断链，以及 `truncated` 标志。`kind` 只保留顶层 frontmatter `kind:` 匹配的文件，而链接目标仍会对整个可见文本文件解析——因此引用了 source 页面的 wiki 页面不会被报为断链，孤儿则在被选中的层内计算。
- `stale_pages {vault_id, path_prefix?, at?, limit?}`：报告哪些 wiki 页面的 source 在该页自身 `source_commit` 锚点之后发生了变化。每个条目列出 `changed_sources`、`removed_sources`、`renamed`（`from`/`to`）、`unresolved_sources` 和 `last_reconciled`。比较基于 git tree diff，因此它回答的是“证据移动了”——绝不会是“这个论断错了”，后者仍然由 agent 判断。三条诚实规则：服务端无法核实的 source（隐藏、被排除或不存在）会记录在 `unresolved_sources` 中，而不是被丢弃；声明了 `sources:` 却没有 `source_commit` 的页面会列入 `missing_source_commit`，而不是拿猜测去度量；diff 在清单上限处被裁剪的页面即使什么都没列出，也会返回 `verified: false`，因此不完整的扫描不可能被读成干净。需要 LLM Wiki 工作流指南中记录的 `sources:`/`source_commit:` frontmatter 约定。
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`：列出自 `since_commit` 以来新增、修改、删除或重命名的文件。响应包含 `from_commit`、当前 `to_commit`、`changes` 和 `truncated`；如果 `since_commit` 不是 HEAD 的祖先，工具会返回 `unrelated_commit`，以便客户端重新读取笔记库。
- `write_file {vault_id, path, content, parent_commit}`：以 `parent_commit` 进行乐观并发控制，创建或更新文本文件。
- `delete_file {vault_id, path, parent_commit}`：以 `parent_commit` 进行乐观并发控制，删除文件。
- `write_files {vault_id, parent_commit, writes?, deletes?}`：在一个 commit 中原子地创建、更新和／或删除多个文本文件。`writes[]` 包含 `{path, content}` 对象；`deletes[]` 包含路径。
- `move_file {vault_id, parent_commit, from, to}`：在一个 commit 中移动或重命名文本文件，并保留 git rename 历史。目标路径不能已经存在。

被当前 `SyncPathFilter` 拒绝的路径不会被列出、搜索、读取，也不会纳入链接图或变更报告。读取类工具（`list_files`、`read_file`、`read_file_at_commit`、`search`）会接受被按库 allowlist 重新纳入的隐藏路径，并可能返回自动生成的 `.conflict-*` 旁路文件。`link_graph`、`changes_since` 以及所有写入类工具采用更严格的策略，直接拒绝隐藏路径，因此 agent 无法对隐藏文件执行操作。

## stdio transport

本地 AI 工具需要启动命令时，使用 stdio。stdio 模式限定到一个笔记库。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

也可以直接传入 token：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

stdio 同时支持两个协议时代。现代 2026-07-28 客户端在请求中发送 `_meta` 协议字段，并收到现代结果封装；旧版客户端通过 `initialize` 协商，服务端会回显所请求的、它已知的版本（包括 `2026-07-28`），未知版本则回退到最新的旧版本（`2025-11-25`）。现代 stdio 客户端还可以对限定的笔记库调用 `subscriptions/listen`；会话随后会持续输出 `notifications/resources/updated` 行，直到客户端发送携带该 listen 请求 id 的 `notifications/cancelled` 通知（或退出）。如果会话的 Token 在大约 30 秒内始终无法重新校验（例如服务端数据库不可达），会话会将其视为已吊销：为每个活动 listen 输出一条 `notifications/resources/dropped`，随后停止推送。两种 transport 都不支持 JSON-RPC batch：batch 行（JSON 数组）会以单条 `-32600 "batch requests are not supported"` 错误行应答，而不是静默。

## Streamable HTTP transport

当客户端连接到一个已经运行的本地或内网 MCP 端点时，使用 HTTP。MCP HTTP 仅支持内嵌部署：`[mcp].embed_in_serve` 默认为 `true`，`pkvsyncd serve` 会在主服务端口挂载 `/mcp`。设置 `[mcp].embed_in_serve = false` 可禁用内嵌端点。独立进程 `pkvsyncd mcp --transport http --bind` 已在本版本移除；原有的独立部署请改为指向主服务的 `/mcp`。

端点路径始终是 `/mcp`，位于主服务 origin：

```text
POST http://127.0.0.1:6710/mcp
GET  http://127.0.0.1:6710/mcp
```

每个请求都必须包含：

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

部署密钥来自与主 PKV Sync 服务相同的配置文件。缺少或错误的部署密钥会在 bearer token 认证前收到 HTTP `404`。

MCP HTTP 使用固定窗口限流，每 60 秒最多 120 次请求。超限时，服务端返回 HTTP `429`，并返回 code 为 `-63001` 的 JSON-RPC error。失败的 MCP bearer-token 认证返回 HTTP `401` 和 code `-63002`，并且同样按 transport 实例在进程内限流：内嵌的 HTTP 端点共享服务进程的限流器，而每个 `pkvsyncd mcp` 进程会自建状态，因而拥有自己的限流器，每 60 秒最多 30 次失败尝试。

MCP 规范中的 `Origin` 请求头 DNS-rebinding 检查未在 `/mcp` 上实现，这是有意为之：该端点要求非环境凭据的部署密钥和 bearer 设备 token（浏览器无法在跨源请求中附带这两者），管理员会话 cookie 会被拒绝，并且没有任何 CORS 策略向浏览器源暴露该端点。如果某个部署关闭了这些前提中的任何一项，就应当为 `/mcp` 自行前置 `Origin` 检查。

## MCP 2026-07-28（现代）客户端

同一个端点同时服务两个协议时代。现代客户端在 `params._meta` 中发送逐请求协议字段（`io.modelcontextprotocol/protocolVersion` 和 `io.modelcontextprotocol/clientCapabilities`），并且必须在 `MCP-Protocol-Version` 请求头中重复版本、在 `Mcp-Method` 请求头中重复方法；当请求携带 `params.name` 时，还要在 `Mcp-Name` 请求头中重复名称（非纯 ASCII 时以 `=?base64?...?=` 形式 base64 编码）。任一请求头与请求体不匹配都会返回 HTTP `400` 和 code `-32020`。请求不支持的版本会返回 HTTP `400` 和 code `-32022`，并附带受支持版本列表（`2026-07-28`）。现代请求中的无效参数（例如 `_meta` 格式错误，或工具 `arguments` 不是对象）会返回 HTTP `400` 和 code `-32602`。未知方法返回 HTTP `404` 和 `-32601`；不含 `id` 的纯通知消息以 HTTP `202` 应答且无响应体，但 `notifications/cancelled` 除外——它返回 `405`：在 Streamable HTTP 上，取消是通过关闭 SSE 响应流完成的。两种 transport 都不支持 JSON-RPC batch：`POST /mcp` 的数组请求体会在 `200` JSON-RPC 信封中返回 code `-32600`（`batch requests are not supported`），与 stdio 对 batch 行的应答是同一条错误，而不是假装接受并返回 `202`。信封始终是旧版的：数组无法携带 `params._meta`，因此永远不会被归类为现代请求。

现代结果带有 `resultType`，`tools/list` 和 `server/discover` 还会额外带有 `ttlMs`/`cacheScope` 缓存提示。`server/discover` 会报告 `supportedVersions`，客户端可以在连接前据此选择协议时代。（仅现代版本；旧版 `initialize` 协商不在此列表内）旧版 `initialize` 客户端仍可使用：服务端会回显所请求的旧版本；若请求的是现代版本 `2026-07-28` 则原样回显，未知版本则回退到最新的旧版本（`2025-11-25`），并以旧版封装应答。

## 变更通知

- **旧版客户端**：携带 `Accept: text/event-stream` 的 GET `/mcp` 订阅 `vault_changed` 通知。每个事件的 id 就是 commit SHA 本身，客户端重连时可作为 `Last-Event-ID` 传回，以 replay 该 vault 错过的 commit。Replay 有上限；如果服务端无法覆盖错过的历史，会发送 `lagged`，客户端应通过同步 API 刷新。服务端无法核实（历史不可读或不可恢复）的续传位置也按同样方式处理，绝不会静默转入直播流。包含 `:` 的值不会被当作续传位置；当 Token 可见多个 vault 时，只有正在续传的那个 vault 位置已知，因此服务端也会为其他 vault 发送 `lagged`。直播产生的 `lagged` 事件在事件 data 中携带受影响的 vault id；回放级别的 `lagged` 事件 data 为空。
- **现代客户端**：POST 一个 `subscriptions/listen` 请求，将 `params.notifications.resourceSubscriptions` 设为 `["pkv://vault/<id>"]` URI（只有 Token 可见的 vault 会被接受）。`resourceSubscriptions` 中重复的 URI 会在确认（acknowledgment）中去重（以首次出现的为准）。响应是一条 SSE 流：先发送回显已接受订阅的 `notifications/subscriptions/acknowledged` 事件，随后发送 `notifications/resources/updated` 事件，其 params 携带 `uri`、`commit`、`event` 和 `_meta.subscriptionId`。如果服务端的事件缓冲区溢出（流消费较慢时一次涌入超过 64 个 commit），listen 流会发送 `notifications/resources/dropped` 通知，携带受影响的 `uri` 和同一个 `_meta` 订阅 id；客户端应当重新读取该 vault 的状态。关闭流即取消订阅。现代流不可续传：没有 `Last-Event-ID` replay，重连后客户端需要重新 listen 并对比读取结果。

除非放在可信网络控制之后，否则请将 HTTP 绑定到 loopback。bearer token 会授予该用户所有笔记库的读写访问权限。

## 读取和搜索上限

`search` 最多扫描 5000 个可见 tree 文件——当 vault 的可见文件超过 5000 个时，调用会以 `too many files to search` 失败，而不是只扫描一部分。它最多返回 500 条匹配（默认 100），并在生产环境已读取文本超过 256 MiB 后报错中止。`link_graph` 最多扫描 5000 个可见文本文件，并使用相同的生产环境文本预算，但会返回 `truncated: true` 而不是失败；单文件链接提取上限为 10000 条、失效链接累计上限为 20000 条，触及任一上限都会置 `truncated: true`。`changes_since` 最多返回 5000 条可见变更项。`read_file` 和 `read_file_at_commit` 会在返回前解析 blob pointer；二进制内容以 base64 返回并带有 `encoding: "base64"`，超过 64 MiB 的二进制/blob 响应会被拒绝，而不是被 base64 展开进 JSON。

## 写入工具

PKV Sync 在读取工具之外提供四个 MCP 写入工具：

- `write_file(vault_id, path, content, parent_commit, dry_run?, expected_hashes?)`：创建或更新文本文件。
- `delete_file(vault_id, path, parent_commit, dry_run?, expected_hashes?)`：删除文件。
- `write_files(vault_id, parent_commit, writes[], deletes[], dry_run?, expected_hashes?)`：在一个 commit 中原子地创建、更新和删除多个文本文件。如果任一路径无效、文件超过 `max_file_size`、批次为空（`empty_batch`），或批次超过 100 个变更（`batch_too_large`），服务端不会提交任何内容。陈旧的 `parent_commit` 会返回常规 `Conflict` 响应。
- `move_file(vault_id, parent_commit, from, to, dry_run?, expected_hashes?)`：在单个 commit 中移动或重命名一个文本文件。它会拒绝已存在的目标（`target_exists`）、二进制／blob-pointer 源文件（`unsupported_binary_move`），以及缺失或隐藏的源文件（`not_found`）。

### Dry run 与内容守卫

两个可选参数让 agent 的写入在真正发生之前可被审查，并且不会被并发的人工编辑破坏。

`dry_run: true` 会校验真实调用会校验的一切（parent 是否仍然最新、路径可见性、大小与批次上限、下面的守卫），并返回 `{"dry_run": true, "would_write": [{"path", "change", "old_path?", "size_bytes"}], "parent_commit"}`，但不提交、不记录活动、不消耗写入额度。移动会报告为一条 `rename` 条目。它不是承诺：位于提交路径之下的检查（文件类型与 exclude glob 强制）不会运行，因此 dry run 可能通过，而真实写入被拒绝。

`expected_hashes: {"<path>": "<sha256>"}` 逐文件守卫——也就是 agent 读取这些文件时 `read_file` 返回的哈希。如果任何被守卫的路径不再匹配，整个调用会被拒绝并报 `content_diverged: path=<p> expected=<sha> actual=<sha|"null">`（二进制目标会被直接拒绝，因为调用方不可能对从未收到过的字节计算哈希），并且不会提交任何内容。这个 map 是增量式的，因此 `deletes[]` 的形态对现有客户端保持不变。`expected_hashes` 在 `parent_commit` 之后校验：父提交过期时仍然返回 `Conflict`，因为客户端无论如何都必须重新读取。

### 乐观并发控制

每次写入都必须提供 `parent_commit`，也就是客户端认为当前笔记库 HEAD 所在的 commit hash。如果客户端上次读取后笔记库已经前进，服务端会返回 `{ "conflict": true, "current_head": "..." }`，并且不会写入。客户端需要重新读取、必要时合并，再用新的 `parent_commit` 重试。

### 限流

写入工具按 `(token, vault)` 组合限流，每分钟最多 60 次写入。`write_files` 整个批次只消耗一次限流记录。读取工具和 SSE 订阅不受这个写入额度影响。

1.2.1 的加固让写入校验保持 fail-closed：`writes[]` 和 `deletes[]` 中归一化后重复的路径会被拒绝，隐藏或排除路径不会泄漏目标存在性，无效的 `move_file` 来源会在消耗写入额度前被拒绝。MCP 鉴权错误保持泛化，Streamable HTTP JSON 请求体上限为 100 MiB。

### 审计记录

每次成功写入、批量写入、移动或删除都会在活动日志中记录为 `mcp_write` 或 `mcp_delete`，details 中包含发起该改动的工具、路径摘要、commit 和 size。管理页会在设备（agent）名旁显示工具，并支持按 `?agent=<设备名>` 与 `?tool=<工具名>` 过滤，因此“哪个 agent 删了它”和“这个 agent 一直在做什么”都能回答。dry run 不记录任何内容，因为什么都没发生。

### 注意：写入会进入 git 历史

AI 驱动的写入会成为笔记库 git 历史中的 commit。你可以通过普通 git 操作回滚，但无法让已经提交的改动“从未发生”；这种可审计性是有意设计。

## Registry

服务端已收录于官方 MCP Registry，名为 `io.github.cyberkurry/pkv-sync`（本仓库内的 `mcp/server.json`），它指向已发布的容器镜像，并用 `PKV_TOKEN` 和 `PKV_VAULT_ID` 配置会话。自托管的服务端也可以按上文命令手工接入。

## 客户端提示

- Claude Code、Codex CLI、Cherry Studio、OpenCode，以及通过桥接使用 MCP 的客户端，都可以通过启动 `pkvsyncd mcp` 使用 stdio 模式。
- 支持 Streamable HTTP 的客户端可以指向 `/mcp`，并在每个请求上发送 bearer auth 和部署密钥。
- 两种 transport 均为双时代：旧版 `initialize` 客户端继续可用，现代 2026-07-28 客户端可获得逐请求版本处理、`server/discover` 和 `subscriptions/listen` 推送。
- 服务端是无状态的，不要求也不返回 `Mcp-Session-Id`。
