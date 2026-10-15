# PKV Sync

PKV Sync는 Obsidian Vault를 자체 호스팅으로 동기화하는 Git 네이티브 서버입니다.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

문서 버전: v1.6.1.

[English](./README.md) · [简体中文](./README.zh-CN.md) · [繁體中文](./README.zh-Hant.md) · [日本語](./README.ja.md) · 한국어

## 기능

### 동기화

- **다중 사용자·다중 Vault** 동기화. 인증된 기기를 통해 이루어지며, Vault별 푸시 잠금과 멱등 재시도를 갖춥니다.
- **실시간 푸시.** 작은 편집은 Server-Sent Events로 1초 이내에 반영되며, 폴링은 안전망으로 남아 있습니다.
- **Git이 유일한 정보원.** 각 Vault는 bare git 저장소이므로 파일별 기록, unified diff, 단일 파일 복원이 기본으로 동작합니다 — 플러그인에서도 관리자 패널에서도.
- **충돌로 데이터가 깨지지 않습니다.** 플러그인은 로컬 편집을 조용히 덮어쓰지 않습니다. 충돌은 `.conflict-*` 파일로 나타나며 클릭 한 번으로 해결됩니다.

### 운영

- **관리자 패널**은 5개 언어(English, 简中, 繁中, 日本語, 한국어)를 지원하며 사용자, 기기 Token, Vault, 초대 코드, 활동 로그, blob GC를 다룹니다. 파괴적인 Vault 작업과 사용자 작업에는 확인 대화상자가 표시됩니다.
- **AI가 읽을 수 있는 Vault.** MCP가 stdio 또는 `pkvsyncd serve`에 내장된 `/mcp` 경로(기본 활성)를 통해 읽기·쓰기 도구를 제공하며, 이 서버는 [MCP Registry](https://registry.modelcontextprotocol.io)에 `io.github.cyberkurry/pkv-sync`로 등록되어 있습니다.
- **LLM이 유지하는 wiki를 위해 설계되었습니다.** 읽기와 쓰기 외에도 agent는 frontmatter로 검색하고, 결과를 정확한 페이지네이션으로 넘겨 보고, 층별 링크를 lint하고, 근거가 낡은 페이지를 찾고, dry run과 콘텐츠 해시 가드를 거친 뒤에 쓰기를 수행할 수 있습니다 — [LLM Wiki 워크플로](./public-docs/llm-wiki-howto.ko.md) 문서를 참조하세요.
- **기본값부터 경계를 둡니다.** 관리자가 만드는 비밀번호에는 setup과 동일한 강도 정책을 적용하고, Token 시크릿은 한 번만 표시하며, 업로드와 MCP 응답에는 크기 상한을 두고, 상시 연결된 SSE 스트림은 폐기된 Token을 다시 검증합니다.

### 설계

- **의도적으로 성숙한 기술을 선택합니다.** 바이너리 하나, SQLite 메타데이터 DB 하나, Vault마다 bare git 저장소 하나, 첨부 파일마다 content-addressed blob 하나.

## Docker Compose로 빠르게 시작

권장 경로입니다. `deploy/caddy/`의 Caddy가 Let's Encrypt로 HTTPS를 종료하고, PKV Sync는 compose 네트워크 안의 `127.0.0.1:6710`에서 대기합니다. 공용 인터넷에서 평문 HTTP가 직접 들어오는 일은 없습니다.

도메인 이름(예: `sync.example.com`)이 필요합니다. A/AAAA 레코드가 이 서버를 가리켜야 하고, 인터넷에서 `80`과 `443` 포트에 접근할 수 있어야 합니다(80번은 ACME HTTP-01 검증에 사용합니다).

### 스택 설정하기

1. 배포 키를 생성합니다:

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` 옆에 `config.toml`을 둡니다:

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 출력으로 교체
   public_host    = "sync.example.com"   # 필수. 관리자 패널의 POST가 동작합니다

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker 브리지 네트워크

   [mcp]
   embed_in_serve = true                 # 기본값. false로 비활성화
   ```

3. `deploy/caddy/Caddyfile`을 편집해 `sync.example.com`을 실제 도메인으로 바꿉니다.

4. 스택을 띄웁니다:

   ```bash
   docker compose up -d
   ```

   브라우저에서 `https://sync.example.com/setup`을 열고 첫 관리자 계정을 만듭니다.

5. Obsidian에 `pkv-sync-plugin.zip`을 설치하고
   (`<vault>/.obsidian/plugins/pkv-sync/`에 넣습니다) 활성화한 뒤, 관리자 패널의
   공유 URL을 붙여 넣고 로그인하거나 가입해 Vault를 고릅니다.

### 업데이트

```bash
docker compose pull && docker compose up -d
```

### 다음에 읽을 것

네이티브 설치, 리버스 프록시 튜닝(Caddy / Nginx / Traefik), `public_host`의 의미, 백업과 복원, 디스크 암호화는 [배포 강화 가이드](./public-docs/deployment-hardening.ko.md)에 있습니다.

## MCP 배포 모드

MCP HTTP는 내장 방식만 제공합니다. `[mcp].embed_in_serve`가 기본적으로 `true`이므로 `pkvsyncd serve`가 메인 서버 포트에 `/mcp`를 마운트하고, 동일한 TLS 종료, 리버스 프록시, 배포 키, bearer Token 검증을 공유합니다. `embed_in_serve = false`로 비활성화할 수 있습니다. 단독 실행형 `pkvsyncd mcp --transport http --bind` 프로세스는 이번 버전에서 제거되었습니다. 기존 단독 배포는 메인 서버의 `/mcp`를 가리키도록 바꾸세요. 로컬 AI 도구는 stdio 트랜스포트(`pkvsyncd mcp --vault <id>`)도 사용할 수 있습니다.

## Obsidian 플러그인

로컬 파일이 유일한 정보원입니다. 플러그인은 디스크에 있는 평범한 Obsidian Vault를 그대로 읽고 쓰며, 프록시 파일 시스템을 쓰지 않습니다. 민감하지 않은 플러그인 설정과 동기화 인덱스는 `<vault>/.obsidian/plugins/pkv-sync/data.json`에 저장됩니다. 로그인 상태, 현재 활성 bearer 기기 Token, 배포 키, 안정적인 기기 식별자는 Obsidian의 기기 로컬 저장소에 저장됩니다. Obsidian 기기 로컬 저장소, 평문 백업, 예전 버전이 남긴 플러그인 `data.json` 사본은 모두 민감 정보로 취급하세요. 기기 Token은 사용할 때 갱신되고 90일 동안 쓰이지 않으면 만료되며, 절대 수명은 365일입니다. 같은 기기에서 다시 로그인하면 활성 Token이 교체됩니다.

일상적인 기능 — 명령 팔레트, 파일 기록, 나란히 보기 diff, 충돌 해결, `.obsidian` 선택 동기화, 기기 관리, 자동 업데이트 — 은 [사용자 매뉴얼](./public-docs/user-manual.ko.md)에서 다룹니다.

## 현재 시점의 암호화

PKV Sync 1.0은 네이티브 종단 간 암호화를 **아직 제공하지 않습니다**. 서버가 Vault 내용을 읽을 수 있습니다. Vault별 네이티브 E2EE는 1.x 로드맵에 옵트인 방식으로 계획되어 있습니다. 암호화를 넣으면 Git 네이티브 PKV를 유용하게 만드는 서버 측 기능(기록 diff, 3방향 자동 병합, SSE 인라인 페이로드, MCP 읽기·쓰기)과 맞바꾸게 되기 때문입니다.

E2EE가 나오기 전에 필요하다면 Vault에 [`git-crypt`](https://github.com/AGWA/git-crypt)를 겹쳐 두세요. 표시한 경로는 서버가 복호화할 수 없는 암호문 blob으로 도착합니다. 파일 이름은 서버에서 평문으로 남습니다(대부분의 위협 모델에서 허용됩니다). 키를 가진 클라이언트라면 표준 `git clone`과 `pkvsyncd materialize`가 계속 동작합니다.

실제 배포에서는 추가로 HTTPS 뒤에서 실행하고, `trusted_proxies`를 좁히고, 데이터 디스크와 백업을 암호화하세요 — [배포 강화 가이드](./public-docs/deployment-hardening.ko.md)를 참고하세요.

## 문서

| 주제 | 문서 |
| --- | --- |
| 플러그인의 일상적인 사용 | [사용자 매뉴얼](./public-docs/user-manual.ko.md) |
| 서버 관리와 실행 시점 설정 | [관리자 매뉴얼](./public-docs/admin-manual.ko.md) |
| 모든 CLI 명령과 플래그 | [CLI 레퍼런스](./public-docs/cli-reference.ko.md) |
| 0.x에서 1.0으로 업그레이드 | [1.0 업그레이드 노트](./public-docs/upgrade-notes-v1.0.ko.md) |
| 리버스 프록시, TLS, 백업, 강화 | [배포 강화](./public-docs/deployment-hardening.ko.md) |
| HTTP API 계약 | [OpenAPI 명세](./public-docs/openapi.yaml) |
| MCP 설정과 도구 목록 | [MCP 하우투](./public-docs/mcp-howto.ko.md) |
| LLM이 유지하는 wiki 워크플로 | [LLM Wiki 하우투](./public-docs/llm-wiki-howto.ko.md) |
| Obsidian Sync에서 마이그레이션 | [마이그레이션 가이드](./public-docs/migrate-from-obsidian-sync.ko.md) |
| 보안 제보 | [SECURITY.md](./SECURITY.md) |
| 릴리스 기록 | [CHANGELOG.md](./CHANGELOG.md) |

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

CI는 Linux와 Windows에서 Rust 전체 매트릭스를 돌리고, 플러그인의 test／typecheck／build／package, Docker 빌드, 릴리스 바이너리 스모크 테스트를 함께 실행합니다.

## 기여

버그 제보, 기능 요청, Pull Request를 환영합니다. 버그 수정보다 큰 변경은 먼저 issue를 열어 논의해 주세요. 빌드와 테스트 명령, 커밋과 릴리스 흐름, CI가 검증하는 문서 규칙은 [CONTRIBUTING.md](./CONTRIBUTING.md)에 있습니다.

## 라이선스

AGPL-3.0-only. [LICENSE](./LICENSE)를 참고하세요.
