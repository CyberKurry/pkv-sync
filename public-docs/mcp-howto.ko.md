# AI 도구용 MCP 접근

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | [日本語](./mcp-howto.ja.md) | 한국어

문서 버전: v1.7.1.

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

PKV Sync는 MCP server를 통해 vault 내용을 노출할 수 있습니다. 서버는 파일 내용을 반환하기 전에 blob pointers를 해석하고, 명시적인 read-write tools를 통해 쓰기도 할 수 있으며, 일반 PKV Sync bearer device token이 필요합니다.

## Tools

- `list_vaults`: 인증된 사용자가 사용할 수 있는 vault를 나열합니다.
- `list_files {vault_id, at?}`: HEAD 또는 `at`이 지정된 경우 해당 commit SHA의 paths를 나열합니다.
- `read_file {vault_id, path}`: HEAD의 파일을 읽습니다. 응답에는 write 가드가 기대하는 hash인 `sha256`이 포함됩니다.
- `read_file_at_commit {vault_id, path, commit}`: 특정 commit의 파일을 읽습니다(마찬가지로 `sha256` 포함).
- `search {vault_id, query, at?, limit?, path_prefix?, frontmatter?, after?}`: 텍스트 파일에서 대소문자를 구분하지 않는 substring search를 수행합니다. `at`은 과거 commit으로 범위를 한정하고, `limit`은 반환되는 일치 수의 상한을 지정합니다(엄격한 상한이므로 한 페이지가 이 값을 넘지 않습니다). `path_prefix`는 파일을 읽기 전에 스캔 범위를 하나의 subtree로 제한합니다. `frontmatter`는 최상위 frontmatter로 필터링합니다(`{"kind":"wiki"}`는 trim된 전체 scalar와 일치하고, `{"sources":true}`는 키가 존재해야 하며, 그 외 값은 `invalid_path`로 거부되고 필터에서 걸러진 파일도 content budget에 계상됩니다). `after`는 불투명 커서입니다. 이전 응답의 `next_after`를 그대로 되돌려 전달하면 페이지들이 겹침이나 누락 없이 결과를 정확히 나눠 가지므로, 건너뛰거나 반복되는 항목이 없습니다. 이 서버가 발급하지 않은 커서는 `invalid_cursor`로 거부됩니다.
- `link_graph {vault_id, at?, path_prefix?, limit?, kind?}`: vault의 wikilink 및 Markdown link graph를 반환합니다. 응답에는 파일별 node와 `outlinks`, 계산된 `inlinks`, orphaned pages, `missing` 또는 `ambiguous` reason이 있는 broken links, 그리고 `truncated` flag가 포함됩니다. `kind`는 최상위 frontmatter의 `kind:`가 일치하는 파일만 남기지만, 링크 대상은 여전히 보이는 모든 텍스트 파일에 대해 해석됩니다. 따라서 wiki 페이지가 source 페이지를 인용해도 broken으로 잘못 보고되지 않으며, orphaned pages는 선택한 층 안에서 계산됩니다.
- `stale_pages {vault_id, path_prefix?, at?, limit?}`: 페이지 자체의 `source_commit` 앵커 이후 source가 변경된 wiki 페이지를 보고합니다. 각 항목에는 `changed_sources`, `removed_sources`, `renamed`(`from`/`to`), `unresolved_sources`, `last_reconciled`가 나열됩니다. 이 비교는 git tree diff이므로 "근거가 움직였다"는 점을 알려줄 뿐이며, "이 주장이 틀렸다"는 판단은 하지 않습니다. 그 판단은 agent의 몫입니다. 세 가지 정직 규칙이 있습니다. 서버가 확인할 수 없는 source(hidden, excluded 또는 존재하지 않음)는 누락시키지 않고 `unresolved_sources`에 보고합니다. `sources:`를 선언했지만 `source_commit`이 없는 페이지는 추측과 비교하는 대신 `missing_source_commit`에 나열합니다. 그리고 diff가 목록 상한에서 잘린 페이지는 나열된 항목이 없더라도 `verified: false`로 반환되므로, 불완전한 스캔이 깨끗한 결과로 읽히지 않습니다. LLM Wiki workflow 문서에 설명된 `sources:`/`source_commit:` frontmatter 규칙이 필요합니다.
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`: `since_commit` 이후 추가, 수정, 삭제, rename된 파일을 나열합니다. 응답에는 `from_commit`, 현재 `to_commit`, `changes`, `truncated`가 포함됩니다. `since_commit`이 HEAD의 ancestor가 아니면 클라이언트가 vault를 다시 읽을 수 있도록 `unrelated_commit`을 반환합니다.
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit`을 사용한 optimistic concurrency로 텍스트 파일을 만들거나 업데이트합니다.
- `delete_file {vault_id, path, parent_commit}`: `parent_commit`을 사용한 optimistic concurrency로 파일을 삭제합니다.
- `write_files {vault_id, parent_commit, writes?, deletes?}`: 여러 텍스트 파일의 생성, 업데이트, 삭제를 하나의 commit으로 atomically 수행합니다. `writes[]`에는 `{path, content}` objects가 들어가고, `deletes[]`에는 paths가 들어갑니다.
- `move_file {vault_id, parent_commit, from, to}`: 텍스트 파일을 하나의 commit에서 이동하거나 rename하며 git rename history를 보존합니다. target path는 이미 존재하면 안 됩니다.

현재 `SyncPathFilter`에서 거부된 paths는 나열, 검색, 읽기, link graph 및 변경 보고 대상에서 제외됩니다. 읽기 도구(`list_files`, `read_file`, `read_file_at_commit`, `search`)는 vault allowlist로 다시 허용된 hidden paths를 받아들이며, 자동 생성된 `.conflict-*` 사이드카를 반환할 수 있습니다. `link_graph`, `changes_since` 및 모든 쓰기 도구는 더 엄격한 정책을 적용해 hidden paths를 일괄 거부하므로, agent가 hidden files를 작업 대상으로 삼을 수 없습니다.

## stdio transport

명령을 실행하는 로컬 AI 도구에는 stdio를 사용합니다. stdio mode는 하나의 vault로 scope됩니다.

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

token을 직접 전달할 수도 있습니다.

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

stdio는 두 프로토콜 시대를 모두 지원합니다. 모던 2026-07-28 클라이언트는 요청마다 `_meta` 프로토콜 필드를 보내고 모던 결과 봉투를 받습니다. 레거시 클라이언트는 `initialize`로 협상하며, 서버는 알고 있는 요청 버전(`2026-07-28` 포함)을 그대로 되돌려주고, 알 수 없는 버전이면 최신 레거시 버전(`2025-11-25`)으로 응답합니다. 모던 stdio 클라이언트는 스코프된 vault에 대해 `subscriptions/listen`을 호출할 수도 있습니다. 세션은 클라이언트가 해당 listen의 요청 id를 담은 `notifications/cancelled` 알림을 보내거나(또는 종료할 때까지) `notifications/resources/updated` 줄을 계속 출력합니다. 세션의 Token을 약 30초 동안 재검증할 수 없으면(예: 서버 데이터베이스에 연결할 수 없는 경우) 세션은 이를 폐기로 간주하여 활성 listen마다 `notifications/resources/dropped`를 한 줄 출력한 뒤 push를 중단합니다.

## Streamable HTTP transport

클라이언트가 이미 실행 중인 로컬 또는 내부 MCP endpoint에 연결할 때는 HTTP를 사용합니다. MCP HTTP는 내장 방식만 제공됩니다. `[mcp].embed_in_serve`는 기본적으로 `true`이며, `pkvsyncd serve`가 메인 서버 포트에 `/mcp`를 마운트합니다. `[mcp].embed_in_serve = false`를 설정하면 내장 endpoint를 비활성화할 수 있습니다. 독립 실행형 `pkvsyncd mcp --transport http --bind` 프로세스는 이 버전에서 제거되었습니다. 기존 독립 배포는 메인 서버의 `/mcp`를 가리키도록 변경하세요.

endpoint path는 항상 `/mcp`이며 메인 서버 origin에 있습니다.

```text
POST http://127.0.0.1:6710/mcp
GET  http://127.0.0.1:6710/mcp
```

모든 요청에는 다음이 포함되어야 합니다.

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

배포 키는 주 PKV Sync 서버와 같은 설정 파일에서 읽습니다. 키가 없거나 잘못되면 bearer token 인증 전에 HTTP `404`를 반환합니다.

MCP HTTP는 고정 창 방식으로 60초당 120개 요청으로 제한됩니다. 제한을 초과하면 서버는 HTTP `429`와 JSON-RPC error code `-63001`을 반환합니다. 실패한 MCP bearer token 인증은 HTTP `401`과 code `-63002`를 반환하며, 프로세스 내에서도 transport 인스턴스별로 제한됩니다. 내장 HTTP endpoint는 서버 프로세스의 limiter를 공유하고, 각 `pkvsyncd mcp` 프로세스는 자체 상태와 limiter를 만듭니다. 60초당 최대 30회 실패 시도까지 허용됩니다.

MCP 사양의 `Origin` 헤더 DNS-rebinding 검사는 `/mcp`에서 의도적으로 구현하지 않았습니다. 이 endpoint는 비앰비언트(non-ambient) 배포 키와 bearer device token을 필수로 요구하고(브라우저는 이 둘을 cross-origin 요청에 첨부할 수 없음), admin session cookie는 거부되며, 어떤 CORS 정책도 이 endpoint를 브라우저 origin에 노출하지 않기 때문입니다. 이 전제 중 하나라도 비활성화하는 배포는 `/mcp` 앞에 자체 `Origin` 검사를 두어야 합니다.

## MCP 2026-07-28(모던) 클라이언트

같은 endpoint가 두 프로토콜 시대를 모두 서비스합니다. 모던 클라이언트는 `params._meta`에 요청별 프로토콜 필드(`io.modelcontextprotocol/protocolVersion`과 `io.modelcontextprotocol/clientCapabilities`)를 담아 보내고, `MCP-Protocol-Version` 헤더에 버전을, `Mcp-Method` 헤더에 메서드를 반드시 반복해야 합니다. 요청에 `params.name`이 있으면 `Mcp-Name` 헤더에도 이름을 반복합니다(순수 ASCII가 아니면 `=?base64?...?=` 형식으로 base64 인코딩). 헤더와 body가 일치하지 않으면 HTTP `400`과 code `-32020`을 반환합니다. 지원되지 않는 버전을 요청하면 HTTP `400`과 code `-32022`, 지원 버전 목록(`2026-07-28`)을 반환합니다. 모던 요청의 잘못된 params(예: `_meta` 형식 오류, tool `arguments`가 object가 아닌 경우)도 HTTP `400`과 code `-32602`를 반환합니다. 알 수 없는 메서드는 HTTP `404`와 `-32601`을 반환하며, `id`가 없는 알림 전용 메시지는 body 없이 HTTP `202`로 응답됩니다. 단, `notifications/cancelled`는 예외로 `405`를 반환합니다. Streamable HTTP에서의 취소는 SSE 응답 스트림을 닫는 방식으로 수행됩니다. JSON-RPC batch는 어느 transport에서도 지원되지 않습니다. `POST /mcp`의 배열 body는 `200` JSON-RPC envelope 안에서 code `-32600`(`batch requests are not supported`)을 반환하며, 이는 stdio가 batch 줄에 돌려주는 오류와 같습니다. 수락한 척하며 `202`를 반환하지 않습니다. envelope은 항상 레거시입니다. 배열은 `params._meta`를 담을 수 없으므로 모던 요청으로 분류되지 않습니다.

모던 결과에는 `resultType`이 포함되고, `tools/list`와 `server/discover`에는 추가로 `ttlMs`/`cacheScope` 캐시 힌트가 붙습니다. `server/discover`는 `supportedVersions`를 알려주므로 클라이언트는 연결 전에 자신의 시대를 선택할 수 있습니다.(모던 버전만 해당. 레거시 `initialize` 협상은 이 목록 밖에서 이루어집니다) 레거시 `initialize` 클라이언트도 계속 사용할 수 있습니다. 서버는 요청된 레거시 버전을 되돌려주고, 모던 버전 `2026-07-28`이 요청되면 그대로 되돌려주며, 알 수 없는 버전은 최신 레거시 버전(`2025-11-25`)으로 폴백해 레거시 봉투로 응답합니다.

## 변경 알림

- **레거시 클라이언트**: `Accept: text/event-stream`을 가진 GET `/mcp`는 `vault_changed` notifications를 구독합니다. 각 event의 id는 commit SHA 자체이며, 재연결 시 `Last-Event-ID`로 되돌려 보내 해당 vault의 missed commits를 replay할 수 있습니다. Replay에는 상한이 있습니다. 서버가 missed history를 커버할 수 없으면 `lagged`를 내보내며, 클라이언트는 sync API에서 새로 고쳐야 합니다. 서버가 확인할 수 없는 재개 위치(기록을 읽을 수 없거나 복구할 수 없는 경우)도 같은 방식으로 처리하며, 조용히 라이브 스트림으로 전환하지 않습니다. `:`를 포함한 값은 재개 위치로 허용되지 않습니다. Token이 여러 vault를 볼 수 있으면 재개하는 vault의 위치만 알 수 있으므로, 나머지 vault에 대해서도 `lagged`를 내보냅니다. 라이브 브로드캐스트로 인한 `lagged` 이벤트는 이벤트 data에 해당 vault id를 담고 있으며, 리플레이 수준의 `lagged` 이벤트 data는 비어 있습니다.
- **모던 클라이언트**: `subscriptions/listen` 요청을 POST하고 `params.notifications.resourceSubscriptions`를 `["pkv://vault/<id>"]` URI로 설정합니다(Token이 볼 수 있는 vault만 수락됩니다). `resourceSubscriptions`에서 중복된 URI는 acknowledgment에서(첫 번째 등장이 우선하여) 중복 제거됩니다. 응답은 SSE 스트림으로, 먼저 수락된 구독을 되돌려주는 `notifications/subscriptions/acknowledged` event를 보내고, 이어서 params에 `uri`, `commit`, `event`, `_meta.subscriptionId`를 담은 `notifications/resources/updated` event를 보냅니다. 서버의 이벤트 버퍼가 넘치면(스트림 소비가 느린 동안 64 commit을 초과하는 버스트가 도착하면) listen 스트림은 영향을 받은 `uri`와 동일한 `_meta` 구독 id를 담은 `notifications/resources/dropped` 알림을 보냅니다. 클라이언트는 해당 vault의 상태를 다시 읽어야 합니다. 스트림을 닫으면 구독이 취소됩니다. 모던 스트림은 재개할 수 없습니다. `Last-Event-ID` replay가 없으므로 재연결 후에는 다시 listen하고 읽은 내용을 비교해야 합니다.

신뢰할 수 있는 네트워크 제어 뒤에 두지 않는 한 HTTP를 loopback에 bind하세요. bearer token은 해당 사용자가 소유한 모든 vault에 대한 읽기 및 쓰기 접근 권한을 부여합니다.

## Read and search limits

`search`는 최대 5000개 visible tree files를 스캔합니다. vault의 visible files가 5000개를 넘으면 일부만 스캔하는 대신 `too many files to search`로 실패합니다. 최대 500 matches(기본 100)를 반환하며, 프로덕션에서는 읽은 text가 256 MiB를 초과하면 오류로 중단합니다. `link_graph`는 최대 5000개 visible text files를 스캔하고 동일한 프로덕션 text budget을 사용하지만, 실패하지 않고 `truncated: true`를 반환합니다. link 추출은 파일당 10000개, broken link는 총 20000개로 제한되며, 어느 한도에 도달하면 `truncated: true`가 설정됩니다. `changes_since`는 최대 5000개 visible change entries를 반환합니다. `read_file`과 `read_file_at_commit`은 응답 전에 blob pointer를 해석합니다. binary 내용은 `encoding: "base64"`와 함께 base64로 반환되며, 64 MiB를 넘는 binary/blob response는 base64로 JSON에 확장되는 대신 거부됩니다.

## Write tools

PKV Sync는 읽기 tools와 함께 네 개의 MCP write tools를 제공합니다.

- `write_file(vault_id, path, content, parent_commit, dry_run?, expected_hashes?)`: 텍스트 파일을 만들거나 업데이트합니다.
- `delete_file(vault_id, path, parent_commit, dry_run?, expected_hashes?)`: 파일을 삭제합니다.
- `write_files(vault_id, parent_commit, writes[], deletes[], dry_run?, expected_hashes?)`: 여러 텍스트 파일을 하나의 commit에서 atomically 만들고, 업데이트하고, 삭제합니다. path가 유효하지 않거나, 파일이 `max_file_size`를 넘거나, batch가 비어 있거나(`empty_batch`), batch가 100 changes를 넘으면(`batch_too_large`) 아무것도 commit하지 않습니다. 오래된 `parent_commit`은 일반 `Conflict` response를 반환합니다.
- `move_file(vault_id, parent_commit, from, to, dry_run?, expected_hashes?)`: 하나의 텍스트 파일을 단일 commit에서 이동하거나 rename합니다. 이미 존재하는 target(`target_exists`), binary/blob-pointer source(`unsupported_binary_move`), 없거나 hidden인 source(`not_found`)는 거부합니다.

### Dry runs and content guards

두 가지 선택 인자는 agent의 쓰기를 실행 전에 검토할 수 있게 하고, 동시에 이루어지는 사람의 편집으로부터 안전하게 만듭니다.

`dry_run: true`는 실제 호출이 검사할 모든 것(`parent_commit` 최신 여부, path 가시성, 크기 및 batch 상한, 아래의 가드)을 검증하고, commit하거나 활동을 기록하거나 write quota를 소비하지 않은 채 `{"dry_run": true, "would_write": [{"path", "change", "old_path?", "size_bytes"}], "parent_commit"}`를 반환합니다. move는 `rename` 항목 하나로 보고됩니다. 다만 이는 약속이 아닙니다. commit 경로 아래에 있는 검사(file-kind 및 exclude-glob 적용)는 실행되지 않으므로, dry run은 통과하지만 실제 쓰기는 거부될 수 있습니다.

`expected_hashes: {"<path>": "<sha256>"}`는 호출 최상위의 map으로 개별 파일을 지키는 가드입니다. 값은 agent가 파일을 읽을 때 `read_file`이 반환한 hash입니다. 가드 대상 path 중 하나라도 더 이상 일치하지 않으면 호출 전체가 `content_diverged: path=<p> expected=<sha> actual=<sha|"null">`와 함께 거부되며(바이너리 대상은 받은 적 없는 바이트를 hash할 수 없으므로 즉시 거부됩니다) 아무것도 commit되지 않습니다. 이 map은 추가 방식이므로 기존 클라이언트의 `deletes[]` 형태는 그대로입니다. `expected_hashes`는 `parent_commit` 뒤에 검사됩니다. 부모가 오래된 경우에는 클라이언트가 어차피 다시 읽어야 하므로 기존대로 `Conflict`가 우선합니다.

### Optimistic concurrency control

모든 쓰기에는 `parent_commit`이 필요합니다. 이는 클라이언트가 현재 vault head라고 생각하는 commit hash입니다. 클라이언트가 마지막으로 읽은 뒤 vault가 진행되었다면 서버는 `{ "conflict": true, "current_head": "..." }`를 반환하고 쓰지 않습니다. 클라이언트는 다시 읽고, 필요하면 merge한 뒤 새 `parent_commit`으로 retry해야 합니다.

### Rate limit

Write tools는 `(token, vault)` 쌍별로 분당 60 writes로 제한됩니다. `write_files`는 batch 전체에 대해 rate-limit record 하나만 사용합니다. Read tools와 SSE subscriptions는 이 write quota의 영향을 받지 않습니다.

1.2.1 강화는 쓰기 검증을 fail-closed로 유지합니다. `writes[]`와 `deletes[]`에서 정규화 후 중복되는 path는 거부되고, hidden 또는 excluded paths는 대상 존재를 노출하지 않으며, 잘못된 `move_file` 원본은 write quota를 소비하기 전에 거부됩니다. MCP 인증 오류는 일반 메시지로 유지되고 Streamable HTTP JSON body 상한은 100 MiB입니다.

### Audit trail

성공한 모든 write, batch write, move, delete는 activity log에 `mcp_write` 또는 `mcp_delete`로 기록되며, details에는 변경을 수행한 도구, path summary, commit, size가 포함됩니다. 관리자 activity page는 장치(agent) 이름 옆에 도구를 표시하고 `?agent=<device name>` 및 `?tool=<tool name>`으로 필터링할 수 있으므로, "어느 agent가 이것을 삭제했는가"와 "이 agent가 무엇을 해 왔는가"에 모두 답할 수 있습니다. dry run은 아무 일도 일어나지 않았으므로 아무것도 기록하지 않습니다.

### Caveat: writes enter git history

AI-driven writes는 vault git history의 commits가 됩니다. 일반 git operations로 roll back할 수 있지만, 이미 commit된 변경을 "never have happened"로 만들 방법은 없습니다. 이 audit trail은 의도된 것입니다.

## Registry

이 저장소는 그대로 공개할 수 있는 [MCP Registry](https://registry.modelcontextprotocol.io) 메타데이터(`mcp/server.json`)를 `io.github.cyberkurry/pkv-sync`로 함께 제공합니다. 이 항목은 게시된 컨테이너 이미지를 가리키며 `PKV_TOKEN`과 `PKV_VAULT_ID`로 세션을 구성합니다. 자체 호스팅하는 서버는 위 명령으로 직접 연결할 수도 있습니다.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, bridge-based MCP clients는 `pkvsyncd mcp`를 실행해 stdio mode를 사용할 수 있습니다.
- Streamable HTTP를 지원하는 clients는 `/mcp`를 가리키고 모든 요청에 bearer auth와 배포 키를 보낼 수 있습니다.
- 두 transport 모두 dual-era입니다. 레거시 `initialize` 클라이언트는 계속 동작하고, 모던 2026-07-28 클라이언트는 요청별 버전 처리, `server/discover`, `subscriptions/listen` 푸시를 사용할 수 있습니다.
- 서버는 stateless입니다. `Mcp-Session-Id`를 요구하거나 반환하지 않습니다.
