# PKV Sync

**Obsidian ボールトをセルフホストで。** PKV Sync は自前のサーバー上で動き、スマホ・タブレット・デスクトップの間で Obsidian ボールトを同期し続けます。バイナリ 1 つ、SQLite データベース 1 つ、ボールトごとに bare な Git リポジトリ 1 つ — クラスターも S3 もマネージドクラウドも不要です。インストールして、Obsidian の接続先に指定すれば、ノートが同期されます。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

ドキュメントバージョン: v1.6.0。

[English](./README.md) · [简体中文](./README.zh-CN.md) · [繁體中文](./README.zh-Hant.md) · 日本語 · [한국어](./README.ko.md)

## 機能

- **マルチユーザー・マルチボールト**の同期。認証済みデバイス経由で同期し、ボールトごとにプッシュロック（push lock）と冪等リトライが効きます。
- **リアルタイムプッシュ**。小さな編集は Server-Sent Events 経由で 1 秒未満で反映されます。ポーリングは保険として残ります。
- **Git が信頼できる唯一の情報源**。すべてのボールトは bare な Git リポジトリなので、ファイル単位の履歴、unified diff、単一ファイルの復元が標準で動きます — プラグインでも管理パネルでも。
- **コンフリクトに強い**。プラグインがローカルの編集を黙って上書きすることはありません。コンフリクトは `.conflict-*` ファイルとして表れ、ワンクリックで解決できます。
- **管理パネル**は 5 言語対応（English、简中、繁中、日本語、한국어）。ユーザー、デバイストークン（device token）、ボールト、招待コード、アクティビティ、blob GC をここから管理し、破壊的なボールト操作とユーザー操作には確認ダイアログが表示されます。
- **AI から読めるボールト**。MCP は stdio または `pkvsyncd serve` に組み込み済みの `/mcp` ルート（デフォルトで有効）で読み書きツールを公開します。
- **デフォルトで安全側**。管理者が作成するパスワードには setup と同等の強度ポリシーが適用され、トークンのシークレットは一度しか表示されず、アップロードと MCP レスポンスにはサイズ上限があり、SSE のライブストリームは取り消されたトークンを再検証します。
- **あえて退屈な構成**。バイナリ 1 つ、SQLite メタデータ DB 1 つ、ボールトごとに bare な Git リポジトリ 1 つ、添付ファイルごとに content-addressed な blob 1 つ。

## Docker Compose ですぐ始める

これが推奨ルートです。`deploy/caddy/` の Caddy が Let's Encrypt による HTTPS を受け持ち、PKV Sync は compose ネットワーク内の `127.0.0.1:6710` で待ち受けます。公開インターネットからの平文 HTTP は、PKV Sync まで一切届きません。

ドメイン（例: `sync.example.com`）を用意し、A/AAAA レコードをサーバーに向けて、ポート `80` と `443` をインターネットから到達可能にしてください（ポート 80 は ACME HTTP-01 検証で使います）。

1. デプロイメントキー（deployment key）を生成します。

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `config.toml` を `docker-compose.yml` の隣に置きます。

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey の出力に置き換え
   public_host    = "sync.example.com"   # 必須。admin POST が正しく動くようになります

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker ブリッジネットワーク

   [mcp]
   embed_in_serve = true                 # デフォルト。false で無効化
   ```

3. `deploy/caddy/Caddyfile` を編集し、`sync.example.com` を実際のドメインに置き換えます。

4. スタックを起動します。

   ```bash
   docker compose up -d
   ```

   ブラウザで `https://sync.example.com/setup` を開き、最初の管理者アカウントを作成します。

5. Obsidian に `pkv-sync-plugin.zip` をインストールします（`<vault>/.obsidian/plugins/pkv-sync/`）。有効化したら、管理パネルの共有 URL を貼り付け、ログインまたは登録してボールトを選びます。

更新は `docker compose pull && docker compose up -d` です。ネイティブインストール、リバースプロキシのチューニング（Caddy / Nginx / Traefik）、`public_host` の意味、バックアップとリストア、ディスク暗号化については、[デプロイ強化ガイド](./public-docs/deployment-hardening.ja.md)を参照してください。

## MCP デプロイモード

MCP HTTP は埋め込み専用です。`[mcp].embed_in_serve` はデフォルトで `true` のため、`pkvsyncd serve` がメインポートに `/mcp` をマウントし、同じ TLS 終端、リバースプロキシ、デプロイメントキー、bearer トークン検証を共有します。`embed_in_serve = false` で無効化できます。スタンドアロンの `pkvsyncd mcp --transport http --bind` プロセスはこのバージョンで削除されました。既存のスタンドアロン運用は、メインサーバーの `/mcp` を指すようにしてください。ローカルの AI ツールは stdio トランスポート（`pkvsyncd mcp --vault <id>`）も利用できます。

## Obsidian プラグイン

ローカルファイルが信頼できる唯一の情報源です — プラグインはディスク上の通常の Obsidian ボールトをそのまま読み書きします。プロキシファイルシステムは存在しません。プラグインの非機密設定と同期インデックスは `<vault>/.obsidian/plugins/pkv-sync/data.json` に保存されます。ログイン状態、現在有効な bearer デバイストークン、デプロイメントキー、安定したデバイス ID は、Obsidian のデバイスローカルストレージに保存されます。Obsidian のデバイスローカルストレージ、平文のバックアップ、旧バージョンが残したプラグイン `data.json` のコピーは機密として扱ってください。デバイストークンは使用時に更新され、90 日間未使用で失効し、トークンごとに 365 日の絶対有効期限があります。同じデバイスで再ログインすると、有効なトークンが入れ替わります。

日々の機能 — コマンドパレット、ファイル履歴、サイドバイサイド diff、コンフリクト解決、`.obsidian` の選択的同期、デバイス管理、セルフアップデート — は[ユーザーマニュアル](./public-docs/user-manual.ja.md)でひととおり解説しています。

## 現時点の暗号化について

PKV Sync 1.0 は、ネイティブのエンドツーエンド暗号化をまだ同梱していません — サーバーはボールトの内容を読めます。ボールト単位のネイティブ E2EE は 1.x ロードマップにオプトイン方式で計画されています。暗号化を入れると、Git-native な PKV を有用にしているサーバー側の機能（履歴 diff、3-way 自動マージ、SSE のインラインペイロード、MCP の読み書き）と引き換えになるためです。

ネイティブ対応を待たずに E2EE が必要なら、ボールトに [`git-crypt`](https://github.com/AGWA/git-crypt) を重ねてください。マークしたパスは、サーバーにとって復号不能な暗号 blob として届きます。ファイル名はサーバー上で平文のままです（多くの脅威モデルでは許容範囲です）。鍵を持つクライアントなら、通常どおり `git clone` と `pkvsyncd materialize` を使えます。

本番運用では、さらに HTTPS の背後で動かし、`trusted_proxies` を絞り、データディスクとバックアップを暗号化してください — 詳細は[デプロイ強化ガイド](./public-docs/deployment-hardening.ja.md)にあります。

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
| LLM が保守する Wiki ワークフロー | [LLM Wiki ハウツー](./public-docs/llm-wiki-howto.ja.md) |
| Obsidian Sync からの移行 | [移行ガイド](./public-docs/migrate-from-obsidian-sync.ja.md) |
| セキュリティ開示 | [SECURITY.md](./SECURITY.md) |
| リリース履歴 | [CHANGELOG.md](./CHANGELOG.md) |

## ステータス

PKV Sync 1.6.0 は、MCP をファーストクラスの機能にします。サーバーは新しい 2026-07-28 のステートレスプロトコル改訂とレガシークライアントの両方に対応（デュアルエラ）し、`subscriptions/listen` プッシュが加わり、MCP はデフォルトでメインサーバーに組み込まれました（スタンドアロンの `mcp --transport http` デプロイは廃止）。`public_host` を設定できない管理デプロイには `allow_insecure_admin` という回避策が用意され、ヘルスプローブの修正により、セルフアップグレードが黙ってロールバックされず実際に動くようになりました。さらに同期の正確性も修正されました。auto-merge はコミットを飛ばさなくなり（取得していないマージベースを越えてローカルヘッドが進まなくなり）、リアルタイム適用もローカルファイルを黙って上書きしたり復活させたりしなくなりました。

PKV Sync 1.5.0 は、2026-08-16 のリポジトリ全体監査への対応を完遂し、さらにパフォーマンス改善を盛り込みました。blob の保持は git オブジェクトの存続に追従するようになり、ロールバックやファイル単位の履歴復元で添付ファイルが失われなくなります。Windows で問題を起こすパス要素は `materialize` に届く前に拒否され、特権アップデータはダウングレードを拒否し、Docker セルフアップグレードのオーバーレイは強化されました。登録と blob アップロードの経路には不足していたレートリミットが追加され、常駐する SSE 接続はグレースフルシャットダウンを妨げなくなりました。プラグインは allowlist 内の `.obsidian` パスを同期し、初回同期はメモリ上限付きのバッチでストリーム処理します（モバイル向け）。バッチ化した git 読み取りと単一コピーのインデックス更新により、MCP 検索と大規模ボールトの同期が高速化されました。

PKV Sync 1.0 は最初の安定版リリースです。公開 REST API、CLI、ストレージレイアウト、プラグインパッケージ、Docker イメージは同じ semver のもとで一緒にバージョン管理されます。1.X.Y は公開サーフェスで後方互換性を維持し、OpenAPI 仕様が互換性の正本です。0.x で作成した SQLite データベースは 1.0.0 へインプレースでアップグレードできません — [1.0 アップグレードノート](./public-docs/upgrade-notes-v1.0.ja.md)に従ってください。

各 GitHub リリースでは、Linux amd64/arm64 バイナリ、Windows x64 バイナリ、マルチアーキテクチャの GHCR Docker イメージ、Obsidian プラグインの zip、`SHA256SUMS` が公開されます。

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

CI では、Linux と Windows の Rust フルマトリクスに加え、プラグインのテスト・タイプチェック・ビルド・パッケージング、Docker ビルド、リリースバイナリのスモークテストが実行されます。

## ライセンス

AGPL-3.0-only。詳しくは [LICENSE](./LICENSE) を参照してください。
