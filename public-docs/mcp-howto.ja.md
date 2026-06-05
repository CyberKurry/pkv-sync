# AI ツール向け MCP アクセス

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | 日本語 | [한국어](./mcp-howto.ko.md)

ドキュメントバージョン: v1.0.13。

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

PKV Sync は MCP server を通じて vault 内容を公開できます。サーバーはファイル内容を返す前に blob pointers を解決し、明示的な read-write tools を通じて書き込みもでき、通常の PKV Sync bearer device token が必要です。

## Tools

- `list_vaults`: 認証済みユーザーが利用できる vault を一覧表示します。
- `list_files {vault_id, at?}`: HEAD、または `at` が指定された場合はその commit SHA の paths を一覧表示します。
- `read_file {vault_id, path}`: HEAD のファイルを読み取ります。
- `read_file_at_commit {vault_id, path, commit}`: 特定 commit のファイルを読み取ります。
- `search {vault_id, query, at?, limit?}`: テキストファイルに対して大文字小文字を区別しない substring search を実行します。`at` で過去の commit に scope し、`limit` で返される一致数の上限を指定します。
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit` による optimistic concurrency でテキストファイルを作成または更新します。
- `delete_file {vault_id, path, parent_commit}`: `parent_commit` による optimistic concurrency でファイルを削除します。

## stdio transport

コマンドを起動するローカル AI ツールでは stdio を使用します。stdio mode は 1 つの vault に scope されます。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

token を直接渡すこともできます。

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

クライアントがすでに実行中のローカルまたは内部 MCP endpoint に接続する場合は HTTP を使用します。PKV Sync には 2 つの HTTP デプロイモードがあります。

- **Embedded**: `config.toml` で `[mcp].embed_in_serve = true` を設定すると、`pkvsyncd serve` がメインサーバーポートに `/mcp` をマウントします。
- **Standalone**: 専用 bind address、隔離された MCP、独立 scaling が必要な場合は、別 MCP プロセスを実行します。

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

endpoint path は常に `/mcp` です。embedded mode ではメインサーバー origin、standalone mode では専用 bind address を使います。

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

すべてのリクエストには次が必要です。

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

デプロイメントキーは主 PKV Sync サーバーと同じ設定ファイルから読み取られます。キーがない、または間違っている場合は bearer token 認証の前に HTTP `404` を返します。

MCP HTTP は固定ウィンドウで 60 秒あたり 120 リクエストに制限されます。制限を超えると、サーバーは HTTP `429` と JSON-RPC error code `-32029` を返します。失敗した MCP bearer token 認証もプロセス内で制限され、stdio と HTTP transports の合計で 60 秒あたり最大 30 回の失敗試行までです。

POST は JSON-RPC tool calls を運び、JSON responses を返します。`Accept: text/event-stream` を持つ GET は `vault_changed` notifications を購読します。Event ids は `<vault-id>:<commit-sha>` を使用し、再接続時に `Last-Event-ID` として送り返すことで missed commits を replay できます。Replay には上限があります。サーバーが missed history をカバーできない場合は `lagged` を送信し、クライアントは sync API から更新する必要があります。

信頼できるネットワーク制御の背後に置かない限り、HTTP は loopback に bind してください。bearer token は、そのユーザーが所有するすべての vault への読み書きアクセスを与えます。

## Read and search limits

`search` は最大 5000 個の tree files を走査し、最大 500 matches を返し、production では検索済み text が 256 MiB に達すると停止します。`read_file` と `read_file_at_commit` は応答前に blob pointer を解決します。64 MiB を超える binary/blob response は、base64 として JSON に展開される代わりに拒否されます。

## Write tools

PKV Sync は読み取り tools と併せて 2 つの MCP write tools を提供します。

- `write_file(vault_id, path, content, parent_commit)`: テキストファイルを作成または更新します。
- `delete_file(vault_id, path, parent_commit)`: ファイルを削除します。

### Optimistic concurrency control

すべての書き込みには `parent_commit`、つまりクライアントが現在の vault head だと考える commit hash が必要です。クライアントが最後に読んだ後に vault が進んでいる場合、サーバーは `{ "conflict": true, "current_head": "..." }` を返し、書き込みません。クライアントは再読み取りし、必要なら merge し、新しい `parent_commit` で retry する必要があります。

### Rate limit

Write tools は `(token, vault)` ペアごとに 1 分あたり 60 writes に制限されます。Read tools と SSE subscriptions はこの write quota の影響を受けません。

### Audit trail

成功した write または delete はすべて、activity log に `mcp_write` または `mcp_delete` として記録され、details には path、commit、size が含まれます。管理者は activity page から AI-driven changes を確認できます。

### Caveat: writes enter git history

AI-driven writes は vault git history の commits になります。通常の git operations で roll back できますが、commit 済みの変更を「発生しなかった」ことにはできません。この audit trail は意図的なものです。

## Client notes

- Claude Code、Codex CLI、Cherry Studio、OpenCode、および bridge-based MCP clients は、`pkvsyncd mcp` を起動して stdio mode を使用できます。
- Streamable HTTP をサポートする clients は `/mcp` を指し、すべてのリクエストで bearer auth とデプロイメントキーを送信できます。
- サーバーは stateless です。`Mcp-Session-Id` を要求せず、返しません。
