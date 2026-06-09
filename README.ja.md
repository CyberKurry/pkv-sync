# PKV Sync

**Obsidian ボールトをセルフホストで。** PKV Sync は自前のサーバー上で動き、
スマホ、タブレット、デスクトップの間で Obsidian ボールトを同期し続けます。
バイナリひとつ、SQLite データベースひとつ、ボールトごとに bare な Git
リポジトリひとつ — クラスターも S3 もマネージドクラウドも不要です。
インストールして、Obsidian から指し示せば、ノートが同期されます。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

ドキュメントバージョン: v1.1.1。

[English](./README.md) | [简体中文](./README.zh-CN.md) | [繁體中文](./README.zh-Hant.md) | 日本語 | [한국어](./README.ko.md)

## 機能

- **マルチユーザー、マルチボールト**の同期。認証済みデバイス越しに、
  ボールト単位の push lock と冪等リトライ付きで動きます。
- **リアルタイム push**。小さな編集は Server-Sent Events 経由で 1 秒未満で
  届きます。ポーリングは保険として残ります。
- **Git が信頼できる唯一の情報源**。すべてのボールトは bare な Git
  リポジトリなので、ファイル単位の履歴、unified diff、単一ファイルの
  復元が標準で動きます — プラグインでも管理パネルでも。
- **コンフリクトに強い**。プラグインはローカル編集を黙って上書きしません。
  コンフリクトは `.conflict-*` ファイルとして見える化され、ワンクリック
  リゾルバで解決できます。
- **管理パネル**は 5 言語対応（English、简中、繁中、日本語、한국어）。
  ユーザー、デバイストークン、ボールト、招待、アクティビティ、blob GC を
  ここから操作し、破壊的なボールト操作とユーザー操作には確認ダイアログを表示します。
- **AI から読める vault**。MCP は stdio、独立した Streamable HTTP、または `pkvsyncd serve` に埋め込まれた `/mcp` ルートで read/write tools を公開します。
- **既定で上限付き**。管理者が作成／リセットするパスワードは setup と同じ強度ポリシーを使い、token の平文は一度だけ表示され、upload と MCP response はサイズ上限で守られ、live SSE stream は取り消された token を再検証します。
- **退屈なつくりは意図的**。バイナリひとつ、SQLite メタデータ DB ひとつ、
  ボールトごとに bare Git リポジトリひとつ、添付ごとに content-addressed な
  blob ひとつ。

## Docker Compose ですぐ始める

これが推奨ルートです。`deploy/caddy/` の Caddy が Let's Encrypt で HTTPS を
さばき、PKV Sync は compose ネットワーク内の `127.0.0.1:6710` で待ち受け、
公開インターネットからの平文 HTTP には一切触れません。

ドメイン名（例：`sync.example.com`）の A/AAAA レコードをサーバーに向けて
おき、ポート `80` と `443` をインターネットから到達可能にしてください
（ポート 80 は ACME HTTP-01 検証に必要です）。

1. デプロイメントキーを生成します。

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` の隣に `config.toml` を置きます。

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey の出力に置き換える
   public_host    = "sync.example.com"   # 必須、admin POST が通るようになります

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker ブリッジネットワーク

   [mcp]
   embed_in_serve = false                # true でこのサーバーに /mcp をマウント
   ```

3. `deploy/caddy/Caddyfile` を編集し、`sync.example.com` を実際のドメインに
   置き換えます。

4. スタックを立ち上げます。

   ```bash
   docker compose up -d
   ```

   ブラウザで `https://sync.example.com/setup` を開き、最初の管理者アカウントを
   作成します。

5. Obsidian に `pkv-sync-plugin.zip` をインストール
   （`<vault>/.obsidian/plugins/pkv-sync/`）し、有効化したあと、管理パネルの
   共有 URL を貼り付け、ログインまたは登録してボールトを選びます。

更新は `docker compose pull && docker compose up -d` です。ネイティブ
インストール、リバースプロキシのチューニング（Caddy / Nginx / Traefik）、
`public_host` の意味、バックアップ／リストア、ディスク暗号化については
[デプロイ強化ガイド](./public-docs/deployment-hardening.ja.md) を参照してください。

## MCP デプロイモード

PKV Sync は MCP Streamable HTTP transport を 2 通りで公開できます。埋め込み
モードは明示的に有効化します。`[mcp].embed_in_serve = true` を設定すると、
`pkvsyncd serve` がメインサーバーポートに `/mcp` をマウントし、同じ TLS
終端、リバースプロキシ、デプロイメントキー、bearer token 検証を共有します。
スタンドアロンモードは従来どおり別プロセスです: `pkvsyncd mcp --transport
http --bind 127.0.0.1:6711`。MCP を隔離したい場合、専用 bind address を使う
場合、または独立してスケールしたい場合に便利です。

## Obsidian プラグイン

ローカルファイルが信頼できる情報源です — プラグインはディスク上の通常の
Obsidian ボールトをそのまま読み書きします。プロキシファイルシステムは
ありません。プラグインの設定と現在のベアラーデバイストークンは
`<vault>/.obsidian/plugins/pkv-sync/data.json` に保存されます。このファイルは
機密として扱ってください。デバイストークンは使用時に更新され、90 日アイドルで
失効し、各トークンには 365 日の絶対有効期限があります。同じデバイスで再ログインすると有効なトークンが入れ替わります。

日々の機能 — コマンドパレット、ファイル履歴、サイドバイサイド diff、
コンフリクト解決、`.obsidian` の選択的同期、デバイス管理、セルフ
アップデート — は[ユーザーマニュアル](./public-docs/user-manual.ja.md)で
ひととおり解説しています。

## 現時点の暗号化について

PKV Sync 1.0 はまだネイティブの End-to-End 暗号化を **同梱していません** —
サーバーはボールトの内容を読めます。ボールト単位のネイティブ E2EE は 1.x
ロードマップにオプトイン機能として計画していますが、暗号化を入れると
Git-native な PKV を有用にしている機能（履歴 diff、三者自動マージ、SSE の
インラインペイロード、MCP の read/write）と引き換えになります。

ネイティブ対応を待たずに E2EE が必要なら、ボールトに
[`git-crypt`](https://github.com/AGWA/git-crypt) を重ねてください。指定パスは
サーバーから見れば復号不能な ciphertext blob として届きます。ファイル名は
サーバー上では平文のままです（多くの脅威モデルでは許容範囲です）。鍵を持つ
クライアントなら `git clone` と `pkvsyncd materialize` は引き続き機能します。

本番運用では加えて HTTPS の背後で動かし、`trusted_proxies` を絞り、データ
ディスクを暗号化し、バックアップも暗号化してください — 詳細は
[デプロイ強化ガイド](./public-docs/deployment-hardening.ja.md) にあります。

## お探しのものは…

| トピック | ドキュメント |
| --- | --- |
| 日々のプラグイン利用 | [ユーザーマニュアル](./public-docs/user-manual.ja.md) |
| サーバー管理とランタイム設定 | [管理者マニュアル](./public-docs/admin-manual.ja.md) |
| すべての CLI コマンドとフラグ | [CLI リファレンス](./public-docs/cli-reference.ja.md) |
| 0.x から 1.0 へのアップグレード | [1.0 アップグレードノート](./public-docs/upgrade-notes-v1.0.ja.md) |
| リバースプロキシ、TLS、バックアップ、強化 | [デプロイ強化](./public-docs/deployment-hardening.ja.md) |
| HTTP API 仕様 | [OpenAPI spec](./public-docs/openapi.yaml) |
| MCP セットアップとツール一覧 | [MCP ハウツー](./public-docs/mcp-howto.ja.md) |
| Obsidian Sync からの移行 | [移行ガイド](./public-docs/migrate-from-obsidian-sync.ja.md) |
| セキュリティ開示 | [SECURITY.md](./SECURITY.md) |
| リリース履歴 | [CHANGELOG.md](./CHANGELOG.md) |

## ステータス

PKV Sync 1.1.1 は現在の安定セキュリティパッチリリースです。自動マージの競合サイドカーがユーザー除外ルールに一致しても読み取り面と MCP 面で見えるようにし、MCP 認証エラー列挙とレート制限消費の境界ケースを修正し、信頼プロキシ IP、パスワード変更、招待登録、SQLite 権限、Git OID、パス処理、リクエストサイズ上限を強化しました。

PKV Sync 1.0 は最初の安定版リリースです。公開 REST API、CLI サーフェス、
ストレージレイアウト、プラグインパッケージ、Docker イメージは同じ semver で
バージョン管理されます。1.X.Y は公開サーフェスで後方互換性を維持し、
OpenAPI 仕様が互換性の正本となります。0.x で作られた SQLite データベースは
1.0.0 へインプレースでアップグレードできません —
[1.0 アップグレードノート](./public-docs/upgrade-notes-v1.0.ja.md) に従って
ください。

各 GitHub リリースでは Linux amd64/arm64 バイナリ、Windows x64 バイナリ、
マルチアーキの GHCR Docker イメージ、Obsidian プラグインの zip、`SHA256SUMS`
を公開します。

## 開発チェック

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI では Linux と Windows の Rust フルマトリクスに加え、プラグインの
テスト／typecheck／build／パッケージング、Docker ビルド、リリース
バイナリのスモークテストが走ります。

## ライセンス

AGPL-3.0-only。詳しくは [LICENSE](./LICENSE) を参照してください。
