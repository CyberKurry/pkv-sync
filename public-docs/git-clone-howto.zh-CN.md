# 通过 Git 克隆你的 PKV 仓库

[English](./git-clone-howto.md) | 简体中文 | [繁體中文](./git-clone-howto.zh-Hant.md) | [日本語](./git-clone-howto.ja.md) | [한국어](./git-clone-howto.ko.md)

文档版本：v1.0.14。

PKV Sync 可以将每个仓库（vault）通过 HTTPS 以只读 Git 仓库的形式暴露出来。

## 前提条件

- 服务器管理员已在“同步与存储”设置中启用“Git smart HTTP”。
- 服务器上有可用的 `git` 二进制文件。
- 你拥有有效的设备令牌（device token）。

## 克隆

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

冒号前的下划线是用户名。可以填写任意值；只有密码部分的令牌有效。

### 示例

如果你的服务器地址为 `sync.example.com`，仓库 ID 为 `6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`，设备令牌为 `pks_0f1e2d3c4b5a6978...`，运行：

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID 是 32 个字符的小写十六进制（不含连字符）。Admin WebUI 和 `pkvsyncd user list` 会显示有效 ID；像 `abc123` 这样的占位符会被以 `400 invalid_vault_id` 拒绝。

## 还原（Materialize）

克隆后，二进制文件会以指针 JSON 的形式出现，因为 PKV Sync 服务器会单独存储大文件。运行：

```bash
pkvsyncd materialize <vault-id> -o ./output
```

这会将指针文件替换为实际的二进制内容，生成一个完整可用的本地仓库副本。

## 注意事项

- 该仓库通过 HTTP **只读**。你不能通过 Git 推送更改。
- 请使用 PKV Sync 插件进行更改，并通过常规同步 API 推送。
- 如果服务器管理员禁用了 Git smart HTTP，克隆或拉取操作将返回 HTTP 503。
