# CLI 参考

[English](./cli-reference.md) | 简体中文 | [繁體中文](./cli-reference.zh-Hant.md) | [日本語](./cli-reference.ja.md) | [한국어](./cli-reference.ko.md)

文档版本：v1.3.1。

`pkvsyncd` 是 PKV Sync 的服务端守护进程二进制文件。它承载 HTTP/WebSocket 同步 API、管理界面、MCP 服务器，以及一小组运维子命令。

## 全局选项

以下选项对所有子命令生效：

- `-c, --config <PATH>`：TOML 配置文件路径。默认值：`/etc/pkv-sync/config.toml`。
- `-h, --help`：显示帮助信息。
- `-V, --version`：打印 CLI 版本。

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 子命令

`pkvsyncd` 提供九个子命令。最常用的运维流程是 `serve`、`genkey`、`migrate up`、`user add`、`backup` 和 `restore`。

## pkvsyncd serve

启动 HTTP 服务器。

### 概述

```text
pkvsyncd serve
```

### 说明

运行公开的同步 HTTP 监听器、管理界面、SSE 流、Git smart HTTP 路由，以及在已配置时的 MCP HTTP 端点。监听器绑定到 `config.toml` 中的 `[server].bind_addr`。请将其作为前台进程在 systemd 或容器中运行。

### 示例

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

数据库迁移命令。唯一支持的操作是 `up`。

### 概述

```text
pkvsyncd migrate up
```

### 说明

将 `server/migrations/` 中所有待执行的 SQLite 迁移应用到 `[storage].db_path` 处的数据库。可安全地重复运行，已应用的迁移会被跳过。HTTP 服务器在启动时也会运行待执行的迁移，因此手动执行 `migrate up` 通常只在冷恢复流程或迁移离线备份时才需要。

### 示例

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

生成一个适合用于 `[server].deployment_key` 的随机部署密钥。

### 概述

```text
pkvsyncd genkey
```

### 说明

向 stdout 打印一个加密学随机的 `k_*` 令牌。将该值粘贴到 `config.toml` 中，并通过你自己的安全渠道分发给 plugin/admin 客户端。

### 示例

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

用户管理命令。适用于运维恢复（忘记密码、账户被锁定）以及通过脚本批量初始化次级运维账户。

### 概述

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 子命令

- `add <USERNAME> [--admin]`：创建一个用户，并以交互方式提示输入密码。
- `passwd <USERNAME>`：重置某用户的密码，交互式提示输入新密码。
- `list`：列出所有用户，包括其管理员/启用状态以及创建时间。
- `set-active <USERNAME> --active <true|false>`：禁用或重新启用某个用户。被禁用的用户保留其令牌，但无法登录或同步。

### 示例

```bash
# 创建一个用于紧急访问的管理员账户
pkvsyncd user add alice --admin

# 重置忘记的密码
pkvsyncd user passwd alice

# 在不删除数据的情况下禁用一个离职用户
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

将 PKV Sync 保险库的裸 git 仓库展开为磁盘上的普通文件树。

### 概述

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 选项

- `-o, --output <DIR>`：输出目录（必须不存在或为空）。
- `--at <SHA>`：在指定 commit 处物化（默认值：HEAD）。

### 说明

读取 `data_dir/vaults/<vault-id>` 下保险库的裸 git 仓库，并将每个文件写入输出目录：

- 文本文件按原样写入。
- 以 `pkvsync_pointer` JSON 形式存储的二进制文件，通过从服务器的 blob 存储（`data_dir/blobs/`）复制实际的 blob 来解析。

该命令是同步的，且不要求服务器正在运行。它直接从已配置的 `data_dir` 下的磁盘 git 仓库和 blob 存储读取。

### 示例

```bash
# 物化最新版本
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 物化特定 commit
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 退出码

- `0`：成功。
- `1`：错误，例如输出目录非空、保险库不存在、blob 缺失或 commit SHA 无效。

> 保险库 ID 是 32 位的小写十六进制（不含短横线）。上面的示例使用真实形态的 ID；管理界面与 `pkvsyncd user list` 会显示有效的 ID。

## pkvsyncd backup

将服务器数据快照到一个可移植的备份目录中。

### 概述

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 选项

- `-o, --output <DIR>`：备份输出目录（必须不存在或为空）。
- `--data-dir <DIR>`：用于离线运维的 data 目录覆盖项。默认使用加载的配置中的 `[storage].data_dir`。
- `--gzip`：在备份目录旁额外生成一个 `.tar.gz` 归档文件。
- `--include-config`：把已加载的 `config.toml` 一并写入备份。默认备份会省略配置文件，因为其中可能包含部署密钥和本机秘密。

### 说明

将 SQLite 数据库（通过 VACUUM INTO，因此源库不会被阻塞）、每个保险库的裸 git 仓库以及 blob 存储，快照到一个带有 `MANIFEST.json` 的自包含目录中。备份期间 HTTP 服务器可以继续运行；在复制各个保险库仓库时，对应保险库的 push 会被短暂静止。

默认情况下，备份会省略 `config.toml`；只有在你明确要保存配置并保护其中秘密时，才添加 `--include-config`。

### 示例

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

将一个备份目录恢复到指定的 data 目录中。

### 概述

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 选项

- `-i, --input <DIR>`：包含 `MANIFEST.json` 的备份目录。
- `--data-dir <DIR>`：目标 data 目录覆盖项。默认使用 `[storage].data_dir`。
- `--force`：在恢复前清空非空的目标 data 目录。

### 说明

校验备份的 `MANIFEST.json`，并将 SQLite 数据库、保险库仓库与 blob 存储复制到目标 data 目录。在恢复之前请先停止 HTTP 服务器。恢复完成后，如果你恢复的是更老服务器版本生成的备份，请运行 `pkvsyncd migrate up`。

### 示例

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

校验保险库的 git 仓库和内容寻址的 blob。

### 概述

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 选项

- `--data-dir <DIR>`：data 目录覆盖项。
- `--no-fail`：即使校验发现错误也返回退出码 0。适用于希望仅记录而不触发告警的监控脚本。

### 说明

对 `data_dir/vaults/` 下的每个保险库：

- 在裸仓库上运行 `git fsck --strict`。
- 遍历 HEAD 树，并验证每个 `pkvsync_pointer` 都能解析到一个 blob，且其在磁盘上的 SHA-256 与文件名匹配。

按保险库报告错误计数。当任何保险库存在错误时以非零退出码退出，除非设置了 `--no-fail`。

### 示例

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

为 AI 工具启动 MCP（Model Context Protocol）服务器。

### 概述

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 选项

- `--transport <stdio|http>`：传输模式。默认值：`stdio`。
- `--vault <VAULT-ID>`：stdio 模式必填；要向客户端暴露的单一保险库。
- `--token <PKS-TOKEN>`：stdio 模式使用的 bearer 设备令牌。若省略，则使用 `PKV_TOKEN` 环境变量。
- `--bind <ADDR>`：HTTP 绑定地址。默认值：`127.0.0.1:6711`。

### 说明

`stdio` 模式从 stdin 读取 JSON-RPC，并向 stdout 写入 JSON-RPC。`http` 模式在 `/mcp` 上提供一个无状态的 Streamable HTTP MCP 端点。两种模式暴露相同的工具集：`list_vaults`、`list_files`、`read_file`、`read_file_at_commit`、`search`、`link_graph`、`changes_since`、`write_file`、`delete_file`、`write_files` 和 `move_file`。`write_files` 适合原子的多页 wiki 编辑，`move_file` 适合保留历史的重命名或归档移动。写入类工具按 `(token, vault)` 限流为每分钟 60 次写入，且一个 `write_files` 批次只消耗一次写入记录。搜索请求最多扫描 5000 个可见 tree 文件、返回 500 条匹配，并在生产环境搜索文本累计达到 256 MiB 后停止。`link_graph` 最多扫描 5000 个可见文本文件，并使用同一个生产文本预算；`changes_since` 最多返回 5000 条可见变更。超过 64 MiB 的二进制/blob 读取响应会被拒绝，而不是被 base64 展开进 JSON。

`http` 模式要求每个请求都携带服务器部署密钥请求头，与常规同步 API 一致。


这个子命令仍然是独立 MCP 进程。若要把同一个 Streamable HTTP transport 挂到主服务端口，请设置 `[mcp].embed_in_serve = true` 并运行 `pkvsyncd serve`。
### 示例

```bash
# 使用环境变量中的 token 启动 stdio
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 本地 Streamable HTTP 端点
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

在当前可执行文件旁下载一个 PKV Sync 发行版二进制文件。

### 概述

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 选项

- `--dry-run`：显示所选的发行版、资产和目标路径，但不下载任何文件。
- `--yes`：跳过交互式确认提示。
- `--version <VERSION>`：下载指定版本（例如 `1.3.1`），而不是最新发行版。

### 说明

该命令为当前平台选择对应的发行版资产，针对 `SHA256SUMS` 校验下载结果，在当前二进制文件旁写入 `pkvsyncd.new`（Windows 上为 `pkvsyncd.new.exe`），并打印 systemd/手动切换步骤。它不会热替换正在运行的服务器。

Docker 与 Kubernetes 部署应通过拉取或修改镜像标签并重启服务/滚动更新来升级。当该命令检测到容器环境时，会打印基于镜像的指导信息并退出，不会写入任何二进制文件。

### 示例

```bash
# 预览升级计划
pkvsyncd upgrade --dry-run

# 下载最新的已校验二进制文件
pkvsyncd upgrade --yes

# 下载指定版本
pkvsyncd upgrade --yes --version 1.3.1
```
