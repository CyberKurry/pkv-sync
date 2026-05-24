# CLI リファレンス

[English](./cli-reference.md) | [简体中文](./cli-reference.zh-CN.md) | [繁體中文](./cli-reference.zh-Hant.md) | 日本語 | [한국어](./cli-reference.ko.md)

## pkvsyncd materialize

PKV Sync vault の bare git repository を、ディスク上の通常のファイルツリーへ展開します。

### Synopsis

```text
pkvsyncd materialize <vault-id> -o <output-dir> [--at <commit-sha>]
```

### Options

- `-o, --output <DIR>`：出力先ディレクトリ。存在しないか空である必要があります。
- `--at <SHA>`：指定 commit を materialize します。既定は HEAD です。

### Description

vault の bare git repository を読み、各ファイルを出力先へ書き出します。

- テキストファイルはそのまま書き出されます。
- `pkvsync_pointer` JSON として保存されたバイナリファイルは、server の blob storage から実体をコピーして復元されます。

このコマンドは同期的に実行され、server が起動している必要はありません。設定済み `data_dir` 配下の git repository と blob storage を直接読み取ります。

### Examples

```bash
# 最新版を materialize
pkvsyncd materialize abc123 -o ./my-vault

# 特定 commit を materialize
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```

### Exit Codes

- `0`：成功。
- `1`：エラー。出力先が空でない、vault が見つからない、blob が欠落している、commit SHA が無効、など。

## pkvsyncd mcp

AI ツール向け MCP server を起動します。

### Synopsis

```text
pkvsyncd mcp [--transport stdio|http] [--vault <vault-id>] [--token <pks-token>] [--bind <addr>]
```

### Options

- `--transport <stdio|http>`：transport mode。既定は `stdio` です。
- `--vault <vault-id>`：stdio では必須。client に公開する単一 vault です。
- `--token <pks-token>`：stdio 用 bearer device token。省略時は `PKV_TOKEN` を読みます。
- `--bind <addr>`：HTTP bind address。既定は `127.0.0.1:6711` です。

### Description

stdio mode は stdin から JSON-RPC を読み、stdout に JSON-RPC を書きます。HTTP mode は `/mcp` で stateless Streamable HTTP MCP endpoint を提供します。どちらの mode も `list_vaults`、`list_files`、`read_file`、`read_file_at_commit`、`search`、`write_file`、`delete_file` を公開します。

### Examples

```bash
# stdio、token は環境変数から取得
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault abc123

# ローカル Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

HTTP mode では、すべての request に server deployment key header が必要です。

## pkvsyncd upgrade

PKV Sync release binary を、現在の実行ファイルの隣へ side-by-side でダウンロードします。

### Synopsis

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <version>]
```

### Options

- `--dry-run`：選択される release、asset、target path を表示し、ダウンロードしません。
- `--yes`：確認プロンプトを省略します。
- `--version <version>`：最新ではなく、`1.0.0` のような指定 release をダウンロードします。

### Description

現在の platform に合う release asset を選択し、`SHA256SUMS` で検証し、現在の binary の隣に `pkvsyncd.new`（Windows では `pkvsyncd.new.exe`）を書き込み、systemd／手動 swap 手順を表示します。稼働中の server を hot replace することはありません。

Docker と Kubernetes deployment は、image tag を pull または変更して service／rollout を再起動してください。container 環境を検出した場合、この command は image ベースの案内を表示して終了し、binary は書き込みません。

### Examples

```bash
# アップグレード計画を確認
pkvsyncd upgrade --dry-run

# 最新の検証済み binary をダウンロード
pkvsyncd upgrade --yes

# 指定 release をダウンロード
pkvsyncd upgrade --yes --version 1.0.0
```
