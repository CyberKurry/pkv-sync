# PKV Sync

**Obsidian 볼트를 직접 호스팅하세요.** PKV Sync는 자체 서버에서 동작하며,
Obsidian 볼트를 휴대폰, 태블릿, 데스크톱 사이에서 동기화합니다. 바이너리
하나, SQLite 데이터베이스 하나, 볼트마다 bare git 저장소 하나가 전부입니다.
클러스터도, S3도, 매니지드 클라우드도 없습니다. 설치하고, Obsidian에서
가리키도록 설정하면 노트가 동기화됩니다.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | [简体中文](./README.zh-CN.md) | [繁體中文](./README.zh-Hant.md) | [日本語](./README.ja.md) | 한국어

## 기능

- **다중 사용자, 다중 볼트** 동기화를 인증된 기기 사이에서 지원하며,
  볼트별 push lock 과 멱등 재시도를 제공합니다.
- **실시간 push.** 작은 편집은 Server-Sent Events 로 1 초 이내에 도착합니다.
  폴링은 안전망으로 남아 있습니다.
- **Git 이 단일 source of truth.** 모든 볼트는 bare git 저장소이므로
  파일별 이력, unified diff, 단일 파일 복원이 플러그인과 어드민 패널에서
  바로 동작합니다.
- **충돌 안전.** 플러그인은 로컬 편집을 조용히 덮어쓰지 않습니다.
  충돌은 `.conflict-*` 파일로 노출되고 한 번의 클릭으로 해결할 수 있습니다.
- **어드민 패널**은 5 개 언어(English, 简中, 繁中, 日本語, 한국어)로
  사용자, 기기 토큰, 볼트, 초대, 활동, 블롭 GC 를 관리합니다.
- **AI 가 읽을 수 있는 볼트.** `pkvsyncd mcp` 가 stdio 또는 streaming HTTP
  로 읽기／쓰기 MCP 도구를 제공합니다.
- **의도적으로 단순합니다.** 바이너리 하나, SQLite 메타데이터 DB 하나,
  볼트마다 bare git 저장소 하나, 첨부 파일마다 content-addressed 블롭 하나.

## Docker Compose 로 빠르게 시작

권장 경로입니다. `deploy/caddy/` 의 Caddy 가 Let's Encrypt 로 HTTPS 를
처리하고, PKV Sync 는 compose 네트워크 안 `127.0.0.1:6710` 에 머무르며
공용 인터넷의 평문 HTTP 를 직접 받지 않습니다.

도메인 이름(예: `sync.example.com`)이 필요합니다. A／AAAA 레코드가 서버를
가리켜야 하고, 인터넷에서 `80` 과 `443` 포트에 접근할 수 있어야 합니다
(포트 80 은 ACME HTTP-01 검증에 사용됩니다).

1. 배포 키를 생성합니다.

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` 옆에 `config.toml` 을 둡니다.

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_replace_me_with_genkey_output"
   public_host    = "sync.example.com"   # 필수, 어드민 POST 가 동작하려면 필요

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge network
   ```

3. `deploy/caddy/Caddyfile` 을 편집해 `sync.example.com` 을 실제
   도메인으로 바꿉니다.

4. 스택을 띄웁니다.

   ```bash
   docker compose up -d
   ```

   브라우저에서 `https://sync.example.com/setup` 을 열고 첫 관리자
   계정을 만듭니다.

5. `pkv-sync-plugin.zip` 을 Obsidian 에 설치합니다
   (`<vault>/.obsidian/plugins/pkv-sync/`). 활성화하고 어드민 패널의
   share URL 을 붙여 넣은 뒤, 로그인 또는 가입하고 볼트를 고릅니다.

업데이트는 `docker compose pull && docker compose up -d` 입니다. 네이티브
설치, 리버스 프록시 튜닝(Caddy／Nginx／Traefik), `public_host` 의미,
백업／복원, 디스크 암호화는
[배포 강화 가이드](./public-docs/deployment-hardening.ko.md)를
참고하세요.

## Obsidian 플러그인

로컬 파일이 source of truth 입니다. 플러그인은 디스크 위의 평범한
Obsidian 볼트를 읽고 쓰며, 프록시 파일시스템을 만들지 않습니다. 플러그인
설정과 활성 bearer 기기 토큰은
`<vault>/.obsidian/plugins/pkv-sync/data.json` 에 저장되므로 민감 파일로
취급하세요. 기기 토큰은 사용할 때마다 갱신되고 90 일 동안 비활성 상태이면
만료됩니다. 같은 기기에서 다시 로그인하면 활성 토큰이 교체됩니다.

명령 팔레트, 파일 이력, 좌우 diff, 충돌 해결, 선택적 `.obsidian` 동기화,
기기 관리, 자가 업데이트 같은 일상 기능은
[사용자 매뉴얼](./public-docs/user-manual.ko.md)에서 안내합니다.

## 현재 시점의 암호화

PKV Sync 1.0 은 아직 native end-to-end encryption 을 제공하지 **않습니다**.
서버는 볼트 내용을 읽을 수 있습니다. 볼트별 native E2EE 는 1.x 로드맵에
opt-in 모드로 계획되어 있습니다. 암호화는 Git-native PKV 를 쓸모 있게
만드는 서버 측 기능(이력 diff, 3-way 자동 병합, 인라인 SSE payload, MCP
읽기／쓰기)과 trade-off 가 있기 때문입니다.

E2EE 가 도입되기 전에 필요하다면, 볼트에
[`git-crypt`](https://github.com/AGWA/git-crypt) 을 얹으세요. 표시된
경로는 서버가 복호화할 수 없는 ciphertext 블롭으로 도달합니다. 파일
이름은 서버에 평문으로 남습니다(대부분의 위협 모델에서 수용 가능합니다).
표준 `git clone` 과 `pkvsyncd materialize` 는 키를 가진 클라이언트에서
계속 동작합니다.

실제 배포에서는 HTTPS 뒤에서 실행하고, `trusted_proxies` 를 제한하며,
데이터 디스크와 백업을 암호화하세요. 자세한 내용은
[배포 강화 가이드](./public-docs/deployment-hardening.ko.md)를
참고하세요.

## 찾고 계신 건…

| 주제 | 문서 |
| --- | --- |
| 일상적인 플러그인 사용 | [사용자 매뉴얼](./public-docs/user-manual.ko.md) |
| 서버 운영과 런타임 설정 | [관리자 매뉴얼](./public-docs/admin-manual.ko.md) |
| 모든 CLI 명령과 플래그 | [CLI 레퍼런스](./public-docs/cli-reference.ko.md) |
| 0.x 에서 1.0 으로 업그레이드 | [1.0 업그레이드 노트](./public-docs/upgrade-notes-v1.0.ko.md) |
| 리버스 프록시, TLS, 백업, 하드닝 | [배포 강화](./public-docs/deployment-hardening.ko.md) |
| HTTP API 계약 | [OpenAPI 명세](./public-docs/openapi.yaml) |
| MCP 설정과 도구 목록 | [MCP 사용법](./public-docs/mcp-howto.ko.md) |
| Obsidian Sync 에서 이전 | [이전 가이드](./public-docs/migrate-from-obsidian-sync.ko.md) |
| 보안 제보 | [SECURITY.md](./SECURITY.md) |
| 릴리스 이력 | [CHANGELOG.md](./CHANGELOG.md) |

## 상태

PKV Sync 1.0 은 첫 안정 릴리스입니다. 공개 REST API, CLI 표면, 저장소
레이아웃, 플러그인 패키지, Docker 이미지는 같은 semver 로 관리됩니다.
1.X.Y 는 공개 표면에서 하위 호환을 유지하고, OpenAPI 명세가 정식 호환성
계약입니다. 0.x 릴리스로 만든 SQLite 데이터베이스는 1.0.0 으로 in-place
업그레이드할 수 없습니다.
[1.0 업그레이드 노트](./public-docs/upgrade-notes-v1.0.ko.md)를 따르세요.

GitHub 릴리스마다 Linux amd64／arm64 바이너리, Windows x64 바이너리,
멀티 아키텍처 GHCR Docker 이미지, Obsidian 플러그인 zip, `SHA256SUMS` 가
함께 게시됩니다.

## 개발 체크

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 는 Linux 와 Windows 에서 전체 Rust 매트릭스, 플러그인
test／typecheck／build／package, Docker 빌드, 릴리스 바이너리
smoke 테스트를 실행합니다.

## 라이선스

AGPL-3.0-only. [LICENSE](./LICENSE) 를 참고하세요.
