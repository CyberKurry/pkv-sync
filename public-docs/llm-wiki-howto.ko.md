# PKV Sync를 사용한 LLM Wiki workflow

[English](./llm-wiki-howto.md) | [简体中文](./llm-wiki-howto.zh-CN.md) | [繁體中文](./llm-wiki-howto.zh-Hant.md) | [日本語](./llm-wiki-howto.ja.md) | 한국어

문서 버전: v1.6.1.

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

Streamable HTTP를 지원하는 agents의 경우 `pkvsyncd serve`에 내장된 `/mcp` endpoint를 가리키고 모든 요청에 deployment key와 bearer token을 함께 보낼 수 있습니다. transport details는 MCP access guide를 참조하세요.

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
source_commit: 6f1c2ab0d4e5f6789012345678901234567890ab
updated: 2026-06-08
---

# Project Alpha
```

`source_commit`은 페이지를 마지막으로 조정(reconcile)한 기준 commit입니다. 페이지 조정을 마칠 때마다 기록해 두면 vault가 해당 페이지의 근거가 움직였을 때 알려줄 수 있습니다(아래 [Staleness](#staleness) 참조). `sources:`는 선언했지만 이 값이 없는 페이지는 추측으로 측정되지 않고 "앵커 없음(unanchored)"으로 보고됩니다.

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
3. Write: agent가 current `parent_commit`을 확보한 뒤에만 `write_file`, `write_files`, `move_file`, 또는 `delete_file`을 사용하게 합니다. page merge, split, archival move에는 `move_file`을 사용해 git이 history를 잃지 않고 rename으로 보고할 수 있게 합니다. 큰 재작성 전에는 `dry_run: true`로 `would_write` 계획을 먼저 확인하고, agent가 읽을 때 받은 `sha256` 값을 `expected_hashes`로 넘겨 그 사이 사람이 편집한 페이지가 덮어써지지 않고 `content_diverged`로 거부되게 하세요.
4. Lint: `stale_pages`를 실행해 근거가 움직인 페이지를 얻고(아래 참조), `link_graph`로 orphaned, missing, ambiguous links를 찾고, 마지막 reviewed commit부터 `changes_since`로 변경 사항을 summarize합니다.
5. Review: proposed commits를 inspect하고 conflicts를 resolve하며, 불확실한 claims는 사람이 wiki pages로 promote할 때까지 sources에 남겨 둡니다.

v1.2.1에서는 이 루프가 더 큰 wiki vault에 맞게 조정되었습니다. 일괄 ingest는 `write_files`로 원자적으로 유지되고, 구조적인 페이지 이동은 `move_file`로 기록을 보존하며, link/change tools는 상한을 유지하면서 필터링된 paths를 숨기고, 반복 sync cycles는 가능한 경우 cached filters, token checks, scans를 재사용합니다.

## Staleness

이 workflow가 빠지기 쉬운 실패 모드는 보이지 않는 부패입니다. source가 바뀌어도 그 위에 세운 페이지는 vault 어디에서도 그 사실을 알리지 않은 채 낡은 주장을 계속 내세웁니다. 추가만 하는 습관은 상황을 더 나쁘게 만듭니다. 오직 커지기만 하는 knowledge base는 낡은 주장을 조용히 쌓아 갑니다.

`stale_pages`는 그 질문의 기계적인 절반에 답합니다. vault id를 넘기면 모든 페이지의 `sources:`/`source_commit:`을 읽고, 그 앵커를 현재 head와 diff하여 근거가 움직인 페이지를 반환합니다.

- `changed_sources`: source의 내용이 변경되었습니다.
- `removed_sources`: source가 사라졌습니다(이동한 파일을 인용하는 페이지도 여기에 해당합니다).
- `renamed`: 이동한 source의 `{from, to}`입니다. 페이지가 아직 옛 path를 인용하고 있기 때문입니다.
- `unresolved_sources`: 서버가 확인할 수 없는 인용 source입니다(hidden, excluded 또는 존재한 적 없음). `sources:`의 오타는 깨끗한 결과 뒤에 숨지 않고 여기에 나타납니다.
- `missing_source_commit`: sources를 선언했지만 아직 앵커가 없는 페이지입니다.
- `verified: false`: diff가 목록 상한에 도달했으므로 페이지에 표시된 것보다 더 많은 변경이 있을 수 있습니다. 검증되지 않은 페이지는 깨끗한 것이 아니라 확인이 필요하다고 다루세요.

추가가 아니라 조정(reconcile)하세요. 현재 최선의 답을 독자가 가장 먼저 읽게 되도록 페이지를 다시 쓰고, 새 source가 옛 주장을 대체하면 명시적으로 밝히세요(옛 주장을 조용히 지우지 말고, 무엇이 대체했는지를 가리키는 포인터와 함께 남깁니다). 그런 다음 해당 페이지의 `source_commit`을 조정한 head로 갱신하고, 이번 pass를 `log.md`에 기록하세요.

## Lint 루틴

각 maintenance pass 이후 agent에게 다음을 요청하세요.

- vault id와 함께 `stale_pages`를 호출해 보고된 각 페이지를 조정하고(`unresolved_sources`의 오타와 최초 앵커가 필요한 `missing_source_commit` 페이지 포함), 끝난 페이지마다 `source_commit`을 되씁니다.
- vault id와 함께 `link_graph`를 호출하고 broken links, ambiguous basename links, new orphaned pages를 보고합니다. `kind: "wiki"`를 주면 wiki 층만 lint할 수 있습니다.
- 마지막 human-reviewed commit과 함께 `changes_since`를 호출하고 added, modified, deleted, renamed pages를 summarize합니다. `truncated: true`는 목록이 잘렸다는 표시이므로, "다른 변경은 없다"고 결론내기 전에 더 좁은 범위로 다시 실행하세요.
- durable wiki pages가 추가되었으면 `index.md`를 업데이트합니다.
- source material, 변경된 wiki pages, unresolved questions를 설명하는 짧은 entry를 `log.md`에 추가합니다.

Hidden paths는 workflow 전체에서 hidden 상태로 유지됩니다. 어떤 path가 SyncPathFilter 또는 exclude glob에 의해 거부되면 MCP read tools는 file lists, search results, link graphs, change summaries에서 해당 path를 보고하지 않습니다.
