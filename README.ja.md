# PKV Sync

PKV Sync は、Obsidian の Vault をセルフホストで同期する Git ネイティブなサーバーです。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

ドキュメントのバージョン: v1.7.0。

[English](./README.md) · [简体中文](./README.zh-CN.md) · [繁體中文](./README.zh-Hant.md) · 日本語 · [한국어](./README.ko.md)

## 機能

### 同期

- **マルチユーザー・マルチ Vault** 同期。認証済みのデバイス経由で行い、Vault ごとのプッシュロックと冪等リトライを備えます。
- **リアルタイムプッシュ。** 小さな編集は Server-Sent Events で 1 秒未満に反映され、ポーリングはセーフティネットとして残ります。
- **Git が唯一の情報源。** 各 Vault はベア git リポジトリなので、ファイル単位の履歴、unified diff、単一ファイルの復元が最初から使えます — プラグインでも管理パネルでも。
- **競合で壊れない。** プラグインがローカルの編集を黙って上書きすることはありません。競合は `.conflict-*` ファイルとして現れ、ワンクリックで解決できます。

### 運用

- **管理パネル**は 5 言語（English、简中、繁中、日本語、한국어）に対応し、ユーザー、デバイス Token、Vault、招待コード、アクティビティ、blob GC を扱います。破壊的な Vault 操作とユーザー操作には確認ダイアログが表示されます。
- **AI から読める Vault。** MCP が stdio または `pkvsyncd serve` 内蔵の `/mcp` ルート（既定で有効）を通じて読み書きツールを公開し、リリースごとに、そのまま使える [MCP Registry エントリ](./mcp/server.json)（`io.github.cyberkurry/pkv-sync`）が同梱されます。
- **LLM が保守する wiki 向け。** 読み書きに加えて、frontmatter による検索、正確なページング、層ごとのリンク lint、根拠が古くなったページの検出、そして dry run と内容ハッシュのガードを経てからの書き込みができます。[LLM Wiki ワークフロー](./public-docs/llm-wiki-howto.ja.md) を参照してください。
- **既定で境界を設ける。** 管理者が作成するパスワードには setup と同等の強度ポリシーを適用し、Token のシークレットは一度だけ表示し、アップロードと MCP レスポンスにはサイズ上限を設け、常時接続の SSE ストリームは失効した Token を再検証します。

### 設計

- **意図的に枯れた技術を選ぶ。** バイナリ 1 つ、SQLite メタデータ DB 1 つ、Vault ごとにベア git リポジトリ 1 つ、添付ファイルごとに content-addressed な blob 1 つ。

## Docker Compose ですぐ始める

推奨の導入手順です。`deploy/caddy/` の Caddy が Let's Encrypt で HTTPS を終端し、PKV Sync は compose ネットワーク内の `127.0.0.1:6710` で待ち受けます。公開インターネットから平文 HTTP が直接届くことはありません。

ドメイン名（例: `sync.example.com`）が必要です。A/AAAA レコードがこのサーバーを指していること、そして `80` と `443` ポートにインターネットから到達できることが条件です（80 番は ACME HTTP-01 の検証に使います）。

### スタックをセットアップする

1. デプロイキーを生成します:

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` の隣に `config.toml` を置きます:

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey の出力に置き換え
   public_host    = "sync.example.com"   # 必須。管理画面の POST が通るようになります

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker ブリッジネットワーク

   [mcp]
   embed_in_serve = true                 # 既定値。false で無効化
   ```

3. `deploy/caddy/Caddyfile` を編集し、`sync.example.com` を実際のドメインに置き換えます。

4. スタックを起動します:

   ```bash
   docker compose up -d
   ```

   ブラウザで `https://sync.example.com/setup` を開き、最初の管理者アカウントを作成します。

5. Obsidian に `pkv-sync-plugin.zip` をインストールし
   （`<vault>/.obsidian/plugins/pkv-sync/` に配置）、有効化して、管理パネルの
   共有 URL を貼り付け、ログインまたは登録して Vault を選びます。

### 更新

```bash
docker compose pull && docker compose up -d
```

### 次に読むもの

ネイティブインストール、リバースプロキシのチューニング（Caddy / Nginx / Traefik）、`public_host` の意味、バックアップと復元、ディスク暗号化については、[デプロイ強化ガイド](./public-docs/deployment-hardening.ja.md)を参照してください。

## MCP デプロイモード

MCP HTTP は内蔵方式のみです。`[mcp].embed_in_serve` は既定で `true` のため、`pkvsyncd serve` はメインサーバーのポートに `/mcp` をマウントし、同じ TLS 終端、リバースプロキシ、デプロイキー、bearer Token 検証を共有します。`embed_in_serve = false` で無効化できます。単体動作する `pkvsyncd mcp --transport http --bind` プロセスはこのバージョンで削除されました。既存の単体デプロイは、メインサーバーの `/mcp` を指すように変更してください。ローカルの AI ツールは stdio トランスポート（`pkvsyncd mcp --vault <id>`）も利用できます。

## Obsidian プラグイン

ローカルファイルが唯一の情報源です。プラグインはディスク上の通常の Obsidian Vault をそのまま読み書きし、プロキシファイルシステムは使いません。機密性のないプラグイン設定と同期インデックスは `<vault>/.obsidian/plugins/pkv-sync/data.json` に保存されます。ログイン状態、現在有効な bearer デバイス Token、デプロイキー、安定したデバイス識別子は、Obsidian のデバイスローカルストレージに保存されます。Obsidian のデバイスローカルストレージ、平文のバックアップ、旧バージョンが残したプラグイン `data.json` のコピーは、いずれも機密情報として扱ってください。デバイス Token は使用時に更新され、90 日間使われないと失効し、絶対有効期限は 365 日です。同じデバイスで再ログインすると、有効な Token が入れ替わります。

日々の機能 — コマンドパレット、ファイル履歴、サイドバイサイド diff、競合の解決、`.obsidian` の選択同期、デバイス管理、自動更新 — は[ユーザーマニュアル](./public-docs/user-manual.ja.md)で解説しています。

## 現時点の暗号化について

PKV Sync 1.0 はネイティブのエンドツーエンド暗号化を**まだ搭載していません**。サーバーは Vault の内容を読めます。Vault 単位のネイティブ E2EE は 1.x のロードマップにオプトイン方式として計画されています。暗号化を入れると、Git ネイティブな PKV を有用にしているサーバー側の機能（履歴 diff、三者自動マージ、SSE のインラインペイロード、MCP の読み書き）と引き換えになるためです。

E2EE が実装される前に必要なら、Vault に [`git-crypt`](https://github.com/AGWA/git-crypt) を重ねてください。マークしたパスは、サーバーが復号できない暗号文 blob として届きます。ファイル名はサーバー上では平文のままです（多くの脅威モデルでは許容範囲です）。鍵を持つクライアントであれば、標準の `git clone` と `pkvsyncd materialize` は引き続き機能します。

本番運用ではさらに、HTTPS の背後で動かし、`trusted_proxies` を絞り、データディスクとバックアップを暗号化してください — [デプロイ強化ガイド](./public-docs/deployment-hardening.ja.md)を参照してください。

## ドキュメント

| トピック | ドキュメント |
| --- | --- |
| プラグインの日常的な使い方 | [ユーザーマニュアル](./public-docs/user-manual.ja.md) |
| サーバー管理と実行時設定 | [管理者マニュアル](./public-docs/admin-manual.ja.md) |
| すべての CLI コマンドとフラグ | [CLI リファレンス](./public-docs/cli-reference.ja.md) |
| 0.x から 1.0 へのアップグレード | [1.0 アップグレードノート](./public-docs/upgrade-notes-v1.0.ja.md) |
| リバースプロキシ、TLS、バックアップ、強化 | [デプロイ強化](./public-docs/deployment-hardening.ja.md) |
| HTTP API の契約 | [OpenAPI 仕様](./public-docs/openapi.yaml) |
| MCP の設定とツール一覧 | [MCP ハウツー](./public-docs/mcp-howto.ja.md) |
| LLM が保守する wiki ワークフロー | [LLM Wiki ハウツー](./public-docs/llm-wiki-howto.ja.md) |
| Obsidian Sync からの移行 | [移行ガイド](./public-docs/migrate-from-obsidian-sync.ja.md) |
| セキュリティ報告 | [SECURITY.md](./SECURITY.md) |
| リリース履歴 | [CHANGELOG.md](./CHANGELOG.md) |

## 開発

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

cd plugin
npm run typecheck
npm exec vitest run
npm run build
```

CI は Linux と Windows で Rust のフルマトリクスを実行し、加えてプラグインの test／typecheck／build／package、Docker ビルド、リリースバイナリのスモークテストを行います。

## コントリビューション

バグ報告、機能要望、Pull Request を歓迎します。バグ修正より大きな変更は、まず issue を立てて相談してください。ビルドとテストのコマンド、コミットとリリースの流れ、CI が検証するドキュメントのルールは [CONTRIBUTING.md](./CONTRIBUTING.md) にあります。

## ライセンス

AGPL-3.0-only。[LICENSE](./LICENSE) を参照してください。
