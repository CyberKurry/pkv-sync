# PKV Sync 관리자 설명서

[English](./admin-manual.md) | [简体中文](./admin-manual.zh-CN.md) | [繁體中文](./admin-manual.zh-Hant.md) | [日本語](./admin-manual.ja.md) | 한국어

문서 버전: v1.2.2.

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

이 설명서는 자체 호스팅 PKV Sync 서버의 일상적인 관리를 다룹니다. 네트워크와 호스트 강화는 배포 강화 가이드도 함께 읽어 주세요.

## 최초 실행

1. 배포 키를 생성합니다.

   ```bash
   pkvsyncd genkey
   ```

2. `config.example.toml`을 기반으로 `/etc/pkv-sync/config.toml`을 만듭니다.
3. 새로운 1.x 데이터 디렉터리용 v1 데이터베이스 baseline을 초기화합니다.

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. 서버를 시작합니다.

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 새 데이터베이스를 처음 시작한 뒤 브라우저에서 `/setup`을 열고 첫 관리자 계정을 만듭니다. PKV Sync는 임의의 관리자 비밀번호를 stderr 또는 컨테이너 로그에 출력하지 않습니다.
6. setup이 끝난 뒤 일반 관리자 로그인에는 `/admin/login`을 사용합니다.

PKV Sync 1.0은 단일 v1 SQLite baseline을 사용합니다. 0.x에서 만든 데이터베이스는 1.0.0으로 인플레이스 upgrade할 수 없습니다. [`upgrade-notes-v1.0.ko.md`](./upgrade-notes-v1.0.ko.md) 절차를 따르세요. 이 v1 baseline 이후 게시되는 1.x migrations는 append-only입니다.

## Admin Web 패널

열기:

```text
https://sync.example.com/admin/login
```

웹 패널에는 다음이 포함됩니다.

- 시스템, 스토리지, vault, 사용자, 최근 활동 지표가 있는 대시보드
- 검색과 상태 필터가 있는 사용자 목록
- 비밀번호 재설정, 활성/관리자 제어, token 확인을 위한 사용자 상세 페이지
- token을 나열, 생성, 철회하는 전역 장치 token 페이지
- 소유자, 파일 수, 크기, 마지막 동기화, reconcile, 삭제 작업 및 vault별 동기화 설정이 있는 vault 카드
- 파일 미리보기, 파일별 기록 타임라인, unified diff 렌더링을 지원하는 읽기 전용 vault 파일 브라우저
- 선택적 만료 시간이 있는 초대 생성, 활성 초대 목록, 사용하지 않은 초대 삭제
- General, Security, Sync & Storage, Network로 묶인 런타임 설정. 업데이트 확인 on/off와 간격도 포함됩니다
- 동기화, vault 수명 주기, 읽기 전용 탐색 행을 실제 사용자와 작업으로 필터링하는 활동 로그
- Blob 가비지 컬렉션 트리거
- 영어, 중국어 간체, 중국어 번체, 일본어, 한국어 언어 전환

1.2.1에서는 사용자 상세 통계가 실제 vault 수와 마지막 동기화 시각에 기반하며, 기간 라벨은 함께 제공되는 모든 admin 언어에 맞게 현지화됩니다. reconciliation 및 metadata repair 처리도 가능한 경우 증분 처리 또는 batch 처리를 사용합니다.

타임스탬프, 기간, 바이트 크기, 가동 시간, 활동 데이터는 사람이 읽기 쉬운 형식으로 표시됩니다. 기본 시간대는 `Asia/Shanghai`이며 설정에서 변경할 수 있습니다.

## 업데이트 알림

PKV Sync는 기본적으로 24시간마다 GitHub release를 확인합니다. 더 새 서버 release가 있으면 대시보드에 현재 버전, 최신 버전, release notes 링크, 짧은 요약이 있는 배너를 표시합니다.

`config.toml`의 `[update_check].enabled`와 `[update_check].interval_seconds`는 새 데이터베이스의 첫 시작 때 런타임 설정으로 seed됩니다. 이후에는 Admin WebUI Settings 페이지가 우선합니다. **Network** 섹션에서 업데이트 확인을 켜거나 끄고 간격을 바꿀 수 있으며, 백그라운드 작업은 다음 주기에서 새 런타임 값을 다시 읽습니다. 현재 비활성 상태라면 다시 켠 뒤 약 60초 안에 반영됩니다. `[update_check].repo`는 에어갭 mirror 배포를 위한 정적 `config.toml` 필드로 유지됩니다.

```toml
[update_check]
enabled = false
interval_seconds = 86400
repo = "cyberkurry/pkv-sync"
```

업데이트 확인은 정보 제공용입니다. PKV Sync는 실행 중인 서버 바이너리나 컨테이너 이미지를 자동으로 교체하지 않습니다.

## 사용자 관리

- **Users** 또는 CLI에서 사용자를 만듭니다.
- 사용자 이름은 3-32자의 ASCII 문자, 숫자, `_`, `-`, `.`이어야 합니다.
- 관리자가 생성/재설정하는 비밀번호, 공개 등록 비밀번호, 사용자가 직접 변경하는 비밀번호는 모두 12자 이상이어야 하며 대문자, 소문자, 숫자를 포함해야 합니다.
- Users 페이지의 검색과 상태 필터로 표를 좁힐 수 있습니다.
- 사용자 상세 페이지에서 비밀번호를 재설정하고, 계정을 활성화 또는 비활성화하고, 관리자 권한을 승격 또는 강등하고, 해당 사용자의 장치 token을 확인할 수 있습니다.
- 나중에 감사 기록이 필요할 수 있으면 사용자를 삭제하는 대신 비활성화하세요.
- Admin WebUI는 사용자를 비활성화하거나 관리자를 강등하기 전에 확인 대화상자를 표시합니다. 자신의 관리자 세션 비활성화와 마지막 관리자 강등은 차단되며 사용자 상세 페이지에 현지화된 피드백이 표시됩니다.
- 남아 있는 모든 관리자 계정을 비활성화하지 마세요.

Admin WebUI에서 비밀번호를 재설정하면 해당 사용자의 기존 장치 token이 철회됩니다. 사용자는 다시 로그인해야 합니다.

CLI 대체 명령:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 장치 Token

장치 bearer token은 인증된 사용 시 갱신되며 90일 동안 사용하지 않으면 만료되고, 각 token에는 365일의 절대 수명이 있습니다. 사용자는 자신의 token을 철회할 수 있고, 관리자는 모든 사용자의 token을 철회할 수 있습니다.

운영 참고 사항:

- Token 평문은 생성 시 한 번만 표시됩니다.
- 데이터베이스에는 SHA-256 token hash만 저장됩니다.
- 관리자 token 목록 endpoint와 표는 공개 token 메타데이터만 표시하며, 평문 token이나 내부 만료/철회 필드는 반환하지 않습니다.
- 모든 인증된 요청은 token 만료 시간을 해당 요청 시각으로부터 90일 뒤로 연장하되 token 생성 후 365일을 넘지 않습니다.
- 같은 안정적인 플러그인 장치 ID에서 다시 로그인하면 그 장치의 이전 활성 token이 대체됩니다.
- 활동 행에서 참조하는 철회된 token은 활동 기록을 보존한 채 정리할 수 있습니다.

## Vault

Admin WebUI에서 vault를 삭제하려면 추가 확인 대화상자가 필요합니다. 참조되지 않는 blob이 garbage collection 전까지 남을 수 있더라도 삭제는 파괴적 작업으로 취급하세요.

vault를 삭제하면 다음이 제거됩니다.

- vault 데이터베이스 행
- 해당 행에서 cascade되는 관련 메타데이터 행
- `data_dir/vaults/<vault-id>` 아래의 백엔드 bare Git 저장소
- 메모리의 vault별 push 잠금

Blob 파일은 콘텐츠 주소 지정 방식이며, 가비지 컬렉션이 유예 기간을 지나 참조되지 않음을 확인할 때까지 남아 있을 수 있습니다.

중단된 작업 후 파일 수, 크기 또는 blob 참조가 잘못 보이면 vault 메타데이터 reconciliation을 사용하세요. Reconciliation은 tree entry에서 blob pointer hash를 직접 읽고 blob 참조 복구를 batch 처리하므로 pointer 파일을 하나씩 다시 열 필요가 없습니다.

### Vault별 동기화 설정

**Vaults**에서 vault 카드의 **Settings**를 열어 vault별 `extra_sync_globs` allowlist를 편집합니다. 이 설정은 선택된 `.obsidian` 설정 파일을 포함한 숨김 경로 중 동기화 가능한 항목을 제어합니다.

새 vault는 권장 starter allowlist를 자동으로 받습니다. 기존 vault는 관리자 또는 vault 소유자가 starter list를 적용할 때까지 비어 있습니다. **Apply starter allowlist** 작업은 테마, CSS snippets, 단축키, 앱 환경설정, 모양 환경설정, 활성화된 플러그인 목록에 대한 권장 목록을 씁니다.

### 읽기 전용 파일 기록

**Vaults**에서 vault 카드의 **Browse files**를 엽니다. 브라우저는 현재 HEAD 파일을 크기와 텍스트/바이너리 종류와 함께 나열합니다. 파일을 열면 텍스트 파일은 읽기 전용 미리보기를 표시하고 **History** 및 **Diff with previous** 링크를 제공합니다.

기록 페이지는 해당 파일의 commit을 나열하고, 각 commit 시점의 파일과 해당 diff로 연결합니다. diff 페이지는 unified diff 행을 추가/삭제/hunk 색상으로 렌더링합니다. 바이너리 파일은 메타데이터만 표시하고 바이너리 diff 내용은 렌더링하지 않습니다. 현재 동기화 필터에서 거부된 경로는 파일 탐색, commit list, history, diff 화면에서 숨겨집니다.

파일, 기록, diff 탐색은 `view_commit`, `view_history`, `view_diff` 활동 행을 기록합니다. Vault rollback controls는 Admin history에서 사용할 수 있습니다. 대상 commit을 확인한 뒤 사용하세요. rollback은 선택한 기록 지점에서 새 vault 상태를 만듭니다.

## 초대와 등록

**Settings**에서 등록을 설정합니다.

- `disabled`: 관리자만 계정을 만듭니다
- `invite_only`: 사용자가 초대 코드로 등록합니다
- `open`: 배포 URL을 가진 누구나 등록할 수 있습니다

초대 생성 시 선택적으로 미래 만료 시간을 지정할 수 있습니다. Admin WebUI는 사람이 읽는 날짜-시간 입력을 사용하고 내부적으로 Unix 초를 저장합니다. 사용된 초대는 admin API에서 삭제할 수 없으며 감사 기록으로 보관하세요.

`open`은 짧은 시간 창 또는 추가 모니터링과 속도 제한이 있는 공개 배포에서만 사용하세요.

## 런타임 설정

Settings 페이지는 SQLite에 저장된 값을 편집합니다. 변경 사항은 새 요청에 즉시 적용되며 저장 시 메모리 캐시가 갱신됩니다.

**General** — 서버 이름, 기본 시간대, `enable_metrics` 메트릭 스위치. 활성화하면 `/metrics`를 사용할 수 있지만 배포 키 middleware, 플러그인 User-Agent guard, 관리자 bearer token이 계속 필요합니다.

**Security** — 등록 모드(`disabled` / `invite_only` / `open`), 로그인 실패 임계값, 실패 창, 잠금 기간. 로그인 속도 제한기는 실패 횟수와 진행 중인 비밀번호 검증을 모두 계산하므로 동시 추측 폭주로 임계값을 우회할 수 없습니다. 인증된 동기화 API 경로는 경로, 메서드, 클라이언트 IP, bearer 장치 token별로 60초당 최대 600개 요청의 고정 창 제한을 적용합니다. 실패한 bearer token 인증 시도도 클라이언트 IP별로 60초당 최대 120회로 제한되므로 가짜 token을 바꿔 가며 실패 예산을 우회할 수 없습니다.

**Sync & Storage**
- 최대 파일 크기(기본값 `100 MiB`). Blob upload request body는 이 runtime 설정을 더 높여도 항상 hard storage cap(프로덕션 `512 MiB`)으로 제한됩니다
- 지원되는 텍스트 확장자 — 목록 밖의 파일은 바이너리 blob으로 처리됩니다. 이 목록은 Admin WebUI에서 읽기 전용으로 표시됩니다. 변경이 필요하면 `text_extensions` 런타임 설정 행(또는 SQLite `runtime_config` 테이블을 직접 편집)으로 수정하세요.
- 추가 exclude glob — 내장 `.obsidian/`, `.trash/`, `.conflict-*`, `.git/` 제외 목록을 보완하는 관리자 조정 가능 패턴
- 기록 UI와 diff 엔드포인트 토글
- **Auto-merge text**(`enable_auto_merge`, 기본 켜짐): 활성화하면 서버는 conflict 파일을 쓰기 전에 3-way 라인 병합을 시도합니다. 겹치지 않는 편집은 깔끔하게 병합되며, 겹치는 편집은 여전히 merge 마커가 포함된 conflict 파일을 만듭니다.
- **Push debounce**(`push_debounce_ms`, 기본값 `250`): 로컬 편집이 안정된 뒤 push하기 전까지 기다리는 시간입니다. 낮추면 종단 간 지연이 줄고, 높이면 push당 더 많은 입력을 묶습니다
- **Inline SSE content cap**(`inline_content_max_bytes`, 기본값 `8192`, 최대 `65536`): 이 크기 이하의 텍스트 변경은 SSE 이벤트 안에 실려 수신 플러그인이 별도 pull 없이 적용할 수 있습니다. 더 큰 파일은 pull로 대체됩니다
- **SSE heartbeat**(`sse_heartbeat_seconds`, 기본값 `30`): 유휴 SSE 연결이 리버스 프록시에서 끊기지 않도록 하는 이벤트 스트림 keep-alive입니다. 동시 SSE 구독은 기본적으로 사용자당 16개로 제한되며 전역 상한 1024를 유지합니다. 열려 있는 이벤트 스트림은 bearer token을 주기적으로 재검증하며, token 철회나 계정 비활성화 후 닫힙니다.
- **Git smart HTTP**(`enable_git_smart_http`, 기본값 꺼짐): 켜면 권한 있는 장치가 `git clone https://_:<token>@host/git/<vault-id>`를 사용할 수 있습니다. 서버에는 `PATH` 안의 `git` 바이너리도 필요하며, 공개 `/api/config` capability는 두 조건을 모두 반영합니다.

**Network and update checks** — `public_host`, bind address, trusted proxies, `[update_check].repo`는 시작 시 `config.toml`에서 읽습니다. 업데이트 확인 활성 상태와 간격은 SQLite에 저장되는 런타임 설정입니다. 허용 범위는 60초부터 30일까지입니다.

## 활동

활동 로그는 push, pull, create_vault, delete_vault, view_commit, view_history, view_diff 같은 동기화, vault 수명 주기, 읽기 전용 탐색 작업을 기록합니다. 포함 항목:

- user
- vault
- action
- device name
- file count
- byte size
- client IP
- User-Agent
- details
- timestamp

활동 필터를 사용해 특정 사용자나 작업 유형을 확인할 수 있습니다.

`create_vault`와 `delete_vault`는 관리 패널, 플러그인, API의 vault 생성/삭제 작업에서 옵니다.

## 서버 URL 공유

서버 또는 Admin WebUI가 출력하는 URL을 공유합니다.

```text
https://sync.example.com/k_xxx/
```

민감한 정보로 다루세요. 사용자 비밀번호는 아니지만 배포 키를 포함하며 플러그인 API 트래픽의 첫 번째 사전 인증 관문입니다.

## 유지보수 체크리스트

- 운영 스냅샷에는 `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]`을 사용합니다. 출력 디렉터리는 없거나 비어 있어야 합니다. 명령은 `VACUUM INTO`로 SQLite를 스냅샷하고, `vaults/`와 `blobs/`를 복사하며, pkvsyncd 버전, 컴포넌트 hash, 크기, 개수가 담긴 `MANIFEST.json`을 씁니다. 기본 백업은 `config.toml`을 생략합니다. 배포 키와 기타 로컬 비밀을 저장하고 보호하려는 경우에만 `--include-config`를 추가하세요.
- `pkvsyncd restore --input <backup-dir> --data-dir <dir>`로 없거나 빈 데이터 디렉터리에 복원합니다. 대상을 먼저 비워도 된다는 것을 확인한 경우에만 `--force`를 추가하세요. restore는 복사 전에 manifest hash를 확인하고 이후 verify를 실행합니다.
- 유지보수 후 또는 호스트 스토리지 사고 후 `pkvsyncd verify [--data-dir <dir>]`를 실행합니다. 참조된 blob 파일을 검사하고, 고아 blob을 보고하며, `git2`로 vault git 저장소를 검증하고, 누락, 손상, git 오류가 있으면 0이 아닌 값으로 종료합니다. `--no-fail`은 보고서를 유지하되 성공 종료 코드를 강제합니다.
- `pkvsyncd materialize <vault-id> -o <dir>`로 vault의 HEAD를 일반 파일 트리로 내보냅니다(텍스트 파일은 그대로, 바이너리 blob은 blob store에서 해석). 오프라인 내보내기, 임시 감사 또는 콜드 마이그레이션에 유용합니다. 과거 commit을 materialize하려면 `--at <commit-sha>`와 함께 사용하세요.
- `[mcp].embed_in_serve = true`를 설정하면 메인 `pkvsyncd serve` 포트의 `/mcp`에서 읽기/쓰기 MCP Streamable HTTP endpoint를 노출합니다. 또는 `pkvsyncd mcp --transport http --bind 127.0.0.1:6711`을 독립 MCP 프로세스로 실행할 수 있습니다. 단일 vault stdio 세션은 `pkvsyncd mcp --vault <id>`를 사용하세요.
- 대량 첨부 삭제 후 blob 가비지 컬렉션을 실행합니다.
- 로그와 활동에서 반복되는 `401`, `403`, `404`, `409`, `429` 응답을 확인합니다.
- 서버 바이너리, 플러그인 패키지, Docker 이미지, 리버스 프록시, 호스트 OS를 최신 상태로 유지합니다.
- release tag를 만들기 전에 CI를 확인합니다.
- 각 release에 Linux amd64, Linux arm64, Windows x64, 플러그인 zip, checksums, GHCR Docker 이미지 tag가 포함되어 있는지 확인합니다.
