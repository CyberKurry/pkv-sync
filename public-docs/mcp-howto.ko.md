# AI 도구용 MCP 접근

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | 한국어

문서 버전: v1.2.0.

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

PKV Sync는 MCP server를 통해 vault 내용을 노출할 수 있습니다. 서버는 파일 내용을 반환하기 전에 blob pointers를 해석하고, 명시적인 read-write tools를 통해 쓰기도 할 수 있으며, 일반 PKV Sync bearer device token이 필요합니다.

## Tools

- `list_vaults`: 인증된 사용자가 사용할 수 있는 vault를 나열합니다.
- `list_files {vault_id, at?}`: HEAD 또는 `at`이 지정된 경우 해당 commit SHA의 paths를 나열합니다.
- `read_file {vault_id, path}`: HEAD의 파일을 읽습니다.
- `read_file_at_commit {vault_id, path, commit}`: 특정 commit의 파일을 읽습니다.
- `search {vault_id, query, at?, limit?}`: 텍스트 파일에서 대소문자를 구분하지 않는 substring search를 수행합니다. `at`은 과거 commit으로 범위를 한정하고, `limit`은 반환되는 일치 수의 상한을 지정합니다.
- `link_graph {vault_id, at?, path_prefix?, limit?}`: vault의 wikilink 및 Markdown link graph를 반환합니다. 응답에는 파일별 node와 `outlinks`, 계산된 `inlinks`, orphaned pages, `missing` 또는 `ambiguous` reason이 있는 broken links, 그리고 `truncated` flag가 포함됩니다.
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`: `since_commit` 이후 추가, 수정, 삭제, rename된 파일을 나열합니다. 응답에는 `from_commit`, 현재 `to_commit`, `changes`, `truncated`가 포함됩니다. `since_commit`이 HEAD의 ancestor가 아니면 클라이언트가 vault를 다시 읽을 수 있도록 `unrelated_commit`을 반환합니다.
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit`을 사용한 optimistic concurrency로 텍스트 파일을 만들거나 업데이트합니다.
- `delete_file {vault_id, path, parent_commit}`: `parent_commit`을 사용한 optimistic concurrency로 파일을 삭제합니다.
- `write_files {vault_id, parent_commit, writes?, deletes?}`: 여러 텍스트 파일의 생성, 업데이트, 삭제를 하나의 commit으로 atomically 수행합니다. `writes[]`에는 `{path, content}` objects가 들어가고, `deletes[]`에는 paths가 들어갑니다.
- `move_file {vault_id, parent_commit, from, to}`: 텍스트 파일을 하나의 commit에서 이동하거나 rename하며 git rename history를 보존합니다. target path는 이미 존재하면 안 됩니다.

모든 MCP read tools는 현재 SyncPathFilter를 준수합니다. 기본 hidden-path rules 또는 runtime exclude globs에 의해 거부된 paths는 나열, 검색, 읽기, link graph 포함, 변경 사항 보고 대상에서 제외됩니다.

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

클라이언트가 이미 실행 중인 로컬 또는 내부 MCP endpoint에 연결할 때는 HTTP를 사용합니다. PKV Sync는 두 가지 HTTP 배포 모드를 제공합니다.

- **Embedded**: `config.toml`에서 `[mcp].embed_in_serve = true`를 설정하면 `pkvsyncd serve`가 메인 서버 포트에 `/mcp`를 마운트합니다.
- **Standalone**: 전용 bind address, 격리된 MCP, 독립 scaling이 필요할 때 별도 MCP 프로세스를 실행합니다.

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

endpoint path는 항상 `/mcp`입니다. embedded mode에서는 메인 서버 origin을, standalone mode에서는 전용 bind address를 사용합니다.

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

MCP HTTP는 고정 창 방식으로 60초당 120개 요청으로 제한됩니다. 제한을 초과하면 서버는 HTTP `429`와 JSON-RPC error code `-32029`를 반환합니다. 실패한 MCP bearer token 인증도 프로세스 내에서 제한되며, stdio와 HTTP transports 합산 60초당 최대 30회 실패 시도까지 허용됩니다.

POST는 JSON-RPC tool calls를 담고 JSON responses를 반환합니다. `Accept: text/event-stream`이 있는 GET은 `vault_changed` notifications를 구독합니다. Event ids는 `<vault-id>:<commit-sha>`를 사용하며, 재연결 시 `Last-Event-ID`로 되돌려 보내 missed commits를 replay할 수 있습니다. Replay에는 상한이 있습니다. 서버가 missed history를 커버할 수 없으면 `lagged`를 내보내며, 클라이언트는 sync API에서 새로 고쳐야 합니다.

신뢰할 수 있는 네트워크 제어 뒤에 두지 않는 한 HTTP를 loopback에 bind하세요. bearer token은 해당 사용자가 소유한 모든 vault에 대한 읽기 및 쓰기 접근 권한을 부여합니다.

## Read and search limits

`search`는 최대 5000개 visible tree files를 스캔하고 최대 500 matches를 반환하며, 프로덕션에서는 검색한 text가 256 MiB에 도달하면 중단합니다. `link_graph`는 최대 5000개 visible text files를 스캔하고 동일한 프로덕션 text budget을 사용합니다. `changes_since`는 최대 5000개 visible change entries를 반환합니다. `read_file`과 `read_file_at_commit`은 응답 전에 blob pointer를 해석합니다. 64 MiB를 넘는 binary/blob response는 base64로 JSON에 확장되는 대신 거부됩니다.

## Write tools

PKV Sync는 읽기 tools와 함께 네 개의 MCP write tools를 제공합니다.

- `write_file(vault_id, path, content, parent_commit)`: 텍스트 파일을 만들거나 업데이트합니다.
- `delete_file(vault_id, path, parent_commit)`: 파일을 삭제합니다.
- `write_files(vault_id, parent_commit, writes[], deletes[])`: 여러 텍스트 파일을 하나의 commit에서 atomically 만들고, 업데이트하고, 삭제합니다. path가 유효하지 않거나, 파일이 `max_file_size`를 넘거나, batch가 비어 있거나(`empty_batch`), batch가 100 changes를 넘으면(`batch_too_large`) 아무것도 commit하지 않습니다. 오래된 `parent_commit`은 일반 `Conflict` response를 반환합니다.
- `move_file(vault_id, parent_commit, from, to)`: 하나의 텍스트 파일을 단일 commit에서 이동하거나 rename합니다. 이미 존재하는 target(`target_exists`), binary/blob-pointer source(`unsupported_binary_move`), 없거나 hidden인 source(`not_found`)는 거부합니다.

### Optimistic concurrency control

모든 쓰기에는 `parent_commit`이 필요합니다. 이는 클라이언트가 현재 vault head라고 생각하는 commit hash입니다. 클라이언트가 마지막으로 읽은 뒤 vault가 진행되었다면 서버는 `{ "conflict": true, "current_head": "..." }`를 반환하고 쓰지 않습니다. 클라이언트는 다시 읽고, 필요하면 merge한 뒤 새 `parent_commit`으로 retry해야 합니다.

### Rate limit

Write tools는 `(token, vault)` 쌍별로 분당 60 writes로 제한됩니다. `write_files`는 batch 전체에 대해 rate-limit record 하나만 사용합니다. Read tools와 SSE subscriptions는 이 write quota의 영향을 받지 않습니다.

### Audit trail

성공한 모든 write, batch write, move, delete는 activity log에 `mcp_write` 또는 `mcp_delete`로 기록되며, details에는 path summary, commit, size가 포함됩니다. 관리자는 activity page에서 AI-driven changes를 검토할 수 있습니다.

### Caveat: writes enter git history

AI-driven writes는 vault git history의 commits가 됩니다. 일반 git operations로 roll back할 수 있지만, 이미 commit된 변경을 "never have happened"로 만들 방법은 없습니다. 이 audit trail은 의도된 것입니다.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, bridge-based MCP clients는 `pkvsyncd mcp`를 실행해 stdio mode를 사용할 수 있습니다.
- Streamable HTTP를 지원하는 clients는 `/mcp`를 가리키고 모든 요청에 bearer auth와 배포 키를 보낼 수 있습니다.
- 서버는 stateless입니다. `Mcp-Session-Id`를 요구하거나 반환하지 않습니다.
