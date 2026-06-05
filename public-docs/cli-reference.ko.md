# CLI 레퍼런스

[English](./cli-reference.md) | [简体中文](./cli-reference.zh-CN.md) | [繁體中文](./cli-reference.zh-Hant.md) | [日本語](./cli-reference.ja.md) | 한국어

`pkvsyncd`는 PKV Sync 서버 데몬 바이너리입니다. HTTP/WebSocket 동기화 API, 관리자 UI, MCP 서버, 그리고 소수의 운영용 서브커맨드를 호스팅합니다.

## 글로벌 옵션

다음 플래그는 모든 서브커맨드에 공통으로 적용됩니다.

- `-c, --config <PATH>`: TOML 설정 파일 경로입니다. 기본값: `/etc/pkv-sync/config.toml`.
- `-h, --help`: 도움말을 표시합니다.
- `-V, --version`: CLI 버전을 출력합니다.

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 서브커맨드

`pkvsyncd`는 9개의 서브커맨드를 제공합니다. 가장 자주 사용되는 운영 흐름은 `serve`, `genkey`, `migrate up`, `user add`, `backup`, `restore`입니다.

## pkvsyncd serve

HTTP 서버를 시작합니다.

### 개요

```text
pkvsyncd serve
```

### 설명

퍼블릭 동기화 HTTP 리스너, 관리자 UI, SSE 스트림, Git smart HTTP 라우트, 그리고 설정된 경우 MCP HTTP 엔드포인트를 실행합니다. 리스너는 `config.toml`의 `[server].bind_addr`에 바인딩됩니다. systemd 아래나 컨테이너 안에서 포그라운드 프로세스로 실행하십시오.

### 예시

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

데이터베이스 마이그레이션 커맨드입니다. 유일한 작업은 `up`입니다.

### 개요

```text
pkvsyncd migrate up
```

### 설명

`server/migrations/`에 있는 모든 미적용 SQLite 마이그레이션을 `[storage].db_path`의 데이터베이스에 대해 실행합니다. 재실행해도 안전하며, 이미 적용된 마이그레이션은 건너뜁니다. HTTP 서버 또한 시작 시점에 미적용 마이그레이션을 실행하므로, 수동 `migrate up`은 일반적으로 콜드 복구 흐름이나 오프라인 백업을 마이그레이션할 때에만 필요합니다.

### 예시

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

`[server].deployment_key`에 적합한 무작위 배포 키를 생성합니다.

### 개요

```text
pkvsyncd genkey
```

### 설명

암호학적으로 무작위인 `k_*` 토큰을 stdout으로 출력합니다. 그 값을 `config.toml`에 붙여넣고 자체적인 안전한 채널을 통해 플러그인/관리자 클라이언트에 공유하십시오.

### 예시

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

사용자 관리 커맨드입니다. 운영 복구(비밀번호 분실, 계정 잠금) 및 보조 운영자 계정의 스크립트 기반 부트스트래핑에 유용합니다.

### 개요

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 서브커맨드

- `add <USERNAME> [--admin]`: 사용자를 생성하며, 비밀번호를 대화형으로 입력받습니다.
- `passwd <USERNAME>`: 사용자의 비밀번호를 재설정하며, 새 값을 대화형으로 입력받습니다.
- `list`: 모든 사용자를 관리자/활성 상태 및 생성 시각과 함께 나열합니다.
- `set-active <USERNAME> --active <true|false>`: 사용자를 비활성화하거나 다시 활성화합니다. 비활성화된 사용자는 토큰은 유지되지만 로그인이나 동기화는 불가능합니다.

### 예시

```bash
# 비상 접근용 관리자 계정 생성
pkvsyncd user add alice --admin

# 잊어버린 비밀번호 재설정
pkvsyncd user passwd alice

# 데이터를 삭제하지 않고 떠나는 사용자 비활성화
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

PKV Sync 볼트의 bare git 저장소를 디스크의 일반 파일 트리로 펼쳐냅니다.

### 개요

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 옵션

- `-o, --output <DIR>`: 출력 디렉터리입니다(존재하지 않거나 비어 있어야 합니다).
- `--at <SHA>`: 특정 commit 시점에서 materialize합니다(기본값: HEAD).

### 설명

`data_dir/vaults/<vault-id>` 아래의 볼트 bare git 저장소를 읽어 각 파일을 출력 디렉터리에 기록합니다.

- 텍스트 파일은 그대로 기록됩니다.
- `pkvsync_pointer` JSON으로 저장된 바이너리 파일은 서버의 blob 저장소(`data_dir/blobs/`)에서 실제 blob을 복사하여 해석됩니다.

이 커맨드는 동기식이며 서버가 실행 중일 필요가 없습니다. 설정된 `data_dir` 아래의 온디스크 git 저장소와 blob 저장소에서 직접 읽습니다.

### 예시

```bash
# 최신 버전 materialize
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 특정 commit materialize
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 종료 코드

- `0`: 성공.
- `1`: 출력 디렉터리가 비어 있지 않음, 볼트를 찾을 수 없음, blob 누락, 잘못된 commit SHA 등의 오류.

> 볼트 ID는 32자리 소문자 16진수입니다(대시 없음). 위 예시는 실제 형식의 ID를 사용합니다. 관리자 UI와 `pkvsyncd user list`에 유효한 ID가 표시됩니다.

## pkvsyncd backup

서버 데이터를 휴대 가능한 백업 디렉터리로 스냅숏합니다.

### 개요

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 옵션

- `-o, --output <DIR>`: 백업 출력 디렉터리입니다(존재하지 않거나 비어 있어야 합니다).
- `--data-dir <DIR>`: 오프라인 작업을 위한 데이터 디렉터리 오버라이드입니다. 기본값은 로드된 설정의 `[storage].data_dir`입니다.
- `--gzip`: 백업 디렉터리 옆에 `.tar.gz` 아카이브도 함께 생성합니다.
- `--include-config`: 로드한 `config.toml`을 백업에 포함합니다. 기본 백업은 배포 키와 로컬 비밀이 들어 있을 수 있어 config를 제외합니다.

### 설명

SQLite 데이터베이스(VACUUM INTO를 통해 원본을 차단하지 않음), 모든 볼트의 bare git 저장소, 그리고 blob 저장소를 `MANIFEST.json`이 포함된 자체 완결형 디렉터리로 스냅숏합니다. 백업 중에도 HTTP 서버는 계속 실행될 수 있으며, 볼트 push는 해당 저장소가 복사되는 동안 볼트 단위로 잠시 정지됩니다.

기본적으로 백업은 `config.toml`을 생략합니다. 설정을 저장하고 그 안의 비밀을 보호하려는 경우에만 `--include-config`를 추가하세요.

### 예시

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

백업 디렉터리를 데이터 디렉터리에 복원합니다.

### 개요

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 옵션

- `-i, --input <DIR>`: `MANIFEST.json`이 포함된 백업 디렉터리입니다.
- `--data-dir <DIR>`: 대상 데이터 디렉터리 오버라이드입니다. 기본값은 `[storage].data_dir`입니다.
- `--force`: 복원 전에 비어 있지 않은 대상 데이터 디렉터리를 비웁니다.

### 설명

백업의 `MANIFEST.json`을 검증한 뒤 SQLite DB, 볼트 저장소, blob 저장소를 대상 데이터 디렉터리로 복사합니다. 복원 전에 HTTP 서버를 중지하십시오. 더 오래된 서버 버전에서 만들어진 백업을 복원하는 경우, 복원 후 `pkvsyncd migrate up`을 실행하십시오.

### 예시

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

볼트 git 저장소와 콘텐츠 주소 지정 blob을 검증합니다.

### 개요

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 옵션

- `--data-dir <DIR>`: 데이터 디렉터리 오버라이드입니다.
- `--no-fail`: 검증에서 오류가 발견되더라도 종료 코드 0을 반환합니다. 페이징 없이 로그만 남기려는 모니터링 스크립트에 유용합니다.

### 설명

`data_dir/vaults/` 아래의 각 볼트에 대해 다음을 수행합니다.

- bare 저장소에서 `git fsck --strict`를 실행합니다.
- HEAD 트리를 순회하며 모든 `pkvsync_pointer`가 그 파일명과 일치하는 온디스크 SHA-256을 가진 blob으로 해석되는지 검증합니다.

볼트별 오류 개수를 보고합니다. 어떤 볼트라도 오류가 있으면 0이 아닌 코드로 종료하며, `--no-fail`이 설정된 경우에는 그렇지 않습니다.

### 예시

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

AI 도구를 위한 MCP(Model Context Protocol) 서버를 시작합니다.

### 개요

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 옵션

- `--transport <stdio|http>`: 전송 모드입니다. 기본값: `stdio`.
- `--vault <VAULT-ID>`: stdio에서는 필수이며, 클라이언트에 노출되는 단일 볼트입니다.
- `--token <PKS-TOKEN>`: stdio용 bearer 디바이스 토큰입니다. 생략하면 `PKV_TOKEN` 환경 변수가 사용됩니다.
- `--bind <ADDR>`: HTTP 바인딩 주소입니다. 기본값: `127.0.0.1:6711`.

### 설명

`stdio` 모드는 stdin에서 JSON-RPC를 읽고 stdout으로 JSON-RPC를 씁니다. `http` 모드는 `/mcp`에서 무상태 Streamable HTTP MCP 엔드포인트를 제공합니다. 두 모드 모두 동일한 툴셋, 즉 `list_vaults`, `list_files`, `read_file`, `read_file_at_commit`, `search`, `write_file`, `delete_file`을 노출합니다. 쓰기 툴은 `(token, vault)`마다 분당 60회 쓰기로 속도 제한됩니다.

`http` 모드는 일반 동기화 API와 마찬가지로 모든 요청에 서버 배포 키 헤더를 포함해야 합니다.


이 서브커맨드는 계속 독립 MCP 프로세스입니다. 같은 Streamable HTTP transport를 메인 서버 포트에서 제공하려면 `[mcp].embed_in_serve = true`를 설정하고 `pkvsyncd serve`를 사용하세요.
### 예시

```bash
# 환경에서 가져온 토큰으로 stdio 실행
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 로컬 Streamable HTTP 엔드포인트
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

PKV Sync 릴리스 바이너리를 현재 실행 파일 옆에 함께 다운로드합니다.

### 개요

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 옵션

- `--dry-run`: 아무것도 다운로드하지 않고 선택된 릴리스, 에셋, 대상 경로를 표시합니다.
- `--yes`: 대화형 확인 프롬프트를 건너뜁니다.
- `--version <VERSION>`: 최신 릴리스 대신 `1.0.11` 같은 특정 릴리스를 다운로드합니다.

### 설명

이 커맨드는 현재 플랫폼에 해당하는 릴리스 에셋을 선택하고, 다운로드를 `SHA256SUMS`와 대조하여 검증하며, 현재 바이너리 옆에 `pkvsyncd.new`(Windows에서는 `pkvsyncd.new.exe`)를 기록한 뒤, systemd/수동 교체 절차를 출력합니다. 실행 중인 서버를 핫 리플레이스하지는 않습니다.

Docker 및 Kubernetes 배포는 이미지 태그를 풀하거나 변경한 다음 서비스를 재시작하거나 롤아웃하는 방식으로 업그레이드해야 합니다. 컨테이너 환경을 감지하면 이미지 기반 안내를 출력하고 바이너리를 기록하지 않은 채 종료합니다.

### 예시

```bash
# 업그레이드 계획 미리보기
pkvsyncd upgrade --dry-run

# 검증된 최신 바이너리 다운로드
pkvsyncd upgrade --yes

# 특정 릴리스 다운로드
pkvsyncd upgrade --yes --version 1.0.11
```
