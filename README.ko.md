# PKV Sync

**Obsidian 볼트를 직접 호스팅하세요.** PKV Sync는 자체 서버에서 동작하며 휴대폰, 태블릿, 데스크톱 사이에서 Obsidian 볼트를 계속 동기화합니다. 바이너리 하나, SQLite 데이터베이스 하나, 볼트마다 bare git 저장소 하나가 전부입니다. 클러스터도, S3도, 매니지드 클라우드도 필요 없습니다. 설치하고 Obsidian의 접속 주소로 지정하면 노트가 동기화됩니다.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

문서 버전: v1.6.0.

[English](./README.md) · [简体中文](./README.zh-CN.md) · [繁體中文](./README.zh-Hant.md) · [日本語](./README.ja.md) · 한국어

## 기능

- **다중 사용자, 다중 볼트** 동기화. 인증된 기기를 통해서만 동기화가 이루어지고, 볼트마다 푸시 잠금(push lock)과 멱등 재시도가 적용됩니다.
- **실시간 푸시.** 작은 편집은 Server-Sent Events(SSE)로 1초 이내에 도착합니다. 폴링은 만약에 대비한 안전망으로 남습니다.
- **Git이 유일한 기준(source of truth)입니다.** 모든 볼트가 bare git 저장소이므로 파일별 이력, unified diff, 단일 파일 복원이 별도 설정 없이 동작합니다 — 플러그인에서도 관리자 패널에서도.
- **충돌에 안전합니다.** 플러그인은 로컬 편집을 조용히 덮어쓰지 않습니다. 충돌은 `.conflict-*` 파일로 드러나고 클릭 한 번으로 해결할 수 있습니다.
- **관리자 패널**은 5개 언어(English, 简中, 繁中, 日本語, 한국어)로 제공됩니다. 사용자, 기기 토큰(device token), 볼트, 초대 코드, 활동 로그, blob GC를 여기서 관리하며 파괴적인 볼트/사용자 작업에는 확인 대화상자가 나타납니다.
- **AI가 읽을 수 있는 볼트.** MCP는 stdio 또는 `pkvsyncd serve`에 내장된 `/mcp` 라우트(기본 켜짐)로 읽기/쓰기 도구를 노출합니다.
- **기본값부터 안전 쪽으로.** 관리자가 만드는 비밀번호는 setup과 같은 수준의 강한 정책을 따르고, 토큰 시크릿은 한 번만 표시되며, 업로드와 MCP 응답에는 크기 상한이 걸리고, 라이브 SSE 스트림은 취소된 토큰을 다시 검증합니다.
- **의도적으로 단순하게.** 바이너리 하나, SQLite 메타데이터 DB 하나, 볼트마다 bare git 저장소 하나, 첨부 파일마다 content-addressed blob 하나.

## Docker Compose로 빠르게 시작

권장 경로입니다. `deploy/caddy/`의 Caddy가 Let's Encrypt로 HTTPS를 처리하고, PKV Sync는 compose 네트워크 안 `127.0.0.1:6710`에 머무릅니다. 공용 인터넷의 평문 HTTP는 서버에 닿지 않습니다.

도메인 이름(예: `sync.example.com`)이 필요하고, A/AAAA 레코드가 서버를 가리켜야 하며, 인터넷에서 `80`과 `443` 포트에 접근할 수 있어야 합니다(포트 80은 ACME HTTP-01 검증에 사용됩니다).

1. 배포 키(deployment key)를 생성합니다.

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` 옆에 `config.toml`을 둡니다.

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 출력으로 교체
   public_host    = "sync.example.com"   # 필수, 관리자 POST가 동작하려면 필요

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker 브리지 네트워크

   [mcp]
   embed_in_serve = true                 # 기본값. false로 비활성화
   ```

3. `deploy/caddy/Caddyfile`에서 `sync.example.com`을 실제 도메인으로 바꿉니다.

4. 스택을 띄웁니다.

   ```bash
   docker compose up -d
   ```

   브라우저에서 `https://sync.example.com/setup`을 열고 첫 관리자 계정을 만듭니다.

5. Obsidian에 `pkv-sync-plugin.zip`을 설치합니다(`<vault>/.obsidian/plugins/pkv-sync/`). 활성화한 뒤 관리자 패널의 공유 URL을 붙여넣고, 로그인하거나 가입해서 볼트를 고릅니다.

업데이트는 `docker compose pull && docker compose up -d`입니다. 네이티브 설치, 리버스 프록시 튜닝(Caddy / Nginx / Traefik), `public_host` 의미, 백업과 복원, 디스크 암호화는 [배포 강화 가이드](./public-docs/deployment-hardening.ko.md)를 참고하세요.

## MCP 배포 모드

MCP HTTP는 내장 방식만 제공됩니다. `[mcp].embed_in_serve`가 기본적으로 `true`이므로 `pkvsyncd serve`가 메인 서버 포트에 `/mcp`를 마운트하고 같은 TLS 종료, 리버스 프록시, 배포 키, bearer 토큰 검증을 공유합니다. `embed_in_serve = false`로 비활성화할 수 있습니다. 독립 실행형 `pkvsyncd mcp --transport http --bind` 프로세스는 이번 버전에서 제거되었으므로 기존 독립 배포는 메인 서버의 `/mcp`를 가리키도록 바꾸세요. 로컬 AI 도구는 stdio 트랜스포트(`pkvsyncd mcp --vault <id>`)도 사용할 수 있습니다.

## Obsidian 플러그인

로컬 파일이 유일한 기준입니다 — 플러그인은 디스크에 있는 평범한 Obsidian 볼트를 그대로 읽고 쓰며, 프록시 파일 시스템 같은 것을 만들지 않습니다. 플러그인의 민감하지 않은 설정과 동기화 인덱스는 `<vault>/.obsidian/plugins/pkv-sync/data.json`에 저장됩니다. 로그인 상태, 활성 bearer 기기 토큰, 배포 키, 안정적인 기기 신원은 Obsidian의 기기 로컬 저장소에 보관됩니다. Obsidian 기기 로컬 저장소, 평문 백업, 예전 버전이 남긴 플러그인 `data.json` 사본은 민감 정보로 취급하세요. 기기 토큰은 사용할 때마다 갱신되고 90일 동안 쓰지 않으면 만료되며, 토큰마다 365일의 절대 수명이 있습니다. 같은 기기에서 다시 로그인하면 활성 토큰이 교체됩니다.

명령 팔레트, 파일 이력, 나란히 보기 diff, 충돌 해결, 선택적 `.obsidian` 동기화, 기기 관리, 자가 업데이트 같은 일상 기능은 [사용자 매뉴얼](./public-docs/user-manual.ko.md)에서 모두 안내합니다.

## 현재 시점의 암호화

PKV Sync 1.0은 네이티브 종단 간 암호화를 아직 제공하지 **않습니다** — 서버가 볼트 내용을 읽을 수 있습니다. 볼트 단위 네이티브 E2EE는 1.x 로드맵에 옵트인 방식으로 계획되어 있습니다. 암호화는 Git-native PKV를 쓸모 있게 만드는 서버 쪽 기능(이력 diff, 3-way 자동 병합, 인라인 SSE 페이로드, MCP 읽기/쓰기)과 맞바꾸는 것이기 때문입니다.

네이티브 E2EE가 들어오기 전에 필요하다면 볼트에 [`git-crypt`](https://github.com/AGWA/git-crypt)를 얹으세요. 표시된 경로는 서버가 복호화할 수 없는 암호문 blob로 도달하고, 파일 이름은 서버에 평문으로 남습니다(대부분의 위협 모델에서 감수할 수 있는 수준입니다). 키를 가진 클라이언트는 표준 `git clone`과 `pkvsyncd materialize`를 계속 쓸 수 있습니다.

실제 배포에서는 추가로 HTTPS 뒤에서 실행하고 `trusted_proxies`를 좁히고, 데이터 디스크와 백업을 암호화하세요 — 자세한 내용은 [배포 강화 가이드](./public-docs/deployment-hardening.ko.md)에 있습니다.

## 찾고 계신 건…

| 주제 | 문서 |
| --- | --- |
| 일상적인 플러그인 사용 | [사용자 매뉴얼](./public-docs/user-manual.ko.md) |
| 서버 운영과 런타임 설정 | [관리자 매뉴얼](./public-docs/admin-manual.ko.md) |
| 모든 CLI 명령과 플래그 | [CLI 레퍼런스](./public-docs/cli-reference.ko.md) |
| 0.x에서 1.0으로 업그레이드 | [1.0 업그레이드 노트](./public-docs/upgrade-notes-v1.0.ko.md) |
| 리버스 프록시, TLS, 백업, 강화 | [배포 강화](./public-docs/deployment-hardening.ko.md) |
| HTTP API 계약 | [OpenAPI 명세](./public-docs/openapi.yaml) |
| MCP 설정과 도구 목록 | [MCP 사용법](./public-docs/mcp-howto.ko.md) |
| LLM이 유지 관리하는 Wiki 워크플로 | [LLM Wiki 사용법](./public-docs/llm-wiki-howto.ko.md) |
| Obsidian Sync에서 이전 | [이전 가이드](./public-docs/migrate-from-obsidian-sync.ko.md) |
| 보안 제보 | [SECURITY.md](./SECURITY.md) |
| 릴리스 이력 | [CHANGELOG.md](./CHANGELOG.md) |

## 상태

PKV Sync 1.6.0은 MCP를 일급 시민으로 끌어올립니다. 서버가 새 2026-07-28 스테이트리스 프로토콜 개정과 레거시 클라이언트를 함께 지원(듀얼 에라)하고, `subscriptions/listen` 푸시가 추가되었으며, MCP가 기본으로 메인 서버에 내장됩니다(독립형 `mcp --transport http` 배포는 제거). `public_host`를 설정할 수 없는 관리 배포를 위한 `allow_insecure_admin` 탈출구가 생겼고, 헬스 프로브 수정으로 셀프 업그레이드가 조용히 롤백되지 않고 드디어 제대로 동작합니다. 동기화 정확성 수정도 함께 이루어졌습니다. auto-merge가 커밋을 건너뛰지 않고(가져오지 않은 병합 베이스를 넘어 로컬 헤드가 진행되지 않음), 실시간 적용도 로컬 파일을 조용히 덮어쓰거나 되살리지 않습니다.

PKV Sync 1.5.0은 2026-08-16 전체 저장소 감사에서 나온 수정 사항을 모두 반영하고 성능 개선을 더했습니다. blob 보존이 git 객체의 생존을 따르므로 롤백과 파일 단위 이력 복원에서 첨부 파일이 사라지지 않고, Windows에서 문제를 일으키는 경로 구성 요소는 `materialize`에 닿기 전에 거부됩니다. 특권 업데이터는 다운그레이드를 거부하고 Docker 셀프 업그레이드 오버레이는 강화되었습니다. 등록과 blob 업로드 경로에는 빠져 있던 속도 제한이 추가되었고, 장시간 유지되는 SSE 연결은 우아한 종료를 막지 않습니다. 플러그인은 allowlist에 든 `.obsidian` 경로를 동기화하고 첫 동기화를 메모리 상한이 있는 배치로 스트리밍합니다(모바일 대상). 배치 git 읽기와 단일 복사 인덱스 갱신으로 MCP 검색과 큰 볼트 동기화가 빨라졌습니다.

PKV Sync 1.0은 첫 안정 릴리스입니다. 공개 REST API, CLI, 저장소 레이아웃, 플러그인 패키지, Docker 이미지가 같은 semver로 함께 관리됩니다. 1.X.Y는 공개 표면에서 하위 호환을 유지하고, OpenAPI 명세가 호환성 계약의 정본입니다. 0.x로 만든 SQLite 데이터베이스는 1.0.0으로 제자리 업그레이드할 수 없습니다 — [1.0 업그레이드 노트](./public-docs/upgrade-notes-v1.0.ko.md)를 따르세요.

GitHub 릴리스마다 Linux amd64/arm64 바이너리, Windows x64 바이너리, 멀티 아키텍처 GHCR Docker 이미지, Obsidian 플러그인 zip, `SHA256SUMS`가 함께 게시됩니다.

## 개발

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

cd plugin
npm run typecheck
npm exec vitest run
npm run build
```

CI는 Linux와 Windows에서 전체 Rust 매트릭스를 실행하고, 플러그인 테스트/타입체크/빌드/패키징, Docker 빌드, 릴리스 바이너리 스모크 테스트도 함께 돌립니다.

## 라이선스

AGPL-3.0-only. [LICENSE](./LICENSE)를 참고하세요.
