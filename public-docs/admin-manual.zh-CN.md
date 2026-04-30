# PKV Sync 管理员手册

[English](./admin-manual.md) | 简体中文

## 首次运行

1. 生成部署密钥：

   ```bash
   pkvsyncd genkey
   ```

2. 基于 `config.example.toml` 创建 `/etc/pkv-sync/config.toml`。
3. 执行数据库迁移：

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. 启动服务器：

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 保存首次启动时输出到 stderr 的管理员密码。

## 管理后台

打开：

```text
https://sync.example.com/admin/login
```

使用首次启动的管理员凭据登录，然后修改密码。

管理后台包含仪表盘、用户、邀请码、运行时设置、活动记录和 blob 垃圾回收页面。

## 用户管理

- 在 **用户** 页面创建用户。
- 如果后续可能需要审计历史，请禁用用户而不是删除用户。
- 重置密码会撤销已有设备 token。
- 不要降级或禁用最后一个活跃管理员账号。

CLI 兜底命令：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 注册模式

可在 **设置** 页面配置注册方式：

- `disabled`：由管理员手动创建账号
- `invite_only`：用户使用邀请码注册
- `open`：任何拥有部署 URL 的人都可以注册

只有在短时间窗口或带额外监控的公开部署中，才建议使用 `open`。

## 分享服务器 URL

分享服务器输出的 URL：

```text
https://sync.example.com/k_xxx/
```

请把它视为敏感信息。它不是密码，但它是认证前的第一道入口，不应公开发布给不属于私有部署的人。

## 维护

- 监控仪表盘中的 CPU、内存、用户、仓库和磁盘指标。
- 大量删除文件后运行 blob 垃圾回收。
- 对 metadata、仓库 Git 目录、blobs 和配置做加密备份。
- 关注日志中重复出现的 401、403、404、409 和 429 响应。
- 及时更新服务器二进制、反向代理和宿主机操作系统。
