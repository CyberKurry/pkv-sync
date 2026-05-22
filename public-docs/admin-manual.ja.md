# PKV Sync 管理者マニュアル

[English](./admin-manual.md) | [简体中文](./admin-manual.zh-CN.md) | [繁體中文](./admin-manual.zh-Hant.md) | 日本語 | [한국어](./admin-manual.ko.md)

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

このマニュアルでは、セルフホストした PKV Sync サーバーの日常管理を扱います。ネットワークとホストの強化については、deployment hardening guide も併せて読んでください。

## 初回実行

1. デプロイメントキーを生成します。

   ```bash
   pkvsyncd genkey
   ```

2. `config.example.toml` から `/etc/pkv-sync/config.toml` を作成します。
3. データベース migration を適用します。

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. サーバーを起動します。

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. stderr またはコンテナログに出力される初回管理者パスワードを保存します。
6. `/admin/login` を開き、`admin` としてサインインしてパスワードを変更します。

リリース後の migration は意図的に追記のみで管理します。既存デプロイメント向けに、公開済みの migration ファイルを squash したり編集したりしないでください。

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
- General、Security、Sync & Storage、Network に分かれた実行時設定
- 同期、vault ライフサイクル、読み取り専用閲覧行を対象に、実ユーザーとアクションでフィルターできるアクティビティログ
- Blob ガベージコレクションのトリガー
- 英語と簡体字中国語の言語切替

タイムスタンプ、期間、バイトサイズ、稼働時間、アクティビティデータは人間が読みやすい形式で表示されます。既定のタイムゾーンは `Asia/Shanghai` で、設定から変更できます。

## ユーザー管理

- **Users** または CLI からユーザーを作成します。
- ユーザー名は 3-32 文字の ASCII 英字、数字、`_`、`-`、`.` のいずれかである必要があります。
- Users ページの検索とステータスフィルターで表を絞り込めます。
- ユーザー詳細ページを開くと、パスワードリセット、アカウントの有効化/無効化、管理者権限の付与/解除、そのユーザーの装置 token の確認ができます。
- 監査履歴が必要になる可能性がある場合は、削除ではなく無効化を優先してください。
- 最後のアクティブな管理者アカウントを降格または無効化しないでください。

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

装置 bearer token は認証済みの使用時に更新され、90 日間使用されないと期限切れになります。ユーザーは自分の token を取り消せ、管理者は任意のユーザーの token を取り消せます。

運用上の注意:

- Token の平文は作成時に一度だけ表示されます。
- データベースには SHA-256 token hash のみ保存されます。
- 各認証済みリクエストは、そのリクエスト時刻から 90 日後まで token 期限を延長し、より遅い期限を短縮しません。
- 同じ安定したプラグイン装置 ID から再ログインすると、その装置の以前のアクティブ token が置き換えられます。
- アクティビティ行から参照される取り消し済み token は、アクティビティ履歴を残したままクリーンアップできます。

## Vault

vault を削除すると次が削除されます。

- vault データベース行
- そこから cascade される関連メタデータ行
- `data_dir/vaults/<vault-id>` 配下のバックエンド bare Git リポジトリ
- メモリ内の vault ごとの push ロック

Blob ファイルは内容アドレス指定であり、ガベージコレクションが猶予期間を超えて未参照であることを確認するまで残る場合があります。

中断された操作の後にファイル数、サイズ、blob 参照が正しく見えない場合は、vault メタデータ reconciliation を使用してください。

### Vault ごとの同期設定

**Vaults** から vault カードの **Settings** を開き、vault ごとの `extra_sync_globs` allowlist を編集します。これは、選択した `.obsidian` 設定ファイルを含む隠しパスのうち、どれを同期できるかを制御します。

新しい vault には推奨 starter allowlist が自動的に設定されます。既存の vault は、管理者または vault 所有者が starter list を適用するまで空のままです。**Apply starter allowlist** は、テーマ、CSS snippets、ホットキー、アプリ設定、外観設定、有効化済みプラグイン一覧向けの推奨リストを書き込みます。

### 読み取り専用ファイル履歴

**Vaults** から vault カードの **Browse files** を開きます。ブラウザーは現在の HEAD ファイルを、サイズとテキスト/バイナリ種別付きで一覧表示します。ファイルを開くと、テキストファイルは読み取り専用プレビューを表示し、**History** と **Diff with previous** へのリンクを提供します。

履歴ページはそのファイルの commit を一覧表示し、各 commit 時点のファイルと対応する diff へのリンクを提供します。diff ページは unified diff の行を追加/削除/hunk の色分け付きでレンダリングします。バイナリファイルはメタデータのみ表示し、バイナリ diff 内容はレンダリングしません。

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

**Security** — 登録モード（`disabled` / `invite_only` / `open`）、ログイン失敗しきい値、失敗ウィンドウ、ロック時間。ログインレートリミッターは失敗回数と進行中のパスワード検証の両方を数えるため、同時大量推測でしきい値を回避できません。認証済み同期 API ルートには、ルート、メソッド、クライアント IP、bearer 装置 token ごとに 60 秒あたり最大 600 リクエストの固定ウィンドウ制限があります。

**Sync & Storage**
- 最大ファイルサイズ（既定 `100 MiB`）
- 対応テキスト拡張子 — 一覧外のファイルはバイナリ blob として扱われます
- 追加 exclude glob — 組み込みの `.obsidian/`、`.trash/`、`.conflict-*`、`.git/` 除外リストを補う管理者調整可能なパターン
- 履歴 UI と diff エンドポイントの切替
- **Push debounce**（`push_debounce_ms`、既定 `250`）: ローカル編集が落ち着いてから push するまでの待機時間。小さくするとエンドツーエンド遅延が減り、大きくすると 1 回の push でより多くの入力をまとめられます
- **Inline SSE content cap**（`inline_content_max_bytes`、既定 `8192`、最大 `65536`）: このサイズまでのテキスト変更は SSE イベント内で送信され、受信プラグインは別途 pull せずに適用できます。大きいファイルは pull にフォールバックします
- **SSE heartbeat**（`sse_heartbeat_seconds`、既定 `30`）: アイドル SSE 接続がリバースプロキシで切断されないようにするイベントストリームの keep-alive。並行 SSE 購読は既定でユーザーごとに 16、全体上限は 1024 です。
- **Git smart HTTP**（`enable_git_smart_http`、既定オフ）: 有効時、認可済み装置は `git clone https://_:<token>@host/git/<vault-id>` を使用できます。サーバーには `PATH` 内の `git` バイナリも必要で、公開 `/api/config` capability は両方の条件を反映します。

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

- `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` を運用スナップショットに使用します。出力ディレクトリは存在しないか空である必要があります。コマンドは `VACUUM INTO` で SQLite をスナップショットし、`vaults/`、`blobs/`、存在する場合は `config.toml` をコピーし、pkvsyncd バージョン、コンポーネント hash、サイズ、数を含む `MANIFEST.json` を書き込みます。
- `pkvsyncd restore --input <backup-dir> --data-dir <dir>` で、存在しないか空のデータディレクトリに復元します。先に消去してよい対象であることを確認した場合のみ `--force` を追加してください。restore はコピー前に manifest hash を検証し、完了後に verify を実行します。
- メンテナンス後またはホストストレージ障害後に `pkvsyncd verify [--data-dir <dir>]` を実行します。参照される blob ファイルを確認し、孤立 blob を報告し、`git2` で vault git リポジトリを検証し、欠落、破損、git エラーでは非ゼロ終了します。`--no-fail` はレポートを残したまま成功終了コードを強制します。
- 大量の添付ファイル削除後は blob ガベージコレクションを実行します。
- ログとアクティビティで `401`、`403`、`404`、`409`、`429` の繰り返し応答を監視します。
- サーバーバイナリ、プラグインパッケージ、Docker イメージ、リバースプロキシ、ホスト OS を更新状態に保ちます。
- release tag を付ける前に CI を確認します。
- 各 release に Linux amd64、Linux arm64、Windows x64、プラグイン zip、checksums、GHCR Docker イメージ tag が含まれることを確認します。
