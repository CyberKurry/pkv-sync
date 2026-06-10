# PKV Sync 管理者マニュアル

[English](./admin-manual.md) | [简体中文](./admin-manual.zh-CN.md) | [繁體中文](./admin-manual.zh-Hant.md) | 日本語 | [한국어](./admin-manual.ko.md)

ドキュメントバージョン: v1.2.1。

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

このマニュアルでは、セルフホストした PKV Sync サーバーの日常管理を扱います。ネットワークとホストの強化については、deployment hardening guide も併せて読んでください。

## 初回実行

1. デプロイメントキーを生成します。

   ```bash
   pkvsyncd genkey
   ```

2. `config.example.toml` から `/etc/pkv-sync/config.toml` を作成します。
3. 新しい 1.x データディレクトリ向けに v1 データベース baseline を初期化します。

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. サーバーを起動します。

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 新規データベースの初回起動後、ブラウザーで `/setup` を開き、最初の管理者アカウントを作成します。PKV Sync はランダムな管理者パスワードを stderr やコンテナログへ出力しません。
6. setup 完了後、通常の管理者サインインには `/admin/login` を使用します。

PKV Sync 1.0 は単一の v1 SQLite baseline を使用します。0.x で作成されたデータベースは 1.0.0 へインプレース upgrade できません。[`upgrade-notes-v1.0.ja.md`](./upgrade-notes-v1.0.ja.md) の手順に従ってください。この v1 baseline 以後、公開済みの 1.x migrations は append-only です。

## Admin Web パネル

開く場所:

```text
https://sync.example.com/admin/login
```

Web パネルには次が含まれます。

- システム、ストレージ、vault、ユーザー、最近のアクティビティ指標を表示するダッシュボード
- 検索とステータスフィルター付きのユーザー一覧
- パスワードリセット、有効/管理者制御、token 確認のためのユーザー詳細ページ
- token の一覧表示、作成、取り消しを行うグローバル装置 token ページ
- 所有者、ファイル数、サイズ、最終同期、reconcile、削除操作、vault ごとの同期設定を持つ vault カード
- ファイルプレビュー、ファイル別履歴タイムライン、unified diff レンダリング付きの読み取り専用 vault ファイルブラウザー
- 任意の有効期限付き招待の作成、アクティブな招待一覧、未使用招待の削除
- General、Security、Sync & Storage、Network に分かれた実行時設定。更新確認の有効化と間隔も含まれます
- 同期、vault ライフサイクル、読み取り専用閲覧行を対象に、実ユーザーとアクションでフィルターできるアクティビティログ
- Blob ガベージコレクションのトリガー
- 英語、簡体字中国語、繁体字中国語、日本語、韓国語の言語切替

1.2.1 では、ユーザー詳細統計は実際の vault 数と最終同期時刻に基づき、期間ラベルは同梱されるすべての admin 言語にローカライズされています。reconciliation とメタデータ修復処理も、利用できる場合は増分処理または batch 処理を使います。

タイムスタンプ、期間、バイトサイズ、稼働時間、アクティビティデータは人間が読みやすい形式で表示されます。既定のタイムゾーンは `Asia/Shanghai` で、設定から変更できます。

## 更新通知

PKV Sync は既定で 24 時間ごとに GitHub release を確認します。新しいサーバー版がある場合、ダッシュボードに現在のバージョン、最新バージョン、release notes リンク、短い抜粋を含むバナーが表示されます。

`config.toml` の `[update_check].enabled` と `[update_check].interval_seconds` は、新しいデータベースの初回起動時にランタイム設定へ seed されます。その後は Admin WebUI の Settings ページが優先されます。**Network** セクションで更新確認を切り替えたり間隔を変更したりでき、バックグラウンドタスクは次のサイクルで新しいランタイム値を読み直します。現在無効な場合、再有効化は約 60 秒以内に反映されます。`[update_check].repo` は、エアギャップ mirror デプロイメント用の静的な `config.toml` フィールドのままです。

```toml
[update_check]
enabled = false
interval_seconds = 86400
repo = "cyberkurry/pkv-sync"
```

更新確認は情報提供のみです。PKV Sync は実行中のサーバーバイナリやコンテナイメージを自動的に置き換えません。

## ユーザー管理

- **Users** または CLI からユーザーを作成します。
- ユーザー名は 3-32 文字の ASCII 英字、数字、`_`、`-`、`.` のいずれかである必要があります。
- 管理者による作成/リセット、公開登録、ユーザー自身のパスワード変更はすべて 12 文字以上で、大文字、小文字、数字を含む必要があります。
- Users ページの検索とステータスフィルターで表を絞り込めます。
- ユーザー詳細ページを開くと、パスワードリセット、アカウントの有効化/無効化、管理者権限の付与/解除、そのユーザーの装置 token の確認ができます。
- 監査履歴が必要になる可能性がある場合は、削除ではなく無効化を優先してください。
- Admin WebUI はユーザーの無効化や管理者の降格前に確認ダイアログを表示します。自分自身の管理者セッションの無効化と最後の管理者の降格は拒否され、ユーザー詳細ページにローカライズされたフィードバックが表示されます。
- 残っているすべての管理者アカウントを無効化しないでください。

Admin WebUI からパスワードをリセットすると、そのユーザーの既存の装置 token は取り消されます。ユーザーは再ログインする必要があります。

CLI のフォールバック:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 装置 Token

装置 bearer token は認証済みの使用時に更新され、90 日間使用されないと期限切れになり、各 token には 365 日の絶対有効期限があります。ユーザーは自分の token を取り消せ、管理者は任意のユーザーの token を取り消せます。

運用上の注意:

- Token の平文は作成時に一度だけ表示されます。
- データベースには SHA-256 token hash のみ保存されます。
- 管理者向け token 一覧 endpoint と表は公開 token メタデータだけを表示し、平文 token や内部の期限/取り消しフィールドは返しません。
- 各認証済みリクエストは、そのリクエスト時刻から 90 日後まで token 期限を延長しますが、token 作成から 365 日を超えません。
- 同じ安定したプラグイン装置 ID から再ログインすると、その装置の以前のアクティブ token が置き換えられます。
- アクティビティ行から参照される取り消し済み token は、アクティビティ履歴を残したままクリーンアップできます。

## Vault

Admin WebUI から vault を削除するには追加の確認ダイアログが必要です。未参照の blob がガベージコレクションまで残る場合でも、削除は破壊的操作として扱ってください。

vault を削除すると次が削除されます。

- vault データベース行
- そこから cascade される関連メタデータ行
- `data_dir/vaults/<vault-id>` 配下のバックエンド bare Git リポジトリ
- メモリ内の vault ごとの push ロック

Blob ファイルは内容アドレス指定であり、ガベージコレクションが猶予期間を超えて未参照であることを確認するまで残る場合があります。

中断された操作の後にファイル数、サイズ、blob 参照が正しく見えない場合は、vault メタデータ reconciliation を使用してください。reconciliation は tree entry から blob pointer hash を直接読み取り、blob 参照の修復を batch 化するため、pointer ファイルを 1 つずつ開き直す必要がありません。

### Vault ごとの同期設定

**Vaults** から vault カードの **Settings** を開き、vault ごとの `extra_sync_globs` allowlist を編集します。これは、選択した `.obsidian` 設定ファイルを含む隠しパスのうち、どれを同期できるかを制御します。

新しい vault には推奨 starter allowlist が自動的に設定されます。既存の vault は、管理者または vault 所有者が starter list を適用するまで空のままです。**Apply starter allowlist** は、テーマ、CSS snippets、ホットキー、アプリ設定、外観設定、有効化済みプラグイン一覧向けの推奨リストを書き込みます。

### 読み取り専用ファイル履歴

**Vaults** から vault カードの **Browse files** を開きます。ブラウザーは現在の HEAD ファイルを、サイズとテキスト/バイナリ種別付きで一覧表示します。ファイルを開くと、テキストファイルは読み取り専用プレビューを表示し、**History** と **Diff with previous** へのリンクを提供します。

履歴ページはそのファイルの commit を一覧表示し、各 commit 時点のファイルと対応する diff へのリンクを提供します。diff ページは unified diff の行を追加/削除/hunk の色分け付きでレンダリングします。バイナリファイルはメタデータのみ表示し、バイナリ diff 内容はレンダリングしません。現在の同期フィルターで拒否されたパスは、ファイルブラウズ、commit list、history、diff の画面から隠されます。

ファイル、履歴、diff の閲覧は `view_commit`、`view_history`、`view_diff` のアクティビティ行を記録します。Vault rollback controls は Admin history から利用できます。対象 commit を確認してから使用してください。rollback は選択した履歴時点から新しい vault 状態を作成します。

## 招待と登録

**Settings** から登録を設定します。

- `disabled`: 管理者だけがアカウントを作成します
- `invite_only`: ユーザーは招待コードで登録します
- `open`: デプロイメント URL を持つ誰でも登録できます

招待作成では任意の将来の有効期限を指定できます。Admin WebUI は人間向けの日付時刻入力を使い、内部では Unix 秒を保存します。使用済み招待は admin API から削除できません。監査履歴として保持してください。

`open` は、短時間のウィンドウまたは追加の監視とレート制限がある公開デプロイメントでのみ使用してください。

## 実行時設定

Settings ページは SQLite に保存された値を編集します。変更は新しいリクエストに即時反映され、保存時にメモリ内キャッシュが更新されます。

**General** — サーバー名、既定タイムゾーン、`enable_metrics` メトリクススイッチ。有効化すると `/metrics` が利用できますが、引き続きデプロイメントキー middleware、プラグイン User-Agent guard、管理者 bearer token が必要です。

**Security** — 登録モード（`disabled` / `invite_only` / `open`）、ログイン失敗しきい値、失敗ウィンドウ、ロック時間。ログインレートリミッターは失敗回数と進行中のパスワード検証の両方を数えるため、同時大量推測でしきい値を回避できません。認証済み同期 API ルートには、ルート、メソッド、クライアント IP、bearer 装置 token ごとに 60 秒あたり最大 600 リクエストの固定ウィンドウ制限があります。失敗した bearer token 認証試行もクライアント IP ごとに 60 秒あたり最大 120 回に制限されるため、偽 token をローテーションしても失敗予算を回避できません。

**Sync & Storage**
- 最大ファイルサイズ（既定 `100 MiB`）。Blob upload request body は、この runtime 設定をより高くしても、常に hard storage cap（production では `512 MiB`）で制限されます
- 対応テキスト拡張子 — 一覧外のファイルはバイナリ blob として扱われます。この一覧は Admin WebUI では読み取り専用で表示されます。変更が必要な場合は、`text_extensions` runtime config 行を編集するか、SQLite の `runtime_config` テーブルを直接編集してください。
- 追加 exclude glob — 組み込みの `.obsidian/`、`.trash/`、`.conflict-*`、`.git/` 除外リストを補う管理者調整可能なパターン
- 履歴 UI と diff エンドポイントの切替
- **Auto-merge text**（`enable_auto_merge`、既定オン）: 有効時、サーバーは衝突ファイルを書き出す前に 3-way ライン merge を試みます。重ならない編集はクリーンに merge され、重なる編集は引き続き merge マーカー入りの衝突ファイルになります。
- **Push debounce**（`push_debounce_ms`、既定 `250`）: ローカル編集が落ち着いてから push するまでの待機時間。小さくするとエンドツーエンド遅延が減り、大きくすると 1 回の push でより多くの入力をまとめられます
- **Inline SSE content cap**（`inline_content_max_bytes`、既定 `8192`、最大 `65536`）: このサイズまでのテキスト変更は SSE イベント内で送信され、受信プラグインは別途 pull せずに適用できます。大きいファイルは pull にフォールバックします
- **SSE heartbeat**（`sse_heartbeat_seconds`、既定 `30`）: アイドル SSE 接続がリバースプロキシで切断されないようにするイベントストリームの keep-alive。並行 SSE 購読は既定でユーザーごとに 16、全体上限は 1024 です。開いているイベントストリームは bearer token を定期的に再検証し、token の取り消しまたはアカウント無効化後に閉じます。
- **Git smart HTTP**（`enable_git_smart_http`、既定オフ）: 有効時、認可済み装置は `git clone https://_:<token>@host/git/<vault-id>` を使用できます。サーバーには `PATH` 内の `git` バイナリも必要で、公開 `/api/config` capability は両方の条件を反映します。

**Network and update checks** — `public_host`、bind address、trusted proxies、`[update_check].repo` は起動時に `config.toml` から読み込まれます。更新確認の有効化と間隔は SQLite に保存されるランタイム設定です。許可範囲は 60 秒から 30 日です。

## アクティビティ

アクティビティログは push、pull、create_vault、delete_vault、view_commit、view_history、view_diff などの同期、vault ライフサイクル、読み取り専用閲覧操作を記録します。含まれる項目:

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

アクティビティフィルターで特定のユーザーまたは操作種別を確認できます。

`create_vault` と `delete_vault` は、管理パネル、プラグイン、API からの vault 作成/削除操作に由来します。

## サーバー URL の共有

サーバーまたは Admin WebUI が表示する URL を共有します。

```text
https://sync.example.com/k_xxx/
```

これは機密情報として扱ってください。ユーザーパスワードではありませんが、デプロイメントキーを含み、プラグイン API トラフィックの最初の事前認証ゲートになります。

## メンテナンスチェックリスト

- `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` を運用スナップショットに使用します。出力ディレクトリは存在しないか空である必要があります。コマンドは `VACUUM INTO` で SQLite をスナップショットし、`vaults/` と `blobs/` をコピーし、pkvsyncd バージョン、コンポーネント hash、サイズ、数を含む `MANIFEST.json` を書き込みます。デフォルトでは `config.toml` を省略します。デプロイメントキーやその他のローカル秘密を保存して保護するつもりがある場合だけ、`--include-config` を追加してください。
- `pkvsyncd restore --input <backup-dir> --data-dir <dir>` で、存在しないか空のデータディレクトリに復元します。先に消去してよい対象であることを確認した場合のみ `--force` を追加してください。restore はコピー前に manifest hash を検証し、完了後に verify を実行します。
- メンテナンス後またはホストストレージ障害後に `pkvsyncd verify [--data-dir <dir>]` を実行します。参照される blob ファイルを確認し、孤立 blob を報告し、`git2` で vault git リポジトリを検証し、欠落、破損、git エラーでは非ゼロ終了します。`--no-fail` はレポートを残したまま成功終了コードを強制します。
- `pkvsyncd materialize <vault-id> -o <dir>` で vault の HEAD を平坦なファイルツリーとしてエクスポートします（テキストファイルはそのまま、バイナリ blob は blob ストアから解決されます）。オフラインエクスポート、臨時監査、コールドマイグレーションに有用です。`--at <commit-sha>` と組み合わせると、過去の commit を materialize できます。
- `[mcp].embed_in_serve = true` を設定すると、メインの `pkvsyncd serve` ポートの `/mcp` で read/write MCP Streamable HTTP endpoint を公開できます。独立 MCP プロセスとして `pkvsyncd mcp --transport http --bind 127.0.0.1:6711` を実行することもできます。単一 vault 専用の stdio セッションには `pkvsyncd mcp --vault <id>` を使用します。
- 大量の添付ファイル削除後は blob ガベージコレクションを実行します。
- ログとアクティビティで `401`、`403`、`404`、`409`、`429` の繰り返し応答を監視します。
- サーバーバイナリ、プラグインパッケージ、Docker イメージ、リバースプロキシ、ホスト OS を更新状態に保ちます。
- release tag を付ける前に CI を確認します。
- 各 release に Linux amd64、Linux arm64、Windows x64、プラグイン zip、checksums、GHCR Docker イメージ tag が含まれることを確認します。
