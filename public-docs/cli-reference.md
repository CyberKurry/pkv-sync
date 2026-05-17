# CLI Reference

## pkvsyncd materialize

Expand a PKV Sync vault's bare git repository into a plain file tree on disk.

### Synopsis

```
pkvsyncd materialize <vault-id> -o <output-dir> [--at <commit-sha>]
```

### Options

- `-o, --output <DIR>` — Output directory (must not exist or be empty)
- `--at <SHA>` — Materialize at a specific commit (default: HEAD)

### Description

Reads the vault's bare git repository and writes each file to the output directory:

- Text files are written as-is
- Binary files (stored as `pkvsync_pointer` JSON) are resolved by copying the actual blob from the server's blob storage

The command is synchronous and does not require the server to be running. It reads directly from the on-disk git repository and blob storage under the configured `data_dir`.

### Examples

```bash
# Materialize the latest version
pkvsyncd materialize abc123 -o ./my-vault

# Materialize a specific commit
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```

### Exit Codes

- `0` — Success
- `1` — Error (output dir not empty, vault not found, blob missing, invalid commit SHA, etc.)

---

## pkvsyncd materialize (中文)

将 PKV Sync vault 的 bare git 仓库还原为普通文件树。

### 用法

```
pkvsyncd materialize <vault-id> -o <输出目录> [--at <commit-sha>]
```

### 选项

- `-o, --output <目录>` — 输出目录（必须不存在或为空）
- `--at <SHA>` — 还原到指定提交（默认：HEAD）

### 说明

读取 vault 的 bare git 仓库，将每个文件写入输出目录：

- 文本文件原样写入
- 二进制文件（以 `pkvsync_pointer` JSON 存储）通过从服务器的 blob 存储复制实际文件来还原

该命令为同步执行，无需服务器运行。它直接从配置的 `data_dir` 下的磁盘 git 仓库和 blob 存储中读取。

### 示例

```bash
# 还原最新版本
pkvsyncd materialize abc123 -o ./my-vault

# 还原到指定提交
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```
