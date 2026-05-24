# PKV Sync

Rust サーバー、SQLite メタデータ、Git ベースのテキスト履歴、
コンテンツアドレス型添付ファイルストレージ、デスクトップ／モバイル対応の
Obsidian プラグインで構成される、セルフホスト型 Obsidian vault 同期です。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | [简体中文](./README.zh-CN.md) | [繁體中文](./README.zh-Hant.md) | 日本語 | [한국어](./README.ko.md)

## 状態

PKV Sync 1.0 は最初の安定版です。公開 REST API、CLI surface、ストレージ
レイアウト、プラグインパッケージ、Docker イメージ、公開ドキュメントは同じ
バージョンとして管理されます。

PKV Sync はまだネイティブのエンドツーエンド暗号化を提供していません。
サーバーは同期された vault の内容と添付ファイルを読み取れます。vault 単位の
ネイティブ E2EE は 1.x ロードマップの任意機能として計画されています。暗号化は
履歴 diff、三者自動マージ、SSE inline payload、MCP read/write などの
Git-native 機能とトレードオフになるため、既定では通常の Git-native vault として
動作します。実運用では HTTPS、厳格なアカウント管理、暗号化ディスク、暗号化
バックアップ、ホストレベルの hardening を使ってください。詳しくは
[deployment hardening guide](./public-docs/deployment-hardening.ja.md) を参照してください。

今すぐクライアント側暗号化が必要な場合は、ネイティブ E2EE が入るまで
[`git-crypt`](./public-docs/git-crypt-howto.ja.md) を重ねて使えます。暗号化された内容は
PKV Sync から見ると ciphertext blob になります。ただしパスとファイル名は平文のままです。

## 安定性とバージョン

PKV Sync は v1.0.0 から semantic versioning に従います。

- **Major（X.0.0）**：公開 HTTP API、ストレージレイアウト、CLI surface に対する
  後方互換性のない変更。移行手順は `public-docs/upgrade-notes-vX.0.md` に記載します。
- **Minor（1.X.0）**：後方互換性のある機能追加。既存の endpoint、CLI flag、
  ストレージ形式は引き続き使えます。
- **Patch（1.0.X）**：バグ修正とセキュリティ修正。公開 API、ストレージ、CLI、
  プラグイン互換性を壊しません。

公開 REST API の契約は [`public-docs/openapi.yaml`](./public-docs/openapi.yaml) です。
Admin Web UI の form handler など、OpenAPI に載っていない route は内部実装です。
MCP の挙動は [`public-docs/mcp-howto.ja.md`](./public-docs/mcp-howto.ja.md) に記載しています。

PKV Sync 1.0 は SQLite migration baseline を意図的にリセットします。新しい 1.x
データベースは `server/migrations/0001_initial.sql` から始まり、今後の 1.x
migration は append-only です。0.x で作成された SQLite database は 1.0.0 へ
インプレース upgrade できません。
[`public-docs/upgrade-notes-v1.0.ja.md`](./public-docs/upgrade-notes-v1.0.ja.md) に従ってください。

セキュリティ報告は [`SECURITY.ja.md`](./SECURITY.ja.md) を参照してください。

## 主な機能

- **複数ユーザー、複数 vault**：認証済みデバイスで Obsidian vault を同期し、
  vault 単位の push lock と冪等 push を使います。
- **リアルタイム push**：Server-Sent Events により commit event を配信し、小さな
  テキスト変更（8 KiB 以下）は event 内に inline されます。
- **Git-native**：各 vault はディスク上の bare git repository です。ファイル履歴、
  unified diff、単一ファイル restore、任意の read-only `git clone` を提供します。
- **AI から読める／書ける vault**：`pkvsyncd mcp` が stdio または stateless
  Streamable HTTP で MCP read/write tools を公開します。
- **選択式 `.obsidian` 同期**：新しい vault には theme、snippet、hotkey、
  app preference、appearance、有効プラグイン一覧の starter allowlist が入ります。
  プラグインコードとプラグイン設定は opt-in です。
- **衝突に強い workflow**：ローカル変更があるファイルを SSE inline apply で上書きせず、
  `.conflict-*` ファイルとして残し、プラグインの command palette から解決できます。
- **Admin Web UI**：ユーザー、device token、vault、invite、runtime settings、
  activity、blob garbage collection、update visibility を管理します。
- **セキュリティ基盤**：Argon2id password hash、login rate limit、fail-closed CSRF、
  使用時に更新される bearer device token、同一デバイス再ログイン時の token replacement。
- Linux amd64 / arm64、Windows x64 の binary と multi-arch GHCR Docker image を配布します。

運用と利用者向けの詳細は
[管理者マニュアル](./public-docs/admin-manual.ja.md)、
[ユーザーマニュアル](./public-docs/user-manual.ja.md)、
[deployment hardening guide](./public-docs/deployment-hardening.ja.md) を参照してください。

## ストレージレイアウト

```text
data_dir/
  metadata.db        SQLite metadata
  vaults/<vault-id>/ remote vault ごとの bare Git repository
  blobs/<sha256>     content-addressed binary blob
```

`metadata.db` はユーザー、vault、device token、invite、runtime settings、
sync activity、blob reference、idempotency record を保持します。vault の Git history が
versioned file state の source of truth です。メンテナンス前には `pkvsyncd backup` で
data root と対応する `config.toml` を snapshot してください。

## リリース資産

GitHub release には次の資産が含まれます。

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker image は GHCR に multi-arch（`linux/amd64`、`linux/arm64`）で公開されます。

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v1.0.0
```

## Quick Start: Docker Compose

推奨構成は Docker Compose と `deploy/caddy/` です。Caddy が Let's Encrypt 証明書を取得・更新し、
PKV Sync は compose network 内で待ち受けます。

1. `sync.example.com` などの DNS をサーバーへ向けます。
2. deployment key を生成します。

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

3. `docker-compose.yml` と同じ場所に `config.toml` を作成します。

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_replace_me_with_genkey_output"
   public_host = "sync.example.com"

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]

   [logging]
   level = "info"
   format = "json"
   ```

   `public_host` は production admin POST に必須です。未設定の場合、admin CSRF check は
   fail-closed になり、admin POST は拒否されます。

4. `deploy/caddy/Caddyfile` の domain を変更し、stack を起動します。

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 新しい database では setup wizard を開きます。

   ```text
   https://sync.example.com/setup
   ```

6. `https://sync.example.com/admin/login` へログインし、ユーザーと vault を作成します。
   `pkv-sync-plugin.zip` を Obsidian にインストールし、Admin Web UI の share URL を
   プラグインへ貼り付けます。

## Upgrade

1.x deployment では database migration は起動時に自動適用され、v1 baseline 以後は
append-only です。0.x SQLite database は 1.0.0 へインプレース upgrade できません。
先に [1.0 upgrade notes](./public-docs/upgrade-notes-v1.0.ja.md) を読んでください。

binary install では次を使えます。

```bash
pkvsyncd upgrade [--dry-run] [--yes] [--version 1.0.0]
```

Docker と Kubernetes deployment は、container 内の binary を置き換えるのではなく、
image tag を pull または変更して upgrade してください。

## Server CLI

```bash
pkvsyncd genkey
pkvsyncd -c /etc/pkv-sync/config.toml migrate up
pkvsyncd -c /etc/pkv-sync/config.toml serve
pkvsyncd -c /etc/pkv-sync/config.toml user add alice [--admin]
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
pkvsyncd -c /etc/pkv-sync/config.toml materialize <vault-id> --output <dir>
pkvsyncd -c /etc/pkv-sync/config.toml backup --output <dir> [--data-dir <dir>] [--gzip]
pkvsyncd -c /etc/pkv-sync/config.toml restore --input <backup-dir> --data-dir <dir> [--force]
pkvsyncd -c /etc/pkv-sync/config.toml verify [--data-dir <dir>] [--no-fail]
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
pkvsyncd upgrade [--dry-run] [--yes] [--version 1.0.0]
```

`mcp` の HTTP mode は bearer token authentication に加えて、すべての `/mcp` request で
`X-PKVSync-Deployment-Key` を要求します。

## Obsidian プラグイン

release zip の `pkv-sync-plugin.zip` を `<vault>/.obsidian/plugins/pkv-sync/` に展開し、
Obsidian で **PKV Sync** を有効にします。Admin Web UI から share URL
（例：`https://sync.example.com/k_xxx/`）をコピーし、プラグインに貼り付けて接続します。

プラグインは通常のローカル Obsidian vault を直接読み書きします。
`<vault>/.obsidian/plugins/pkv-sync/data.json` には bearer device token と deployment key が
保存されるため、機密ファイルとして扱ってください。漏えいが疑われる場合は device token を
revoke し、再接続してください。

プラグイン設定の **Updates** では、接続中サーバーに同梱された plugin manifest を確認し、
必要に応じて GitHub release に fallback できます。download した `main.js`、
`manifest.json`、`styles.css` は SHA-256 検証後に書き込まれます。

## 設定

起動時に読む静的 `config.toml` の主な項目です。

| Field | Purpose |
| --- | --- |
| `server.bind_addr` | daemon の listen address。reverse proxy 配下では `127.0.0.1:6710`、Docker Compose では `0.0.0.0:6710`。 |
| `server.deployment_key` | `pkvsyncd genkey` で生成し、client が `X-PKVSync-Deployment-Key` header で送ります。 |
| `server.public_host` | 外部から見える host 名。admin POST、share URL、plugin asset URL に使います。 |
| `storage.data_dir` | `metadata.db`、`vaults/`、`blobs/` を含む data root。 |
| `storage.db_path` | SQLite database path。通常は `<data_dir>/metadata.db`。 |
| `network.trusted_proxies` | `X-Forwarded-For` / `X-Forwarded-Proto` を信頼する CIDR。 |
| `update_check.enabled` | GitHub release check と Admin dashboard の update banner を有効にするか。 |

runtime settings は Admin panel から編集します。詳しくは
[管理者マニュアル](./public-docs/admin-manual.ja.md#runtime-settings) を参照してください。

## HTTP API

すべての `/api/*` route は deployment key header を要求します。認証済み route はさらに
bearer device token を要求します。公開 REST contract は
[`public-docs/openapi.yaml`](./public-docs/openapi.yaml) です。

`GET /api/plugin-manifest` は認証済み endpoint で、サーバーに同梱されたプラグイン version、
SHA-256 hash、self-update 用 download URL を返します。`public_host` が設定されている場合、
URL はその外部 host に固定されます。

production response には clickjacking、MIME sniffing、referrer leakage、CSP に対する
security header が含まれます。`public_host` が設定されている場合は HSTS も送信します。

`/metrics` は `enable_metrics` runtime setting が true のときだけ有効です。有効時も
deployment key、PKV Sync User-Agent guard、admin bearer token が必要です。

## 運用

- `pkvsyncd backup --output /var/backups/pkv/<date>` で snapshot を作成します。
- `pkvsyncd verify` を定期実行し、SHA drift や orphan blob を検出します。
- restore 前には対象 data directory を慎重に確認してください。
- HTTPS の背後で動かし、`[network].trusted_proxies` を実際の proxy CIDR に限定します。
- `401`、`403`、`409`、`429` が繰り返し出る場合は log を確認します。
- 大量の添付ファイル削除後は Admin panel から blob garbage collection を実行します。

## ドキュメント

- [Deployment hardening](./public-docs/deployment-hardening.ja.md)
- [管理者マニュアル](./public-docs/admin-manual.ja.md)
- [ユーザーマニュアル](./public-docs/user-manual.ja.md)
- [1.0 upgrade notes](./public-docs/upgrade-notes-v1.0.ja.md)
- [Security policy](./SECURITY.ja.md)
- [OpenAPI spec](./public-docs/openapi.yaml)
- [Changelog](./CHANGELOG.md)

## Development Checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin exec vitest run
npm --prefix plugin run typecheck
npm --prefix plugin run build
npm --prefix plugin run package
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

## License

AGPL-3.0-only。詳しくは [LICENSE](./LICENSE) を参照してください。
