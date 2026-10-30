# LLM Wiki workflow with PKV Sync

English | [简体中文](./llm-wiki-howto.zh-CN.md) | [繁體中文](./llm-wiki-howto.zh-Hant.md) | [日本語](./llm-wiki-howto.ja.md) | [한국어](./llm-wiki-howto.ko.md)

Document version: v1.7.1.

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

For agents that support Streamable HTTP, point them at the `/mcp` endpoint embedded in `pkvsyncd serve` and send both the deployment key and bearer token on every request. See the MCP access guide for transport details.

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
source_commit: 6f1c2ab0d4e5f6789012345678901234567890ab
updated: 2026-06-08
---

# Project Alpha
```

`source_commit` is the commit the page was last reconciled against — write it
whenever you finish reconciling a page, and the vault can then tell you when
the page's evidence moved (see [Staleness](#staleness) below). Pages that
declare `sources:` without it are reported as unanchored rather than measured
against a guess.

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
3. Write: let the agent use `write_file`, `write_files`, `move_file`, or `delete_file` only after it has a current `parent_commit`. Use `move_file` for page merges, splits, and archival moves so git can report the rename instead of losing history. For a large rewrite, ask for `dry_run: true` first and review the `would_write` plan; pass `expected_hashes` with the `sha256` values from the agent's reads so a page a human edited in the meantime is refused (`content_diverged`) instead of overwritten.
4. Lint: run `stale_pages` to get the pages whose sources moved (below), `link_graph` to find orphaned, missing, or ambiguous links, and `changes_since` from the last reviewed commit to summarize what changed.
5. Review: inspect the proposed commits, resolve conflicts, and keep uncertain claims in sources until a human promotes them into wiki pages.

In v1.2.1, this loop is tuned for larger wiki vaults: batch ingests stay atomic
with `write_files`, structural page moves retain history with `move_file`,
link/change tools remain bounded and hide filtered paths, and repeated sync
cycles reuse cached filters, token checks, and scans where possible.

## Staleness

The failure mode this workflow is prone to is invisible decay: a source
changes, and the pages built on it keep asserting the old claim with nothing
in the vault saying so. Append-only habits make it worse — knowledge bases
that only ever grow accumulate stale claims quietly.

`stale_pages` answers the mechanical half of that question. Give it the vault
id and it reads every page's `sources:`/`source_commit:`, diffs that anchor
against the current head, and returns the pages whose evidence moved:

- `changed_sources`: the source's content changed;
- `removed_sources`: the source is gone (a page citing a moved file lands here too);
- `renamed`: `{from, to}` for a moved source, because the page still cites the old path;
- `unresolved_sources`: a cited source the server cannot check — hidden, excluded, or never present. A typo in `sources:` shows up here instead of hiding behind a clean result;
- `missing_source_commit`: pages that declare sources but no anchor yet;
- `verified: false`: the diff hit its listing cap, so the page may have more changes than shown. Treat an unverified page as needing a look, not as clean.

Reconcile, don't append: rewrite the page so the current best answer is what a
reader meets first, and when a newer source supersedes an older claim, say so
explicitly (keep the old claim with a pointer to what replaced it, rather than
deleting it silently). Then update that page's `source_commit` to the head you
reconciled against, and record the pass in `log.md`.

## Lint routine

After each maintenance pass, ask the agent to:

- call `stale_pages` with the vault id, reconcile every page it reports (including the `unresolved_sources` typos and the `missing_source_commit` pages that need a first anchor), and set `source_commit` on each page it finishes;
- call `link_graph` with the vault id and report broken links, ambiguous basename links, and new orphaned pages; add `kind: "wiki"` to lint the wiki layer alone;
- call `changes_since` with the last human-reviewed commit and summarize added, modified, deleted, and renamed pages; a result with `truncated: true` means the list was clipped, so re-run it against a narrower range before concluding nothing else changed;
- update `index.md` when durable wiki pages were added;
- append a short entry to `log.md` describing source material, wiki pages changed, and unresolved questions.

Hidden paths remain hidden throughout the workflow. If a path is rejected by SyncPathFilter or an exclude glob, MCP read tools do not report it in file lists, search results, link graphs, or change summaries.
