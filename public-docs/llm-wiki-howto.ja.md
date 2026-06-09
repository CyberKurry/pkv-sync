# PKV Sync で作る LLM Wiki ワークフロー

[English](./llm-wiki-howto.md) | [简体中文](./llm-wiki-howto.zh-CN.md) | [繁體中文](./llm-wiki-howto.zh-Hant.md) | 日本語 | [한국어](./llm-wiki-howto.ko.md)

ドキュメントバージョン: v1.1.1。

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

PKV Sync は、LLM が保守する wiki のための storage、history、MCP substrate を提供します。ユーザー自身の MCP 対応 agent が LLM を実行し、通常の PKV Sync device token で読み書きし、承認されたすべての変更を vault の git history に commit します。

## 3 つの層

人間と agent の両方が vault を理解できるように、小さく明示的な構造を使います。

- **Sources**: raw notes、貼り付けた research、imported files、meeting transcripts、その他の evidence。原資料に近い形で残し、後から audit できるだけの provenance を含めます。
- **Wiki**: durable facts、decisions、concepts、people、projects、processes を簡潔に説明する pages。これらの pages は互いに link し、source pages を cite します。
- **Schema**: wiki を lintable にする少数の conventions。required frontmatter、index page、maintenance log などです。

PKV Sync は substrate であり、LLM host ではありません。server は safe read tools、optimistic write tools、link inspection、change inspection を公開します。何を summarize し、rewrite し、確認を求めるかは、ユーザーが選ぶ agent が判断します。

## Agent を接続する

PKV Sync device token を作成または再利用し、stdio で MCP 対応 agent を単一の vault に向けます。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

Streamable HTTP をサポートする agent では、embedded または standalone mode で `/mcp` を公開し、すべての request に deployment key と bearer token を送信できます。transport の詳細は MCP access guide を参照してください。

agent には狭い instruction を渡します。source pages を読み、wiki updates を提案し、書き込み時には最後の read で得た `parent_commit` を使い、facts が不確かまたは conflicts が出たら human review のために停止するよう指示します。

## 推奨 schema

まずこの layout から始め、workflow に対して小さすぎると感じたときだけ調整します。

```text
index.md
log.md
sources/
wiki/
```

`index.md` は wiki の map として使います。

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

`log.md` は maintenance journal として使います。

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

wiki pages には provenance を残すために frontmatter を使います。

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

Source pages は raw のままにできますが、情報の origin を明記します。

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent loop

1. Ingest: `sources/` の下に pages を追加または更新し、可能な限り元の wording を保ちます。
2. Query: agent に関連する source pages と wiki pages を読ませ、`wiki/` の更新案を出させます。
3. Write: agent が current `parent_commit` を持っている場合だけ、`write_file` または `delete_file` を使わせます。
4. Lint: `link_graph` を実行して orphaned、missing、ambiguous links を探し、最後に人間が review した commit から `changes_since` を実行して変更内容を summarize します。
5. Review: proposed commits を inspect し、conflicts を resolve し、不確かな claims は人間が wiki pages に promote するまで sources に残します。

## Lint routine

各 maintenance pass の後、agent に次を依頼します。

- vault id を指定して `link_graph` を呼び出し、broken links、ambiguous basename links、新しい orphaned pages を報告する。
- 最後に human-reviewed された commit を指定して `changes_since` を呼び出し、added、modified、deleted、renamed pages を summarize する。
- durable wiki pages が追加された場合は `index.md` を更新する。
- source material、変更された wiki pages、unresolved questions を説明する短い entry を `log.md` に追加する。

Hidden paths は workflow 全体で hidden のままです。path が SyncPathFilter または exclude glob に拒否された場合、MCP read tools は file lists、search results、link graphs、change summaries でその path を報告しません。
