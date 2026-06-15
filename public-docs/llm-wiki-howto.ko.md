# PKV Sync를 사용한 LLM Wiki workflow

[English](./llm-wiki-howto.md) | [简体中文](./llm-wiki-howto.zh-CN.md) | [繁體中文](./llm-wiki-howto.zh-Hant.md) | [日本語](./llm-wiki-howto.ja.md) | 한국어

문서 버전: v1.4.3.

이 문서는 기계 번역을 바탕으로 다듬은 한국어 문서입니다. 어색한 표현이나 의미가 모호한 부분이 있으면 영어 원문을 함께 확인하세요.

PKV Sync는 LLM이 유지 관리하는 wiki를 위한 storage, history, MCP substrate를 제공합니다. 사용자가 선택한 MCP-capable agent가 LLM을 실행하고, 일반 PKV Sync device token을 통해 읽고 쓰며, 승인된 모든 변경 사항을 vault의 git history에 commit합니다.

## 세 가지 계층

사람과 agent가 모두 vault를 추론할 수 있도록 작고 명시적인 구조를 사용하세요.

- **Sources**: 원본 notes, 붙여넣은 research, imported files, meeting transcripts, 그 밖의 evidence입니다. 원자료에 가깝게 보관하고 나중에 audit할 수 있도록 충분한 provenance를 포함하세요.
- **Wiki**: durable facts, decisions, concepts, people, projects, processes를 간결하게 설명하는 pages입니다. 이 pages는 서로 link하고 source pages를 cite합니다.
- **Schema**: required frontmatter, index page, maintenance log처럼 wiki를 lintable하게 만드는 몇 가지 conventions입니다.

PKV Sync는 substrate이지 LLM host가 아닙니다. 서버는 safe read tools, optimistic write tools, link inspection, change inspection을 노출합니다. 무엇을 summarize, rewrite하거나 사용자에게 confirmation을 요청할지는 사용자가 선택한 agent가 결정합니다.

## Agent 연결

PKV Sync device token을 만들거나 재사용한 뒤, MCP-capable agent가 stdio로 하나의 vault를 가리키게 하세요.

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

Streamable HTTP를 지원하는 agents의 경우 embedded 또는 standalone mode로 `/mcp`를 노출하고 모든 요청에 deployment key와 bearer token을 함께 보낼 수 있습니다. transport details는 MCP access guide를 참조하세요.

agent에는 좁은 instruction을 주세요. source pages를 읽고, wiki updates를 제안하고, 쓸 때는 마지막 read에서 얻은 `parent_commit`을 사용하며, facts가 불확실하거나 conflicts가 나타나면 human review를 위해 멈추도록 지시합니다.

## 권장 schema

다음 layout으로 시작하고, workflow에 비해 너무 작아졌을 때만 조정하세요.

```text
index.md
log.md
sources/
wiki/
```

`index.md`는 wiki의 map으로 사용합니다.

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

`log.md`는 maintenance journal로 사용합니다.

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

wiki pages에는 provenance를 보존하기 위해 frontmatter를 사용합니다.

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

Source pages는 raw 상태로 둘 수 있지만, 정보의 출처를 명시해야 합니다.

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent 루프

1. Ingest: `sources/` 아래 source material을 추가하거나 업데이트하되, 가능하면 원문 표현을 보존합니다. 하나의 source가 10-25개의 source 및 wiki pages로 확장될 때는 `write_files`를 사용해 전체 ingest가 하나의 atomic commit으로 저장되게 합니다.
2. Query: agent에게 관련 source 및 wiki pages를 읽게 한 다음 `wiki/` 아래 updates를 제안하게 합니다.
3. Write: agent가 current `parent_commit`을 확보한 뒤에만 `write_file`, `write_files`, `move_file`, 또는 `delete_file`을 사용하게 합니다. page merge, split, archival move에는 `move_file`을 사용해 git이 history를 잃지 않고 rename으로 보고할 수 있게 합니다.
4. Lint: `link_graph`를 실행해 orphaned, missing, ambiguous links를 찾고, 마지막 reviewed commit부터 `changes_since`를 실행해 변경 사항을 summarize합니다.
5. Review: proposed commits를 inspect하고 conflicts를 resolve하며, 불확실한 claims는 사람이 wiki pages로 promote할 때까지 sources에 남겨 둡니다.

v1.2.1에서는 이 루프가 더 큰 wiki vault에 맞게 조정되었습니다. 일괄 ingest는 `write_files`로 원자적으로 유지되고, 구조적인 페이지 이동은 `move_file`로 기록을 보존하며, link/change tools는 상한을 유지하면서 필터링된 paths를 숨기고, 반복 sync cycles는 가능한 경우 cached filters, token checks, scans를 재사용합니다.

## Lint 루틴

각 maintenance pass 이후 agent에게 다음을 요청하세요.

- vault id와 함께 `link_graph`를 호출하고 broken links, ambiguous basename links, new orphaned pages를 보고합니다.
- 마지막 human-reviewed commit과 함께 `changes_since`를 호출하고 added, modified, deleted, renamed pages를 summarize합니다.
- durable wiki pages가 추가되었으면 `index.md`를 업데이트합니다.
- source material, 변경된 wiki pages, unresolved questions를 설명하는 짧은 entry를 `log.md`에 추가합니다.

Hidden paths는 workflow 전체에서 hidden 상태로 유지됩니다. 어떤 path가 SyncPathFilter 또는 exclude glob에 의해 거부되면 MCP read tools는 file lists, search results, link graphs, change summaries에서 해당 path를 보고하지 않습니다.
