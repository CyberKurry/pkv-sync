# AI ツール向け MCP アクセス

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | [繁體中文](./mcp-howto.zh-Hant.md) | 日本語 | [한국어](./mcp-howto.ko.md)

ドキュメントバージョン: v1.5.1。

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

PKV Sync は MCP server を通じて vault 内容を公開できます。サーバーはファイル内容を返す前に blob pointers を解決し、明示的な read-write tools を通じて書き込みもでき、通常の PKV Sync bearer device token が必要です。

## Tools

- `list_vaults`: 認証済みユーザーが利用できる vault を一覧表示します。
- `list_files {vault_id, at?}`: HEAD、または `at` が指定された場合はその commit SHA の paths を一覧表示します。
- `read_file {vault_id, path}`: HEAD のファイルを読み取ります。
- `read_file_at_commit {vault_id, path, commit}`: 特定 commit のファイルを読み取ります。
- `search {vault_id, query, at?, limit?}`: テキストファイルに対して大文字小文字を区別しない substring search を実行します。`at` で過去の commit に scope し、`limit` で返される一致数の上限を指定します。
- `link_graph {vault_id, at?, path_prefix?, limit?}`: vault の wikilink と Markdown link graph を返します。応答には、ファイルごとの node、`outlinks`、計算された `inlinks`、orphaned pages、`missing` または `ambiguous` reason を持つ broken links、`truncated` flag が含まれます。
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`: `since_commit` 以降に追加、変更、削除、rename されたファイルを一覧表示します。応答には `from_commit`、現在の `to_commit`、`changes`、`truncated` が含まれます。`since_commit` が HEAD の ancestor でない場合、client が vault を再読み取りできるように `unrelated_commit` を返します。
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit` による optimistic concurrency でテキストファイルを作成または更新します。
- `delete_file {vault_id, path, parent_commit}`: `parent_commit` による optimistic concurrency でファイルを削除します。
- `write_files {vault_id, parent_commit, writes?, deletes?}`: 複数のテキストファイルの作成、更新、削除を 1 つの commit として atomically に実行します。`writes[]` は `{path, content}` objects、`deletes[]` は paths を含みます。
- `move_file {vault_id, parent_commit, from, to}`: テキストファイルを 1 つの commit で移動または rename し、git rename history を保ちます。target path は既存であってはいけません。

現在の `SyncPathFilter` に拒否された paths は、一覧表示、検索、読み取り、link graph や change report への含有の対象になりません。読み取り系ツール（`list_files`、`read_file`、`read_file_at_commit`、`search`）は、vault の allowlist で再び許可された hidden paths を受け入れ、自動生成された `.conflict-*` サイドカーを返すことがあります。`link_graph`、`changes_since`、およびすべての書き込み系ツールはより厳格なポリシーを適用し、hidden paths を一律に拒否するため、agent は hidden files を操作対象にできません。

## stdio transport

コマンドを起動するローカル AI ツールでは stdio を使用します。stdio mode は 1 つの vault に scope されます。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

token を直接渡すこともできます。

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

stdio は両方のプロトコル時代に対応します。モダンな 2026-07-28 クライアントはリクエストごとに `_meta` プロトコルフィールドを送信し、モダンな結果エンベロープを受け取ります。レガシークライアントは `initialize` でネゴシエートし、サーバーは要求されたレガシーバージョンをエコーします。モダンな stdio クライアントはスコープされた vault に対して `subscriptions/listen` を呼び出すこともできます。セッションはクライアントがその listen のリクエスト id を持つ `notifications/cancelled` 通知を送る（または終了する）まで、`notifications/resources/updated` 行を出力し続けます。

## Streamable HTTP transport

クライアントがすでに実行中のローカルまたは内部 MCP endpoint に接続する場合は HTTP を使用します。MCP HTTP は埋め込み専用です。`[mcp].embed_in_serve` はデフォルトで `true` であり、`pkvsyncd serve` がメインサーバーポートに `/mcp` をマウントします。`[mcp].embed_in_serve = false` を設定すると埋め込みエンドポイントを無効化できます。スタンドアロンの `pkvsyncd mcp --transport http --bind` プロセスはこのバージョンで削除されました。既存のスタンドアロンデプロイはメインサーバーの `/mcp` を指すようにしてください。

endpoint path は常に `/mcp` で、メインサーバーの origin 上にあります。

```text
POST http://127.0.0.1:6710/mcp
GET  http://127.0.0.1:6710/mcp
```

すべてのリクエストには次が必要です。

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

デプロイメントキーは主 PKV Sync サーバーと同じ設定ファイルから読み取られます。キーがない、または間違っている場合は bearer token 認証の前に HTTP `404` を返します。

MCP HTTP は固定ウィンドウで 60 秒あたり 120 リクエストに制限されます。制限を超えると、サーバーは HTTP `429` と JSON-RPC error code `-63001` を返します。失敗した MCP bearer token 認証は HTTP `401` と code `-63002` を返し、プロセス内でも制限されます。stdio と HTTP transports の合計で 60 秒あたり最大 30 回の失敗試行までです。

MCP 仕様の `Origin` ヘッダーによる DNS-rebinding チェックは `/mcp` では意図的に実装していません。この endpoint は非アンビエントなデプロイメントキーと bearer device token を必須にしており（ブラウザーはこのどちらも cross-origin リクエストに付与できません）、admin session cookie は拒否され、どの CORS ポリシーもこの endpoint をブラウザーの origin に公開しないためです。これらの前提のいずれかを無効化するデプロイメントは、`/mcp` の前に独自の `Origin` チェックを置くべきです。

## MCP 2026-07-28（モダン）クライアント

同じ endpoint が両方のプロトコル時代に対応します。モダンクライアントは `params._meta` にリクエストごとのプロトコルフィールド（`io.modelcontextprotocol/protocolVersion` と `io.modelcontextprotocol/clientCapabilities`）を送信し、`MCP-Protocol-Version` ヘッダーにバージョンを、`Mcp-Method` ヘッダーにメソッドを繰り返さなければなりません。リクエストが `params.name` を持つ場合は、`Mcp-Name` ヘッダーにも名前を繰り返します（純 ASCII でない場合は `=?base64?...?=` 形式で base64 エンコード）。ヘッダーと body が一致しない場合、HTTP `400` と code `-32020` を返します。サポート対象外のバージョンを要求すると HTTP `400` と code `-32022`、サポートされているバージョンのリスト（`2026-07-28`）を返します。不明なメソッドは HTTP `404` と `-32601` を返し、`id` のない通知のみのメッセージは body なしの HTTP `202` で応答されます。ただし `notifications/cancelled` は例外で、`405` を返します。Streamable HTTP でのキャンセルは、SSE 応答ストリームを閉じることで行われます。

モダンな結果には `resultType` が含まれ、`tools/list` と `server/discover` にはさらに `ttlMs`/`cacheScope` キャッシュヒントが付きます。`server/discover` は `supportedVersions` を報告するため、クライアントは接続前に自分の時代を選択できます。（モダン版のみ。レガシーの `initialize` ネゴシエーションはこのリストの外で扱われます）レガシーの `initialize` クライアントも引き続き利用できます。サーバーは要求されたレガシーバージョンをエコーし、レガシーエンベロープで応答します。

## 変更通知

- **レガシークライアント**: `Accept: text/event-stream` を持つ GET `/mcp` は `vault_changed` notifications を購読します。各 event の id は commit SHA そのもので、再接続時に `Last-Event-ID` として送り返すことで、その vault の missed commits を replay できます。Replay には上限があります。サーバーが missed history をカバーできない場合は `lagged` を送信し、クライアントは sync API から更新する必要があります。`:` を含む値は再開位置として受け付けられません。トークンから複数の vault が見える場合、再開する vault の位置しか分からないため、他の vault に対しても `lagged` を送信します。ライブ配信による `lagged` イベントはイベント data に該当する vault id を格納します。リプレイレベルの `lagged` イベントの data は空です。
- **モダンクライアント**: `subscriptions/listen` リクエストを POST し、`params.notifications.resourceSubscriptions` に `["pkv://vault/<id>"]` URI を設定します（トークンが見える vault のみ受け付けられます）。`resourceSubscriptions` 内の重複した URI は、acknowledgment で（最初の出現を優先して）重複排除されます。応答は SSE ストリームで、まず受け入れられた購読をエコーする `notifications/subscriptions/acknowledged` event を送り、その後 params に `uri`、`commit`、`event`、`_meta.subscriptionId` を持つ `notifications/resources/updated` event を送信します。サーバーのイベントバッファが溢れた場合（ストリームの消費が遅い間に 64 commit を超えるバーストが発生した場合）、listen ストリームは影響を受ける `uri` と同じ `_meta` の subscription id を持つ `notifications/resources/dropped` 通知を送ります。クライアントはその vault の状態を読み直すべきです。ストリームを閉じると購読はキャンセルされます。モダンなストリームは再開できません。`Last-Event-ID` replay はないため、再接続後は再度 listen して読み取った内容を比較します。

信頼できるネットワーク制御の背後に置かない限り、HTTP は loopback に bind してください。bearer token は、そのユーザーが所有するすべての vault への読み書きアクセスを与えます。

## Read and search limits

`search` は最大 5000 個の visible tree files を走査します。vault の visible files が 5000 個を超える場合、一部だけを走査するのではなく `too many files to search` で失敗します。最大 500 matches（既定 100）を返し、production では読み取った text が 256 MiB を超えるとエラーで中止します。`link_graph` は最大 5000 個の visible text files を走査し、同じ production text budget を使用しますが、失敗せず `truncated: true` を返します。`changes_since` は最大 5000 個の visible change entries を返します。`read_file` と `read_file_at_commit` は応答前に blob pointer を解決します。binary 内容は `encoding: "base64"` を伴って base64 で返され、64 MiB を超える binary/blob response は base64 として JSON に展開される代わりに拒否されます。

## Write tools

PKV Sync は読み取り tools と併せて 4 つの MCP write tools を提供します。

- `write_file(vault_id, path, content, parent_commit)`: テキストファイルを作成または更新します。
- `delete_file(vault_id, path, parent_commit)`: ファイルを削除します。
- `write_files(vault_id, parent_commit, writes[], deletes[])`: 複数のテキストファイルを 1 つの commit で atomically に作成、更新、削除します。path が無効、file が `max_file_size` を超える、batch が空 (`empty_batch`)、または batch が 100 changes を超える (`batch_too_large`) 場合、何も commit されません。古い `parent_commit` では通常の `Conflict` response を返します。
- `move_file(vault_id, parent_commit, from, to)`: 1 つのテキストファイルを単一 commit で移動または rename します。既存 target (`target_exists`)、binary/blob-pointer source (`unsupported_binary_move`)、存在しないまたは hidden の source (`not_found`) は拒否します。

### Optimistic concurrency control

すべての書き込みには `parent_commit`、つまりクライアントが現在の vault head だと考える commit hash が必要です。クライアントが最後に読んだ後に vault が進んでいる場合、サーバーは `{ "conflict": true, "current_head": "..." }` を返し、書き込みません。クライアントは再読み取りし、必要なら merge し、新しい `parent_commit` で retry する必要があります。

### Rate limit

Write tools は `(token, vault)` ペアごとに 1 分あたり 60 writes に制限されます。`write_files` は batch 全体で 1 つの rate-limit record だけを消費します。Read tools と SSE subscriptions はこの write quota の影響を受けません。

1.2.1 の強化では、書き込み検証を fail-closed に保ちます。`writes[]` と `deletes[]` で正規化後に重複する path は拒否され、hidden または excluded path は対象の存在を漏らさず、無効な `move_file` の移動元は write quota を消費する前に拒否されます。MCP 認証エラーは汎用メッセージのままで、Streamable HTTP JSON body の上限は 100 MiB です。

### Audit trail

成功した write、batch write、move、delete はすべて、activity log に `mcp_write` または `mcp_delete` として記録され、details には path summary、commit、size が含まれます。管理者は activity page から AI-driven changes を確認できます。

### Caveat: writes enter git history

AI-driven writes は vault git history の commits になります。通常の git operations で roll back できますが、commit 済みの変更を「発生しなかった」ことにはできません。この audit trail は意図的なものです。

## Client notes

- Claude Code、Codex CLI、Cherry Studio、OpenCode、および bridge-based MCP clients は、`pkvsyncd mcp` を起動して stdio mode を使用できます。
- Streamable HTTP をサポートする clients は `/mcp` を指し、すべてのリクエストで bearer auth とデプロイメントキーを送信できます。
- どちらの transport も dual-era です。レガシーの `initialize` クライアントは引き続き動作し、モダンな 2026-07-28 クライアントはリクエストごとのバージョン処理、`server/discover`、`subscriptions/listen` プッシュを利用できます。
- サーバーは stateless です。`Mcp-Session-Id` を要求せず、返しません。
