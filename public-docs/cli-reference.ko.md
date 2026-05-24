# CLI 참조

[English](./cli-reference.md) | [简体中文](./cli-reference.zh-CN.md) | [繁體中文](./cli-reference.zh-Hant.md) | [日本語](./cli-reference.ja.md) | 한국어

## pkvsyncd materialize

PKV Sync vault의 bare git repository를 디스크의 일반 파일 트리로 펼칩니다.

### Synopsis

```text
pkvsyncd materialize <vault-id> -o <output-dir> [--at <commit-sha>]
```

### Options

- `-o, --output <DIR>`: 출력 디렉터리입니다. 존재하지 않거나 비어 있어야 합니다.
- `--at <SHA>`: 특정 commit 기준으로 materialize합니다. 기본값은 HEAD입니다.

### Description

vault의 bare git repository를 읽고 각 파일을 출력 디렉터리에 씁니다.

- 텍스트 파일은 그대로 기록됩니다.
- `pkvsync_pointer` JSON으로 저장된 바이너리 파일은 server blob storage에서 실제 blob을 복사해 복원합니다.

이 명령은 동기적으로 실행되며 server가 실행 중일 필요가 없습니다. 설정된 `data_dir` 아래의 git repository와 blob storage를 직접 읽습니다.

### Examples

```bash
# 최신 버전 materialize
pkvsyncd materialize abc123 -o ./my-vault

# 특정 commit materialize
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```

### Exit Codes

- `0`: 성공.
- `1`: 오류. 출력 디렉터리가 비어 있지 않음, vault 없음, blob 누락, 잘못된 commit SHA 등.

## pkvsyncd mcp

AI 도구용 MCP server를 시작합니다.

### Synopsis

```text
pkvsyncd mcp [--transport stdio|http] [--vault <vault-id>] [--token <pks-token>] [--bind <addr>]
```

### Options

- `--transport <stdio|http>`: transport mode입니다. 기본값은 `stdio`입니다.
- `--vault <vault-id>`: stdio에서 필수이며, client에 노출할 단일 vault입니다.
- `--token <pks-token>`: stdio용 bearer device token입니다. 생략하면 `PKV_TOKEN`을 사용합니다.
- `--bind <addr>`: HTTP bind address입니다. 기본값은 `127.0.0.1:6711`입니다.

### Description

stdio mode는 stdin에서 JSON-RPC를 읽고 stdout에 JSON-RPC를 씁니다. HTTP mode는 `/mcp`에서 stateless Streamable HTTP MCP endpoint를 제공합니다. 두 mode 모두 `list_vaults`, `list_files`, `read_file`, `read_file_at_commit`, `search`, `write_file`, `delete_file`을 노출합니다.

### Examples

```bash
# stdio, token은 환경 변수에서 읽음
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault abc123

# 로컬 Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

HTTP mode에서는 모든 request에 server deployment key header가 필요합니다.

## pkvsyncd upgrade

PKV Sync release binary를 현재 실행 파일 옆에 side-by-side로 다운로드합니다.

### Synopsis

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <version>]
```

### Options

- `--dry-run`: 선택된 release, asset, target path를 표시하고 다운로드하지 않습니다.
- `--yes`: 확인 프롬프트를 건너뜁니다.
- `--version <version>`: 최신 release 대신 `1.0.0` 같은 특정 release를 다운로드합니다.

### Description

현재 platform에 맞는 release asset을 선택하고 `SHA256SUMS`로 검증한 뒤 현재 binary 옆에 `pkvsyncd.new`(Windows에서는 `pkvsyncd.new.exe`)를 기록하고 systemd/manual swap 절차를 출력합니다. 실행 중인 server를 hot replace하지 않습니다.

Docker와 Kubernetes 배포는 image tag를 pull하거나 변경한 뒤 service 또는 rollout을 다시 시작해야 합니다. container 환경을 감지하면 이 명령은 image 기반 안내를 출력하고 종료하며 binary를 쓰지 않습니다.

### Examples

```bash
# 업그레이드 계획 미리 보기
pkvsyncd upgrade --dry-run

# 최신 검증 binary 다운로드
pkvsyncd upgrade --yes

# 특정 release 다운로드
pkvsyncd upgrade --yes --version 1.0.0
```
