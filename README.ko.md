# PKV Sync

Rust 서버, SQLite 메타데이터, Git 기반 텍스트 이력, content-addressed 첨부 파일
저장소, 데스크톱／모바일 Obsidian 플러그인으로 구성된 self-hosted Obsidian vault
동기화 프로젝트입니다.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | [简体中文](./README.zh-CN.md) | [繁體中文](./README.zh-Hant.md) | [日本語](./README.ja.md) | 한국어

## 상태

PKV Sync 1.0은 첫 stable release입니다. 공개 REST API, CLI surface, storage layout,
플러그인 패키지, Docker 이미지, 공개 문서는 같은 버전으로 관리됩니다.

PKV Sync는 아직 native end-to-end encryption을 제공하지 않습니다. 서버는 동기화된 vault
내용과 첨부 파일을 읽을 수 있습니다. vault 단위 native E2EE는 1.x roadmap에서 선택형
privacy mode로 계획되어 있습니다. 암호화는 history diff, three-way auto-merge,
SSE inline payload, MCP read/write 같은 Git-native 기능과 trade-off가 있으므로 기본값은
일반 Git-native vault입니다. 운영 배포에서는 HTTPS, 엄격한 계정 제어, 암호화 디스크,
암호화 백업, host-level hardening을 사용하세요. 자세한 내용은
[deployment hardening guide](./public-docs/deployment-hardening.ko.md)를 참고하세요.

지금 client-side encryption이 필요하다면 native E2EE가 들어오기 전까지
[`git-crypt`](./public-docs/git-crypt-howto.ko.md)를 함께 사용할 수 있습니다. 암호화된 내용은
PKV Sync 서버에 ciphertext blob으로 저장됩니다. 단, 경로와 파일 이름은 plaintext로 남습니다.

## 안정성과 버전

PKV Sync는 v1.0.0부터 semantic versioning을 따릅니다.

- **Major（X.0.0）**：공개 HTTP API, storage layout, CLI surface에 대한
  backward-incompatible change입니다. 마이그레이션 노트는
  `public-docs/upgrade-notes-vX.0.md`에 문서화합니다.
- **Minor（1.X.0）**：backward-compatible feature addition입니다. 기존 endpoint,
  CLI flag, storage format은 계속 동작합니다.
- **Patch（1.0.X）**：bug fix와 security patch입니다. 공개 API, storage, CLI,
  plugin compatibility를 깨지 않습니다.

공개 REST API contract는 [`public-docs/openapi.yaml`](./public-docs/openapi.yaml)입니다.
Admin Web UI form handler와 OpenAPI에 없는 route는 internal implementation detail입니다.
MCP 동작은 [`public-docs/mcp-howto.ko.md`](./public-docs/mcp-howto.ko.md)에 문서화되어 있습니다.

PKV Sync 1.0은 SQLite migration baseline을 의도적으로 reset합니다. 새 1.x database는
`server/migrations/0001_initial.sql`에서 시작하고, 이후 1.x migration은 append-only입니다.
0.x release로 만든 SQLite database는 1.0.0으로 in-place upgrade할 수 없습니다.
[`public-docs/upgrade-notes-v1.0.ko.md`](./public-docs/upgrade-notes-v1.0.ko.md)를 따르세요.

보안 제보 절차는 [`SECURITY.ko.md`](./SECURITY.ko.md)에 있습니다.

## 주요 기능

- **Multi-user, multi-vault**：인증된 device로 Obsidian vault를 동기화하며,
  vault별 push lock과 idempotent push를 사용합니다.
- **Real-time push**：Server-Sent Events로 commit event를 전달하고, 작은 text change
  （8 KiB 이하）는 event 안에 inline되어 별도 pull 없이 적용됩니다.
- **Git-native storage**：각 vault는 disk 위의 bare git repository입니다. file history,
  unified diff, single-file restore, optional read-only `git clone`을 제공합니다.
- **AI-readable and writable vaults**：`pkvsyncd mcp`가 stdio 또는 stateless
  Streamable HTTP로 MCP read/write tools를 제공합니다.
- **Selective `.obsidian` sync**：새 vault는 theme, snippet, hotkey, app preference,
  appearance, enabled plugin list에 대한 starter allowlist를 받습니다. plugin code와
  plugin setting은 opt-in입니다.
- **Conflict-safe workflow**：SSE inline apply는 local modified file을 덮어쓰지 않고,
  `.conflict-*` file을 남겨 플러그인 command palette에서 해결할 수 있습니다.
- **Admin Web UI**：user, device token, vault, invite, runtime settings, activity,
  blob garbage collection, update visibility를 관리합니다.
- **Security baseline**：Argon2id password hash, login rate limit, fail-closed CSRF,
  사용 시 갱신되는 bearer device token, same-device re-login token replacement.
- Linux amd64 / arm64, Windows x64 binary와 multi-arch GHCR Docker image를 배포합니다.

운영과 사용자 workflow는
[admin manual](./public-docs/admin-manual.ko.md),
[user manual](./public-docs/user-manual.ko.md),
[deployment hardening guide](./public-docs/deployment-hardening.ko.md)를 참고하세요.

## 저장소 레이아웃

```text
data_dir/
  metadata.db        SQLite metadata
  vaults/<vault-id>/ remote vault별 bare Git repository
  blobs/<sha256>     content-addressed binary blob
```

`metadata.db`는 user, vault, device token, invite, runtime settings, sync activity,
blob reference, idempotency record를 저장합니다. vault Git history가 versioned file state의
source of truth입니다. 유지보수 전에는 `pkvsyncd backup`으로 data root와 대응하는
`config.toml`을 snapshot하세요.

## Release Assets

GitHub release에는 다음 asset이 포함됩니다.

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker image는 GHCR에 multi-arch（`linux/amd64`, `linux/arm64`）로 게시됩니다.

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v1.0.0
```

## Quick Start: Docker Compose

권장 경로는 Docker Compose와 `deploy/caddy/`입니다. Caddy가 Let's Encrypt certificate를
요청하고 갱신하며, PKV Sync는 compose network 내부에서 listen합니다.

1. `sync.example.com` 같은 DNS를 서버로 지정합니다.
2. deployment key를 생성합니다.

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

3. `docker-compose.yml` 옆에 `config.toml`을 만듭니다.

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

   `public_host`는 production admin POST에 필수입니다. 설정하지 않으면 admin CSRF check가
   fail-closed되어 admin POST가 거부됩니다.

4. `deploy/caddy/Caddyfile`의 domain을 바꾸고 stack을 시작합니다.

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 새 database에서는 setup wizard를 엽니다.

   ```text
   https://sync.example.com/setup
   ```

6. `https://sync.example.com/admin/login`에 로그인하고 user와 vault를 만든 뒤,
   `pkv-sync-plugin.zip`을 Obsidian에 설치하고 Admin Web UI의 share URL을 plugin에 붙여 넣습니다.

## Upgrade

1.x deployment에서는 database migration이 시작 시 자동으로 적용되고 v1 baseline 이후에는
append-only입니다. 0.x SQLite database는 1.0.0으로 in-place upgrade할 수 없습니다.
먼저 [1.0 upgrade notes](./public-docs/upgrade-notes-v1.0.ko.md)를 읽으세요.

binary install은 다음 명령을 사용할 수 있습니다.

```bash
pkvsyncd upgrade [--dry-run] [--yes] [--version 1.0.0]
```

Docker와 Kubernetes deployment는 container 안의 binary를 교체하지 말고 image tag를 pull하거나
변경해서 upgrade하세요.

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

`mcp` HTTP mode는 bearer token authentication과 함께 모든 `/mcp` request에
`X-PKVSync-Deployment-Key`를 요구합니다.

## Obsidian Plugin

release zip의 `pkv-sync-plugin.zip`을 `<vault>/.obsidian/plugins/pkv-sync/`에 풀고,
Obsidian에서 **PKV Sync**를 활성화합니다. Admin Web UI share URL
（예: `https://sync.example.com/k_xxx/`）을 plugin에 붙여 넣고 연결하세요.

plugin은 일반 local Obsidian vault를 직접 읽고 씁니다.
`<vault>/.obsidian/plugins/pkv-sync/data.json`에는 bearer device token과 deployment key가
저장되므로 민감 파일로 취급하세요. 유출이 의심되면 device token을 revoke하고 다시 연결하세요.

plugin settings의 **Updates**는 연결된 서버의 bundled plugin manifest를 확인하고, 필요하면
GitHub release로 fallback할 수 있습니다. 다운로드한 `main.js`, `manifest.json`, `styles.css`는
SHA-256 검증 후 기록됩니다.

## Configuration

시작 시 읽는 static `config.toml`의 주요 항목입니다.

| Field | Purpose |
| --- | --- |
| `server.bind_addr` | daemon listen address. reverse proxy 뒤에서는 `127.0.0.1:6710`, Docker Compose에서는 `0.0.0.0:6710`. |
| `server.deployment_key` | `pkvsyncd genkey`로 생성하며 client가 `X-PKVSync-Deployment-Key` header로 보냅니다. |
| `server.public_host` | 외부에서 보이는 host 이름. admin POST, share URL, plugin asset URL에 사용됩니다. |
| `storage.data_dir` | `metadata.db`, `vaults/`, `blobs/`를 포함하는 data root. |
| `storage.db_path` | SQLite database path. 보통 `<data_dir>/metadata.db`. |
| `network.trusted_proxies` | `X-Forwarded-For` / `X-Forwarded-Proto`를 신뢰할 CIDR. |
| `update_check.enabled` | GitHub release check와 Admin dashboard update banner를 사용할지 여부. |

runtime settings는 Admin panel에서 편집합니다. 자세한 내용은
[admin manual](./public-docs/admin-manual.ko.md#runtime-settings)을 참고하세요.

## HTTP API

모든 `/api/*` route는 deployment key header를 요구합니다. 인증된 route는 bearer device token도
요구합니다. 공개 REST contract는 [`public-docs/openapi.yaml`](./public-docs/openapi.yaml)입니다.

`GET /api/plugin-manifest`는 인증된 endpoint이며, 서버에 bundled된 plugin version, SHA-256 hash,
self-update download URL을 반환합니다. `public_host`가 설정되어 있으면 URL은 그 external host에
고정됩니다.

production response에는 clickjacking, MIME sniffing, referrer leakage, CSP에 대한 security header가
포함됩니다. `public_host`가 설정되면 HSTS도 전송합니다.

`/metrics`는 `enable_metrics` runtime setting이 true일 때만 활성화됩니다. 활성화되어도
deployment key, PKV Sync User-Agent guard, admin bearer token이 필요합니다.

## Operations

- `pkvsyncd backup --output /var/backups/pkv/<date>`로 snapshot을 만듭니다.
- `pkvsyncd verify`를 주기적으로 실행하여 SHA drift나 orphan blob을 찾습니다.
- restore 전 대상 data directory를 신중히 확인하세요.
- HTTPS 뒤에서 실행하고 `[network].trusted_proxies`를 실제 proxy CIDR로 제한하세요.
- 반복되는 `401`, `403`, `409`, `429` response를 log에서 확인하세요.
- 대량의 attachment 삭제 후 Admin panel에서 blob garbage collection을 실행하세요.

## Documentation

- [Deployment hardening](./public-docs/deployment-hardening.ko.md)
- [Admin manual](./public-docs/admin-manual.ko.md)
- [User manual](./public-docs/user-manual.ko.md)
- [1.0 upgrade notes](./public-docs/upgrade-notes-v1.0.ko.md)
- [Security policy](./SECURITY.ko.md)
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

AGPL-3.0-only. 자세한 내용은 [LICENSE](./LICENSE)를 참고하세요.
