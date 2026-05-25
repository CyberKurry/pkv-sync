# AI 도구용 MCP 접근

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | 한국어

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

PKV Sync는 MCP server를 통해 vault 내용을 노출할 수 있습니다. 서버는 파일 내용을 반환하기 전에 blob pointers를 해석하고, 명시적인 read-write tools를 통해 쓰기도 할 수 있으며, 일반 PKV Sync bearer device token이 필요합니다.

## Tools

- `list_vaults`: 인증된 사용자가 사용할 수 있는 vault를 나열합니다.
- `list_files {vault_id, at?}`: HEAD 또는 `at`이 지정된 경우 해당 commit SHA의 paths를 나열합니다.
- `read_file {vault_id, path}`: HEAD의 파일을 읽습니다.
- `read_file_at_commit {vault_id, path, commit}`: 특정 commit의 파일을 읽습니다.
- `search {vault_id, query, at?, limit?}`: 텍스트 파일에서 대소문자를 구분하지 않는 substring search를 수행합니다. `at`은 과거 commit으로 범위를 한정하고, `limit`은 반환되는 일치 수의 상한을 지정합니다.
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit`을 사용한 optimistic concurrency로 텍스트 파일을 만들거나 업데이트합니다.
- `delete_file {vault_id, path, parent_commit}`: `parent_commit`을 사용한 optimistic concurrency로 파일을 삭제합니다.

## stdio transport

명령을 실행하는 로컬 AI 도구에는 stdio를 사용합니다. stdio mode는 하나의 vault로 scope됩니다.

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

token을 직접 전달할 수도 있습니다.

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

클라이언트가 이미 실행 중인 로컬 또는 내부 MCP endpoint에 연결할 때는 HTTP를 사용합니다.

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

endpoint:

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

모든 요청에는 다음이 포함되어야 합니다.

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

배포 키는 주 PKV Sync 서버와 같은 설정 파일에서 읽습니다. 키가 없거나 잘못되면 bearer token 인증 전에 HTTP `404`를 반환합니다.

MCP HTTP는 고정 창 방식으로 60초당 120개 요청으로 제한됩니다. 제한을 초과하면 서버는 HTTP `429`와 JSON-RPC error code `-32029`를 반환합니다.

POST는 JSON-RPC tool calls를 담고 JSON responses를 반환합니다. `Accept: text/event-stream`이 있는 GET은 `vault_changed` notifications를 구독합니다. Event ids는 `<vault-id>:<commit-sha>`를 사용하며, 재연결 시 `Last-Event-ID`로 되돌려 보내 missed commits를 replay할 수 있습니다. Replay에는 상한이 있습니다. 서버가 missed history를 커버할 수 없으면 `lagged`를 내보내며, 클라이언트는 sync API에서 새로 고쳐야 합니다.

신뢰할 수 있는 네트워크 제어 뒤에 두지 않는 한 HTTP를 loopback에 bind하세요. bearer token은 해당 사용자가 소유한 모든 vault에 대한 읽기 및 쓰기 접근 권한을 부여합니다.

## Write tools

PKV Sync는 읽기 tools와 함께 두 개의 MCP write tools를 제공합니다.

- `write_file(vault_id, path, content, parent_commit)`: 텍스트 파일을 만들거나 업데이트합니다.
- `delete_file(vault_id, path, parent_commit)`: 파일을 삭제합니다.

### Optimistic concurrency control

모든 쓰기에는 `parent_commit`이 필요합니다. 이는 클라이언트가 현재 vault head라고 생각하는 commit hash입니다. 클라이언트가 마지막으로 읽은 뒤 vault가 진행되었다면 서버는 `{ "conflict": true, "current_head": "..." }`를 반환하고 쓰지 않습니다. 클라이언트는 다시 읽고, 필요하면 merge한 뒤 새 `parent_commit`으로 retry해야 합니다.

### Rate limit

Write tools는 `(token, vault)` 쌍별로 분당 60 writes로 제한됩니다. Read tools와 SSE subscriptions는 이 write quota의 영향을 받지 않습니다.

### Audit trail

성공한 모든 write 또는 delete는 activity log에 `mcp_write` 또는 `mcp_delete`로 기록되며, details에는 path, commit, size가 포함됩니다. 관리자는 activity page에서 AI-driven changes를 검토할 수 있습니다.

### Caveat: writes enter git history

AI-driven writes는 vault git history의 commits가 됩니다. 일반 git operations로 roll back할 수 있지만, 이미 commit된 변경을 "never have happened"로 만들 방법은 없습니다. 이 audit trail은 의도된 것입니다.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, bridge-based MCP clients는 `pkvsyncd mcp`를 실행해 stdio mode를 사용할 수 있습니다.
- Streamable HTTP를 지원하는 clients는 `/mcp`를 가리키고 모든 요청에 bearer auth와 배포 키를 보낼 수 있습니다.
- 서버는 stateless입니다. `Mcp-Session-Id`를 요구하거나 반환하지 않습니다.
