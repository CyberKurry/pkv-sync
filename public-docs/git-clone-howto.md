# Git Clone Your PKV Vault

PKV Sync can expose each vault as a read-only Git repository over HTTPS.

## Prerequisites

- Server admin has enabled "Git smart HTTP" in Sync & Storage settings
- `git` binary is available on the server
- You have a valid device token

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

The underscore before the colon is the username (any value works — only the token matters as the password).

### Example

If your server is at `sync.example.com`, your vault ID is `abc123`, and your device token is `pks_0f1e2d3c4b5a6978…`, run:

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/abc123
```

## Materialize

After cloning, blob files appear as pointer JSON (the PKV Sync server stores large files separately). Run:

```bash
pkvsyncd materialize <vault-id> ./output
```

This replaces pointer files with actual binary content, producing a fully usable local copy of your vault.

## Notes

- The repository is **read-only** over HTTP. You cannot push changes back via Git.
- Use the PKV Sync plugin to make changes and push them through the normal sync API.
- If the server admin disables Git smart HTTP, clone/fetch operations will return HTTP 503.

---

# 通过 Git 克隆你的 PKV 仓库

PKV Sync 可以将每个仓库（vault）通过 HTTPS 以只读 Git 仓库的形式暴露出来。

## 前提条件

- 服务器管理员已在"同步与存储"设置中启用"Git smart HTTP"
- 服务器上可用的 `git` 二进制文件
- 你拥有有效的设备令牌（device token）

## 克隆

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

冒号前的下划线是用户名（可以填写任意值——只有密码部分即令牌有效）。

### 示例

如果你的服务器地址为 `sync.example.com`，仓库 ID 为 `abc123`，设备令牌为 `pks_0f1e2d3c4b5a6978…`，运行：

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/abc123
```

## 还原（Materialize）

克隆后，二进制文件会以指针 JSON 的形式出现（PKV Sync 服务器将大文件单独存储）。运行：

```bash
pkvsyncd materialize <vault-id> ./output
```

这会将指针文件替换为实际的二进制内容，生成一个完整可用的本地仓库副本。

## 注意事项

- 该仓库通过 HTTP **只读**。你不能通过 Git 推送更改。
- 请使用 PKV Sync 插件进行更改并通过常规同步 API 推送。
- 如果服务器管理员禁用了 Git smart HTTP，克隆/拉取操作将返回 HTTP 503。
