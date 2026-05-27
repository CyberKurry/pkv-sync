# CLI リファレンス

[English](./cli-reference.md) | [简体中文](./cli-reference.zh-CN.md) | [繁體中文](./cli-reference.zh-Hant.md) | 日本語 | [한국어](./cli-reference.ko.md)

`pkvsyncd` は PKV Sync のサーバーデーモンバイナリです。HTTP/WebSocket の同期 API、管理 UI、MCP サーバー、および少数の運用サブコマンドをホストします。

## グローバルオプション

以下のフラグはすべてのサブコマンドに適用されます。

- `-c, --config <PATH>`: TOML 設定ファイルのパス。デフォルト: `/etc/pkv-sync/config.toml`。
- `-h, --help`: ヘルプを表示します。
- `-V, --version`: CLI のバージョンを表示します。

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## サブコマンド

`pkvsyncd` は 9 つのサブコマンドを提供します。最も一般的な運用フローは `serve`、`genkey`、`migrate up`、`user add`、`backup`、`restore` です。

## pkvsyncd serve

HTTP サーバーを起動します。

### 形式

```text
pkvsyncd serve
```

### 説明

公開同期用の HTTP リスナー、管理 UI、SSE ストリーム、Git smart HTTP ルート、および設定されている場合は MCP HTTP エンドポイントを実行します。リスナーは `config.toml` の `[server].bind_addr` にバインドします。systemd 配下またはコンテナ内のフォアグラウンドプロセスとして実行してください。

### 例

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

データベースマイグレーションコマンドです。利用できる操作は `up` のみです。

### 形式

```text
pkvsyncd migrate up
```

### 説明

`server/migrations/` 配下の未適用 SQLite マイグレーションを `[storage].db_path` のデータベースに対して適用します。再実行しても安全であり、適用済みのマイグレーションはスキップされます。HTTP サーバーは起動時にも未適用マイグレーションを実行するため、手動の `migrate up` が必要となるのは通常、コールドリストアのフローや、オフラインバックアップを移行する場合に限られます。

### 例

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

`[server].deployment_key` に適したランダムなデプロイメントキーを生成します。

### 形式

```text
pkvsyncd genkey
```

### 説明

暗号学的にランダムな `k_*` トークンを標準出力に表示します。値を `config.toml` に貼り付け、独自の安全な経路でプラグイン/管理クライアントに共有してください。

### 例

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

ユーザー管理コマンドです。運用上の復旧(パスワード忘れ、アカウントロック)や、副次的なオペレーターアカウントのスクリプトによる初期構築に役立ちます。

### 形式

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### サブコマンド

- `add <USERNAME> [--admin]`: ユーザーを作成し、パスワードを対話的に入力します。
- `passwd <USERNAME>`: ユーザーのパスワードをリセットし、新しい値を対話的に入力します。
- `list`: すべてのユーザーを管理者/有効状態および作成時刻とともに一覧表示します。
- `set-active <USERNAME> --active <true|false>`: ユーザーを無効化または再有効化します。無効化されたユーザーはトークンを保持しますが、ログインや同期はできません。

### 例

```bash
# 緊急アクセス用の管理者アカウントを作成
pkvsyncd user add alice --admin

# パスワード忘れをリセット
pkvsyncd user passwd alice

# 退職するユーザーをデータを削除せずに無効化
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

PKV Sync ボールトの bare Git リポジトリを、ディスク上の通常のファイルツリーに展開します。

### 形式

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### オプション

- `-o, --output <DIR>`: 出力ディレクトリ(存在しないか空である必要があります)。
- `--at <SHA>`: 特定のコミットでマテリアライズします(デフォルト: HEAD)。

### 説明

`data_dir/vaults/<vault-id>` 配下にあるボールトの bare Git リポジトリを読み取り、各ファイルを出力ディレクトリに書き込みます。

- テキストファイルはそのまま書き込まれます。
- `pkvsync_pointer` JSON として保存されているバイナリファイルは、サーバーの blob ストレージ(`data_dir/blobs/`)から実際の blob をコピーして解決します。

コマンドは同期的に動作し、サーバーが起動している必要はありません。設定された `data_dir` 配下のディスク上の Git リポジトリと blob ストレージから直接読み取ります。

### 例

```bash
# 最新バージョンをマテリアライズ
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 特定のコミットをマテリアライズ
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 終了コード

- `0`: 成功。
- `1`: 出力ディレクトリが空でない、ボールトが見つからない、blob が欠落している、コミット SHA が無効、などのエラー。

> ボールト ID は 32 文字の小文字 16 進数(ハイフンなし)です。上記の例は実際の形式の ID を使用しており、管理 UI と `pkvsyncd user list` で有効な ID を確認できます。

## pkvsyncd backup

サーバーデータをポータブルなバックアップディレクトリにスナップショットとして保存します。

### 形式

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### オプション

- `-o, --output <DIR>`: バックアップの出力ディレクトリ(存在しないか空である必要があります)。
- `--data-dir <DIR>`: オフライン操作用にデータディレクトリを上書きします。デフォルトでは読み込まれた設定の `[storage].data_dir` が使われます。
- `--gzip`: バックアップディレクトリの隣に `.tar.gz` アーカイブも作成します。
- `--include-config`: 読み込んだ `config.toml` を backup に含めます。デフォルトでは、deployment key などのローカル秘密を含み得るため config は省略されます。

### 説明

SQLite データベース(ソースをブロックしないように VACUUM INTO 経由)、各ボールトの bare Git リポジトリ、および blob ストアを、`MANIFEST.json` を含む自己完結型のディレクトリにスナップショットします。バックアップ中も HTTP サーバーは稼働を続けられます。ボールトのプッシュは、そのリポジトリをコピーしている間だけ、ボールト単位で一時的に静止状態になります。

デフォルトでは、バックアップは `config.toml` を省略します。設定を保存し、その秘密情報を保護するつもりがある場合だけ `--include-config` を追加してください。

### 例

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

バックアップディレクトリをデータディレクトリにリストアします。

### 形式

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### オプション

- `-i, --input <DIR>`: `MANIFEST.json` を含むバックアップディレクトリ。
- `--data-dir <DIR>`: 対象データディレクトリの上書き。デフォルトは `[storage].data_dir`。
- `--force`: リストア前に空でない対象データディレクトリをクリアします。

### 説明

バックアップの `MANIFEST.json` を検証し、SQLite DB、ボールトリポジトリ、blob ストアを対象データディレクトリへコピーします。リストア前に HTTP サーバーを停止してください。古いサーバーバージョンで取得したバックアップをリストアする場合は、リストア後に `pkvsyncd migrate up` を実行してください。

### 例

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

ボールトの Git リポジトリと内容アドレス指定された blob を検証します。

### 形式

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### オプション

- `--data-dir <DIR>`: データディレクトリの上書き。
- `--no-fail`: 検証でエラーが見つかった場合でも終了コード 0 を返します。ページングなしにログだけ取りたい監視スクリプトに便利です。

### 説明

`data_dir/vaults/` 配下の各ボールトに対して次を実行します。

- bare リポジトリに対して `git fsck --strict` を実行します。
- HEAD ツリーをたどり、すべての `pkvsync_pointer` が、ディスク上の SHA-256 がファイル名と一致する blob に解決されることを検証します。

ボールトごとのエラー件数を報告します。いずれかのボールトにエラーがある場合は非ゼロで終了します。ただし `--no-fail` が設定されている場合を除きます。

### 例

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

AI ツール向けの MCP(Model Context Protocol)サーバーを起動します。

### 形式

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### オプション

- `--transport <stdio|http>`: トランスポートモード。デフォルト: `stdio`。
- `--vault <VAULT-ID>`: stdio で必須。クライアントに公開する単一のボールトです。
- `--token <PKS-TOKEN>`: stdio 用のベアラーデバイストークン。省略した場合は環境変数 `PKV_TOKEN` が使われます。
- `--bind <ADDR>`: HTTP のバインドアドレス。デフォルト: `127.0.0.1:6711`。

### 説明

`stdio` モードは標準入力から JSON-RPC を読み取り、標準出力に JSON-RPC を書き込みます。`http` モードはステートレスな Streamable HTTP MCP エンドポイントを `/mcp` で提供します。どちらのモードも同じツールセットを公開します: `list_vaults`、`list_files`、`read_file`、`read_file_at_commit`、`search`、`write_file`、`delete_file`。書き込み系ツールは `(token, vault)` ごとに 1 分あたり 60 回までにレート制限されます。

`http` モードでは、通常の同期 API と同じく、すべてのリクエストにサーバーのデプロイメントキーヘッダーを付与する必要があります。


このサブコマンドは引き続き独立 MCP プロセスです。同じ Streamable HTTP transport をメインサーバーポートから提供するには、`[mcp].embed_in_serve = true` を設定し、`pkvsyncd serve` を使います。
### 例

```bash
# 環境変数のトークンを使った stdio
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# ローカルの Streamable HTTP エンドポイント
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

PKV Sync のリリースバイナリを現在の実行ファイルの隣にダウンロードします。

### 形式

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### オプション

- `--dry-run`: 何もダウンロードせずに、選択されたリリース、アセット、対象パスを表示します。
- `--yes`: 対話的な確認プロンプトをスキップします。
- `--version <VERSION>`: 最新リリースではなく `1.0.6` のような特定のリリースをダウンロードします。

### 説明

このコマンドは現在のプラットフォーム向けのリリースアセットを選択し、ダウンロードを `SHA256SUMS` に照らして検証し、現在のバイナリの隣に `pkvsyncd.new`(Windows では `pkvsyncd.new.exe`)を書き出し、systemd または手動での差し替え手順を表示します。稼働中のサーバーをホットリプレースすることはありません。

Docker および Kubernetes のデプロイは、イメージタグをプルまたは変更し、サービスやロールアウトを再起動することでアップグレードすべきです。コンテナ環境を検出した場合、コマンドはイメージベースのガイダンスを表示し、バイナリを書き出さずに終了します。

### 例

```bash
# アップグレード計画をプレビュー
pkvsyncd upgrade --dry-run

# 最新の検証済みバイナリをダウンロード
pkvsyncd upgrade --yes

# 特定のリリースをダウンロード
pkvsyncd upgrade --yes --version 1.0.6
```
