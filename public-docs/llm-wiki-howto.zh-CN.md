# 使用 PKV Sync 的 LLM Wiki 工作流

[English](./llm-wiki-howto.md) | 简体中文 | [繁體中文](./llm-wiki-howto.zh-Hant.md) | [日本語](./llm-wiki-howto.ja.md) | [한국어](./llm-wiki-howto.ko.md)

文档版本：v1.7.1。

PKV Sync 为由 LLM 维护的 wiki 提供存储、历史和 MCP 基底。你自己的 MCP-capable agent 负责运行 LLM，通过普通的 PKV Sync 设备 token 读写，并把每个已接受的改动提交到笔记库的 git 历史中。

## 三层结构

使用一个小而明确的结构，让人类和 agent 都能理解笔记库。

- **Sources**：原始笔记、粘贴的研究材料、导入文件、会议记录，以及其他证据。尽量贴近原始材料，并包含足够的来源信息，方便以后审计。
- **Wiki**：简洁页面，用来解释长期有效的事实、决策、概念、人物、项目或流程。这些页面彼此链接，并引用 source 页面。
- **Schema**：少量约定，让 wiki 可以被 lint，例如必需的 frontmatter、索引页和维护日志。

PKV Sync 是基底，不是 LLM host。服务端暴露安全读取工具、乐观写入工具、链接检查和变更检查；你选择的 agent 决定要总结、重写哪些内容，或何时请你确认。

## 连接 agent

创建或复用一个 PKV Sync 设备 token，然后通过 stdio 将 MCP-capable agent 指向单个笔记库：

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

对于支持 Streamable HTTP 的 agent，你可以让它指向 `pkvsyncd serve` 内嵌提供的 `/mcp` 端点，并在每个请求中同时发送部署密钥和 bearer token。transport 细节请参见 MCP access guide。

给 agent 一个范围很窄的指令：读取 source 页面、提出 wiki 更新、写入时使用上次读取得到的 `parent_commit`，并在事实不确定或出现冲突时停下来等待人工 review。

## 推荐 schema

从这个布局开始，只有当它对你的工作流来说太小时再调整：

```text
index.md
log.md
sources/
wiki/
```

使用 `index.md` 作为 wiki 地图：

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

使用 `log.md` 作为维护日志：

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

在 wiki 页面上使用 frontmatter 来保留来源：

```markdown
---
kind: wiki
sources:
  - sources/meeting-2026-06-08.md
  - sources/spec-phase-1.md
source_commit: 6f1c2ab0d4e5f6789012345678901234567890ab
updated: 2026-06-08
---

# Project Alpha
```

`source_commit` 是该页面上次调和所依据的 commit——每当调和完一页就写上它，笔记库随后便能告诉你该页面的证据何时发生了移动（见下文 [Staleness](#staleness)）。声明了 `sources:` 却没有它的页面会被报为未锚定，而不是拿猜测去度量。

Source 页面可以保持原始状态，但应说明信息来自哪里：

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent 循环

1. Ingest：在 `sources/` 下新增或更新 source 材料，尽量保留原始措辞。当一个 source 会展开成 10 到 25 个 source 和 wiki 页面时，使用 `write_files`，让整个 ingest 以一个原子 commit 落地。
2. Query：要求 agent 读取相关 source 和 wiki 页面，然后提出 `wiki/` 下的更新。
3. Write：只有在 agent 拿到当前 `parent_commit` 后，才允许它使用 `write_file`、`write_files`、`move_file` 或 `delete_file`。页面合并、拆分和归档移动时使用 `move_file`，让 git 能报告重命名，而不是丢失历史。大改写时先要求 `dry_run: true` 并审查 `would_write` 计划；把 agent 读取时得到的 `sha256` 作为 `expected_hashes` 传入，这样期间被人改过的页面会被拒绝（`content_diverged`）而不是被覆盖。
4. Lint：先运行 `stale_pages` 拿到来源已移动的页面清单（见下文），再运行 `link_graph` 查找孤立链接、缺失链接或有歧义的链接，最后从上次 review 过的 commit 开始运行 `changes_since`，总结发生了什么变化。
5. Review：检查提出的 commit，解决冲突，并把不确定的主张留在 sources 中，直到人类将其提升为 wiki 页面。

在 v1.2.1 中，这个循环更适合大型 wiki 笔记库：批量 ingest 继续通过 `write_files` 保持原子性，结构性的页面移动通过 `move_file` 保留历史，链接和变更工具保持有界并隐藏被过滤路径，重复同步周期会尽可能复用过滤器、token 检查和扫描结果缓存。

## Staleness

这个工作流容易出现的失败模式是“不可见的腐化”：source 变了，建立在它之上的页面却继续断言旧论断，而笔记库里没有任何东西说明这一点。只追加的习惯会让它更糟——只增不减的知识库会悄无声息地积累过期论断。

`stale_pages` 回答这个问题中机械的那一半。给它 vault id，它会读取每个页面的 `sources:`/`source_commit:`，把该锚点与当前 head 做 diff，并返回证据已经移动的页面：

- `changed_sources`：source 的内容变了；
- `removed_sources`：source 已不存在（引用了被移动文件的页面也会落在这里）；
- `renamed`：被移动的 source 的 `{from, to}`，因为页面仍然引用旧路径；
- `unresolved_sources`：服务端无法核实的被引用 source——隐藏、被排除或从未存在过。`sources:` 里的拼写错误会出现在这里，而不是藏在一个干净结果后面；
- `missing_source_commit`：声明了 sources 却还没有锚点的页面；
- `verified: false`：diff 触及了清单上限，因此页面可能还有比展示出来更多的变化。把未核实的页面当作需要检查，而不是当作干净。

调和，而不是追加：重写页面，让当前最佳答案成为读者最先读到的内容；当更新的 source 取代旧论断时，要显式说明（保留旧论断，并加上指向取代它内容的指针，而不是静默删除）。然后把该页面的 `source_commit` 更新为本次调和所依据的 head，并在 `log.md` 中记录这一轮。

## Lint 例行流程

每次维护完成后，请 agent：

- 用 vault id 调用 `stale_pages`，逐页调和它报告的每一页（包括 `unresolved_sources` 里的拼写错误，以及需要首次锚定的 `missing_source_commit` 页面），并为它完成的每一页写回 `source_commit`；
- 用 vault id 调用 `link_graph`，并报告断链、有歧义的 basename 链接，以及新增的孤立页面；可加 `kind: "wiki"` 只 lint wiki 层；
- 用上次人工 review 过的 commit 调用 `changes_since`，并总结新增、修改、删除和重命名的页面；结果带 `truncated: true` 表示清单被裁剪，需以更窄范围重跑，再下“没有其它变化”的结论；
- 当新增了长期有效的 wiki 页面时，更新 `index.md`；
- 向 `log.md` 追加一条简短记录，说明 source 材料、改动过的 wiki 页面，以及未解决的问题。

隐藏路径会在整个工作流中保持隐藏。如果某个路径被 SyncPathFilter 或 exclude glob 拒绝，MCP 读取工具不会在文件列表、搜索结果、链接图或变更摘要中报告它。
