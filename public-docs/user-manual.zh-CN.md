# PKV Sync 用户手册

[English](./user-manual.md) | 简体中文

## 手动安装插件

1. 从 release 下载 `pkv-sync-plugin.zip`。
2. 解压到你的 Obsidian 仓库：

   ```text
   <vault>/.obsidian/plugins/pkv-sync/
   ```

3. 在 Obsidian 中启用社区插件。
4. 启用 **PKV Sync**。

## 连接服务器

向管理员获取 PKV Sync 服务器 URL，通常类似：

```text
https://sync.example.com/k_xxx/
```

在 **设置 -> PKV Sync -> 服务器 URL** 中粘贴该地址，然后点击 **连接**。

如果 URL 中已经包含部署密钥，插件会自动填入。

## 登录或注册

具体流程取决于服务器设置：

- 如果注册已关闭，请让管理员创建账号。
- 如果启用了邀请码注册，请输入你的邀请码。
- 如果启用了开放注册，可以直接创建账号。

登录后，选择要同步的远程仓库。

## 同步行为

PKV Sync 会：

- 在本地改动停止约 2 秒后推送变更
- 约每 60 秒拉取远端变更
- 在切换文件、窗口失焦，或手动运行 **PKV Sync: Manual sync now** 时同步
- 在下次启动 Obsidian 时恢复尚未同步的本地变更

上传大型附件时，请让 Obsidian 保持打开，直到上传完成。

## 冲突

如果两台设备离线编辑了同一个文件，PKV Sync 会保留两个版本。

示例：

```text
note.md
note.conflict-2026-04-25-143022-Android-device.md
```

在 Obsidian 中打开两个文件，手动合并内容，然后删除冲突文件。

## 设备 Token

每次登录都会创建一个设备 token。

- 可在插件设置中退出当前设备。
- 丢失设备后，请让管理员撤销对应设备 token。
- 修改密码会保留当前设备登录状态，并撤销其它设备 token。

## 隐私提醒

PKV Sync 不提供端到端加密。服务器管理员，以及任何拥有服务器文件系统访问权限的人，都可以读取已同步的仓库内容。
