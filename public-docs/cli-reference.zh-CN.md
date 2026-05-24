# CLI 参考

[English](./cli-reference.md) | 简体中文 | [繁體中文](./cli-reference.zh-Hant.md) | [日本語](./cli-reference.ja.md) | [한국어](./cli-reference.ko.md)

## pkvsyncd materialize

将 PKV Sync vault 的 bare git 仓库还原为普通文件树。

### 用法

```text
pkvsyncd materialize <vault-id> -o <输出目录> [--at <commit-sha>]
```

### 选项

- `-o, --output <目录>`：输出目录，必须不存在或为空。
- `--at <SHA>`：还原到指定提交，默认是 HEAD。

### 说明

读取 vault 的 bare git 仓库，将每个文件写入输出目录：

- 文本文件原样写入。
- 以 `pkvsync_pointer` JSON 存储的二进制文件，会通过从服务器 blob 存储复制实际文件来还原。

该命令为同步执行，无需服务器运行。它直接从配置的 `data_dir` 下的磁盘 git 仓库和 blob 存储中读取。

### 示例

```bash
# 还原最新版本
pkvsyncd materialize abc123 -o ./my-vault

# 还原到指定提交
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```

### 退出码

- `0`：成功。
- `1`：错误，例如输出目录非空、vault 不存在、blob 缺失或 commit SHA 无效。

## pkvsyncd mcp

启动面向 AI 工具的 MCP server。

### 用法

```text
pkvsyncd mcp [--transport stdio|http] [--vault <vault-id>] [--token <pks-token>] [--bind <addr>]
```

### 选项

- `--transport <stdio|http>`：transport 模式，默认是 `stdio`。
- `--vault <vault-id>`：stdio 必填，只向客户端暴露这一个笔记库。
- `--token <pks-token>`：stdio 使用的 bearer 设备 token；不传时读取 `PKV_TOKEN`。
- `--bind <addr>`：HTTP 监听地址，默认是 `127.0.0.1:6711`。

### 说明

stdio 模式从 stdin 读取 JSON-RPC，并向 stdout 写入 JSON-RPC。HTTP 模式在 `/mcp` 提供无状态 Streamable HTTP MCP 端点。两种模式都暴露 `list_vaults`、`list_files`、`read_file`、`read_file_at_commit`、`search`、`write_file` 和 `delete_file`。

### 示例

```bash
# stdio，从环境变量读取 token
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault abc123

# 本地 Streamable HTTP 端点
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

HTTP 模式每个请求都需要服务端部署密钥 header。

## pkvsyncd upgrade

把 PKV Sync release 二进制下载到当前可执行文件旁边，供运维手动替换。

### 用法

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <版本>]
```

### 选项

- `--dry-run`：只显示将使用的 release、资产和目标路径，不下载文件。
- `--yes`：跳过交互确认。
- `--version <版本>`：指定 release，例如 `1.0.0`；不传时使用最新 release。

### 说明

命令会根据当前平台选择 release 资产，用 `SHA256SUMS` 校验下载内容，把结果写到当前二进制旁边的 `pkvsyncd.new`（Windows 为 `pkvsyncd.new.exe`），并打印 systemd／手动替换步骤。它不会热替换正在运行的服务端。

Docker 和 Kubernetes 部署应通过拉取或修改镜像 tag 升级，然后重启服务或 rollout。命令检测到容器环境时，会输出镜像升级指引并退出，不写入旁路二进制。

### 示例

```bash
# 预览升级计划
pkvsyncd upgrade --dry-run

# 下载最新校验后的二进制
pkvsyncd upgrade --yes

# 下载指定 release
pkvsyncd upgrade --yes --version 1.0.0
```
