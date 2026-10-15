# PKV Sync で作る LLM Wiki ワークフロー

[English](./llm-wiki-howto.md) | [简体中文](./llm-wiki-howto.zh-CN.md) | [繁體中文](./llm-wiki-howto.zh-Hant.md) | 日本語 | [한국어](./llm-wiki-howto.ko.md)

ドキュメントバージョン: v1.6.1。

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

Streamable HTTP をサポートする agent では、`pkvsyncd serve` に組み込みで提供される `/mcp` endpoint を指し、すべての request に deployment key と bearer token を送信できます。transport の詳細は MCP access guide を参照してください。

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
source_commit: 6f1c2ab0d4e5f6789012345678901234567890ab
updated: 2026-06-08
---

# Project Alpha
```

`source_commit` は、そのページを最後に整合（reconcile）した基準 commit です。ページの整合が済んだら必ず書き込みます。そうすれば vault は、そのページの根拠が動いたときに知らせてくれます（後述の [Staleness](#staleness) を参照）。これを書かずに `sources:` だけを宣言しているページは、推測と比較されるのではなく「アンカー無し」として報告されます。

Source pages は raw のままにできますが、情報の origin を明記します。

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent loop

1. Ingest: `sources/` の下に source material を追加または更新し、可能な限り元の wording を保ちます。1 つの source が 10-25 個の source pages と wiki pages に展開される場合は、`write_files` を使い、ingest 全体を 1 つの atomic commit として保存します。
2. Query: agent に関連する source pages と wiki pages を読ませ、`wiki/` の更新案を出させます。
3. Write: agent が current `parent_commit` を持っている場合だけ、`write_file`、`write_files`、`move_file`、または `delete_file` を使わせます。page merge、split、archive move には `move_file` を使い、git が history を失わず rename として報告できるようにします。大きな書き換えでは、まず `dry_run: true` を要求して `would_write` の計画をレビューします。`expected_hashes` には agent が読み取ったときの `sha256` を渡し、その間に人間が編集したページは上書きされず `content_diverged` として拒否されるようにします。
4. Lint: `stale_pages` を実行して根拠が動いたページを取得し（後述）、`link_graph` で orphaned、missing、ambiguous links を探し、最後に人間が review した commit から `changes_since` を実行して変更内容を summarize します。
5. Review: proposed commits を inspect し、conflicts を resolve し、不確かな claims は人間が wiki pages に promote するまで sources に残します。

v1.2.1 では、この loop がより大きな wiki vault 向けに調整されています。一括 ingest は `write_files` で原子的に保たれ、構造的なページ移動は `move_file` で履歴を保持し、link/change tools は上限付きのままフィルター済み path を隠し、繰り返しの sync cycles は可能な限り cached filters、token checks、scans を再利用します。

## Staleness

このワークフローが陥りやすい失敗モードは、目に見えない腐敗です。source が変わり、それを基にしたページは古い主張を唱え続けているのに、vault のどこにもそのことが書かれていません。追記のみという習慣はこれを悪化させます。成長するだけの knowledge base は、古くなった主張を静かに蓄積していきます。

`stale_pages` はその問いの機械的な半分に答えます。vault id を渡すと、すべてのページの `sources:`/`source_commit:` を読み、そのアンカーを現在の head と diff し、根拠が動いたページを返します。

- `changed_sources`: source の内容が変更された。
- `removed_sources`: source が失われた（移動したファイルを cite しているページもここに入ります）。
- `renamed`: 移動した source の `{from, to}`。ページがまだ古い path を cite しているためです。
- `unresolved_sources`: サーバーが検証できない cite 済みの source。hidden、excluded、またはそもそも存在しないもの。`sources:` の綴り間違いは、クリーンな結果に隠れるのではなくここに現れます。
- `missing_source_commit`: source は宣言しているが、まだアンカーがないページ。
- `verified: false`: diff が一覧の上限に達したため、ページには表示されている以上の変更がある可能性があります。未検証のページはクリーンではなく、確認が必要なものとして扱ってください。

追記ではなく整合（reconcile）します。現在の最良の答えが読者に最初に届くようにページを書き直し、新しい source が古い主張を置き換える場合はそれを明示します（古い主張は黙って削除せず、何が置き換えたかを指すポインタとともに残します）。その後、そのページの `source_commit` を今回整合した head に更新し、`log.md` にその回の記録を残します。

## Lint routine

各 maintenance pass の後、agent に次を依頼します。

- vault id を指定して `stale_pages` を呼び出し、報告された各ページを整合する（`unresolved_sources` の綴り間違いと、最初のアンカーが必要な `missing_source_commit` のページを含む）。終わったページには `source_commit` を書き戻す。
- vault id を指定して `link_graph` を呼び出し、broken links、ambiguous basename links、新しい orphaned pages を報告する。`kind: "wiki"` を付けると wiki 層だけを lint できる。
- 最後に human-reviewed された commit を指定して `changes_since` を呼び出し、added、modified、deleted、renamed pages を summarize する。`truncated: true` は一覧が打ち切られた印なので、「他に変更は無い」と結論する前に、より狭い範囲で再実行する。
- durable wiki pages が追加された場合は `index.md` を更新する。
- source material、変更された wiki pages、unresolved questions を説明する短い entry を `log.md` に追加する。

Hidden paths は workflow 全体で hidden のままです。path が SyncPathFilter または exclude glob に拒否された場合、MCP read tools は file lists、search results、link graphs、change summaries でその path を報告しません。
