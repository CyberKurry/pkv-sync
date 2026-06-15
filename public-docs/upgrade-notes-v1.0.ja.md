# Upgrade notes: 0.x から 1.0 へ

[English](./upgrade-notes-v1.0.md) | [简体中文](./upgrade-notes-v1.0.zh-CN.md) | [繁體中文](./upgrade-notes-v1.0.zh-Hant.md) | 日本語 | [한국어](./upgrade-notes-v1.0.ko.md)

ドキュメントバージョン: v1.4.2。

PKV Sync 1.0 は最初の stable release です。同時に、今後の 1.x maintenance のために
SQLite migration baseline を reset します。

## 重要な database note

PKV Sync 1.0 は単一の `0001_initial.sql` baseline migration を出荷します。
0.x release で作成された SQLite database は 1.0.0 へインプレース upgrade できません。

0.x server を運用している場合は、次のいずれかの経路を選んでください。

1. 既存 deployment は移行準備の backup、materialize、export に必要な間だけ、最終 0.8.x patch release にとどめる。
2. 各 vault を backup または materialize し、新しい 1.0 data directory で起動し、
   user と vault を作り直してから contents を新 server へ import または push する。
3. migration rehearsal を試す前に、0.x data root の完全な `pkvsyncd backup` を保管する。

既存の 0.x `metadata.db` に 1.0 binary や Docker image を直接向けないでください。

## 1.0 が安定化する surface

1.0 以後、次の surface は semantic versioning に従います。

- `public-docs/openapi.yaml` に記載された public REST routes。
- MCP how-to に記載された MCP stdio と Streamable HTTP tool behavior。
- 1.x fresh database 用 SQLite migrations。今後の 1.x migrations はこの v1 baseline 以後
  append-only です。
- vault ごとの git repository layout と content-addressed blob storage。
- CLI subcommand と既存 flag。
- Obsidian plugin settings と sync behavior。通常の backward-compatible な 1.x feature
  addition はあります。

OpenAPI に記載されていない route、たとえば Admin Web UI form handler は internal
implementation detail です。

## 推奨される 0.x から 1.0 への手順

1. 可能であれば、旧 deployment をまず最終 0.8.x patch release へ更新し、backup、materialize、export の準備にだけ使います。
2. `pkvsyncd backup --output <backup-dir>` を実行し、結果を安全に保管します。
3. 各 vault について、最新の Obsidian client、`git clone`、または
   `pkvsyncd materialize <vault-id> --output <dir>` で現在の file tree を作成します。
4. 旧 server を停止します。
5. 空の `data_dir` と `metadata.db` で PKV Sync 1.0 を起動します。
6. `/setup` を完了し、user と vault を作り直してから、materialized vault contents を
   push または import します。
7. user に Obsidian plugin を 1.0.0 へ更新してもらいます。

## Plugin compatibility

1.0 server で supported plugin となるのは、server に bundled された 1.0 Obsidian plugin です。
古い v0.8.x plugin も core sync API は同じですが、新しい修正と self-update hardening は
1.0+ でのみ維持されます。

## 0.x からの breaking changes

- migration が単一の v1 baseline に squash されたため、0.x SQLite database は
  in-place upgrade されません。
- first-run setup は browser-based のままです。fresh server は random admin password を
  log に出力しません。

vault file contents、git history、blob は backup/materialize/recreate/import workflow で
持ち越せます。

## Known caveats

- native per-vault E2EE は 1.0 の範囲外です。client-side encrypted file contents が
  今すぐ必要で、plaintext path を受け入れられる場合は
  [`git-crypt`](./git-crypt-howto.ja.md) を使ってください。
- `/metrics` は default で disabled で、有効化しても production authentication gates が必要です。
- production では `public_host` を設定してください。configured HTTPS public origin を決定できない場合、
  admin POST は意図的に fail-closed します。
