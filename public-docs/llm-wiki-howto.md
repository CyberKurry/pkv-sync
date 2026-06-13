# LLM Wiki workflow with PKV Sync

English | [简体中文](./llm-wiki-howto.zh-CN.md) | [繁體中文](./llm-wiki-howto.zh-Hant.md) | [日本語](./llm-wiki-howto.ja.md) | [한국어](./llm-wiki-howto.ko.md)

Document version: v1.4.0.

PKV Sync provides the storage, history, and MCP substrate for an LLM-maintained wiki. Your own MCP-capable agent runs the LLM, reads and writes through a normal PKV Sync device token, and commits every accepted change into the vault's git history.

## Three layers

Use a small, explicit structure so humans and agents can both reason about the vault.

- **Sources**: raw notes, pasted research, imported files, meeting transcripts, and other evidence. Keep them close to the original material and include enough provenance to audit them later.
- **Wiki**: concise pages that explain durable facts, decisions, concepts, people, projects, or processes. These pages link to each other and cite source pages.
- **Schema**: a few conventions that make the wiki lintable, such as required frontmatter, an index page, and a maintenance log.

PKV Sync is the substrate, not the LLM host. The server exposes safe read tools, optimistic write tools, link inspection, and change inspection; the agent you choose decides what to summarize, rewrite, or ask you to confirm.

## Connect an agent

Create or reuse a PKV Sync device token, then point the MCP-capable agent at a single vault with stdio:

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

For agents that support Streamable HTTP, you can expose `/mcp` in embedded or standalone mode and send both the deployment key and bearer token on every request. See the MCP access guide for transport details.

Give the agent a narrow instruction: read the source pages, propose wiki updates, use `parent_commit` from the last read when writing, and stop for human review when facts are uncertain or conflicts appear.

## Recommended schema

Start with this layout and adjust it only when it becomes too small for your workflow:

```text
index.md
log.md
sources/
wiki/
```

Use `index.md` as the map of the wiki:

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

Use `log.md` as a maintenance journal:

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

Use frontmatter on wiki pages to preserve provenance:

```markdown
---
kind: wiki
sources:
  - sources/meeting-2026-06-08.md
  - sources/spec-phase-1.md
updated: 2026-06-08
---

# Project Alpha
```

Source pages can stay raw, but they should name where the information came from:

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent loop

1. Ingest: add or update source material under `sources/`, preserving original wording where possible. When one source fans out into 10-25 source and wiki pages, use `write_files` so the whole ingest lands atomically in one commit.
2. Query: ask the agent to read relevant source and wiki pages, then propose updates under `wiki/`.
3. Write: let the agent use `write_file`, `write_files`, `move_file`, or `delete_file` only after it has a current `parent_commit`. Use `move_file` for page merges, splits, and archival moves so git can report the rename instead of losing history.
4. Lint: run `link_graph` to find orphaned, missing, or ambiguous links; run `changes_since` from the last reviewed commit to summarize what changed.
5. Review: inspect the proposed commits, resolve conflicts, and keep uncertain claims in sources until a human promotes them into wiki pages.

In v1.2.1, this loop is tuned for larger wiki vaults: batch ingests stay atomic
with `write_files`, structural page moves retain history with `move_file`,
link/change tools remain bounded and hide filtered paths, and repeated sync
cycles reuse cached filters, token checks, and scans where possible.

## Lint routine

After each maintenance pass, ask the agent to:

- call `link_graph` with the vault id and report broken links, ambiguous basename links, and new orphaned pages;
- call `changes_since` with the last human-reviewed commit and summarize added, modified, deleted, and renamed pages;
- update `index.md` when durable wiki pages were added;
- append a short entry to `log.md` describing source material, wiki pages changed, and unresolved questions.

Hidden paths remain hidden throughout the workflow. If a path is rejected by SyncPathFilter or an exclude glob, MCP read tools do not report it in file lists, search results, link graphs, or change summaries.
