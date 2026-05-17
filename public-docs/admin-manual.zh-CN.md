# PKV Sync 管理员手册

[English](./admin-manual.md) | 简体中文

本文覆盖自托管 PKV Sync 服务端的日常管理。网络和主机加固请同时阅读部署加固指南。

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

4. 启动服务端：

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 保存输出到 stderr 或容器日志中的首次管理员密码。
6. 打开 `/admin/login`，使用 `admin` 登录并修改密码。

已发布后的 migration 应保持追加式管理。对于已有部署，不要压缩或编辑已经发布的 migration 文件。

## Admin Web 面板

打开：

```text
https://sync.example.com/admin/login
```

管理后台包含：

- 仪表盘：系统、存储、笔记库、用户和最近活动指标
- 用户列表，支持搜索和状态筛选
- 用户详情页：重置密码、启用/禁用、管理员权限控制和 token 查看
- 全局设备 token 页面，可列出、创建和撤销 token
- 笔记库卡片：所有者、文件数、大小、上次同步、元数据修复和删除操作
- 只读笔记库文件浏览器，支持文件预览、单文件历史时间线和 unified diff 渲染
- 邀请码创建，可选过期时间，活跃邀请码列表，以及删除未使用邀请码
- 运行时设置，分为 General、Security、Sync & Storage、Network
- 活动日志，支持按用户和动作真实筛选 push/pull 记录
- Blob 垃圾回收触发
- 英文和简体中文语言切换

时间戳、持续时间、字节大小、运行时长和活动数据都会以人类可读形式显示。默认时区是 `Asia/Shanghai`，可在设置中修改。

## 用户管理

- 可在 **Users** 页面或 CLI 创建用户。
- 用户名必须是 3-32 个 ASCII 字母、数字、`_`、`-` 或 `.`。
- 用户页面的搜索和状态筛选可以缩小表格范围。
- 打开用户详情页可重置密码、启用或禁用账号、提升或降低管理员权限，并查看该用户的设备 token。
- 如果后续可能需要审计历史，优先禁用用户而不是删除用户。
- 不要降级或禁用最后一个活跃管理员账号。

从 Admin WebUI 重置密码会撤销该用户已有设备 token。用户需要重新登录。

CLI 兜底命令：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 设备 Token

设备 bearer token 有效期为 90 天。用户可以撤销自己的 token，管理员可以撤销任意用户的 token。

运维注意事项：

- Token 明文只在创建时展示一次。
- 数据库只保存 SHA-256 token hash。
- 同一稳定插件设备 ID 再次登录时，会替换该设备旧的活跃 token。
- 被活动记录引用的已撤销 token 可以清理，同时保留活动历史。

## 笔记库

删除笔记库会移除：

- 笔记库数据库行
- 从该行级联的相关元数据
- `data_dir/vaults/<vault-id>` 下的后端裸 Git 仓库
- 内存中的按笔记库 push 锁

Blob 文件是内容寻址的，可能会保留到垃圾回收确认其超过宽限期且不再被引用。

如果中断操作后文件数、大小或 blob 引用看起来不正确，可以使用笔记库元数据修复。

### 只读文件历史

在 **Vaults** 页面点击某个笔记库卡片上的 **Browse files**。文件浏览器会列出当前 HEAD 中的文件、大小以及文本/二进制类型。打开文件后，文本文件会显示只读预览，并提供 **History** 和 **Diff with previous** 链接。

历史页会列出该文件相关的提交，并提供“查看该提交时的文件”和对应 diff 的链接。diff 页会按行渲染 unified diff，并用颜色区分新增、删除和 hunk。二进制文件只显示元数据，不渲染二进制 diff 内容。

Admin WebUI 有意不提供恢复、revert、rollback 或写回控制。浏览文件、历史和 diff 会记录 `view_commit`、`view_history` 和 `view_diff` 活动。

## 邀请码和注册

可从 **Settings** 配置注册模式：

- `disabled`：只允许管理员创建账号
- `invite_only`：用户使用邀请码注册
- `open`：任何拥有部署 URL 的人都可以注册

创建邀请码时可以填写未来过期时间。Admin WebUI 使用人类可读日期时间输入，内部仍存储 Unix 秒。已使用邀请码不能通过 admin API 删除，应保留用于审计历史。

只有在短时间窗口或具备额外监控和限流的公开部署中，才建议使用 `open`。

## 运行时设置

设置页编辑保存在 SQLite 中的配置值,改动对新请求立即生效(保存时刷新内存缓存)。

**通用** — 服务名称、默认时区。

**安全** — 注册模式(`disabled` / `invite_only` / `open`)、登录失败阈值、失败窗口和锁定时长。登录速率限制器同时统计已失败次数和在途密码验证,并发暴力尝试无法绕过阈值。

**同步与存储**
- 最大文件大小(默认 `100 MiB`)
- 支持的文本扩展名 — 列表外的文件按二进制 blob 处理
- 额外 exclude glob — 管理员可调,补充内置的 `.obsidian/`、`.trash/`、`.conflict-*`、`.git/` 排除清单
- 历史界面和 diff 端点开关
- **Push 去抖**(`push_debounce_ms`,默认 `250`):本地编辑稳定到推送之间的延迟。变小可缩短端到端延迟,变大可每次 push 合并更多按键
- **SSE 内联内容上限**(`inline_content_max_bytes`,默认 `8192`,上限 `65536`):此尺寸以内的文本变更随 SSE 事件直接下发,接收端插件无需再 pull;超过则降级走 pull
- **SSE 心跳**(`sse_heartbeat_seconds`,默认 `30`):事件流保活,避免空闲 SSE 连接被反向代理切断
- **Git smart HTTP**(`enable_git_smart_http`,默认关):开启后授权设备可 `git clone https://_:<token>@host/git/<vault-id>`。服务器还需要 `PATH` 中有 `git` 二进制;公开的 `/api/config` 能力两个条件都满足才显示为可用

## 活动日志

活动日志记录 push、pull、view_commit、view_history、view_diff 等同步与只读浏览操作，包括：

- 用户
- 笔记库
- 动作
- 设备名
- 文件数
- 字节大小
- 客户端 IP
- User-Agent
- 详情
- 时间戳

使用活动筛选可以检查特定用户或操作类型。

## 分享服务端 URL

分享服务端或 Admin WebUI 提供的 URL：

```text
https://sync.example.com/k_xxx/
```

请把它视为敏感信息。它不是用户密码，但包含部署密钥，是插件 API 流量的第一道预认证入口。

## 维护清单

- 将 `config.toml`、`metadata.db`、`vaults/` 和 `blobs/` 放在同一备份集合中。
- 大量删除附件后运行 blob 垃圾回收。
- 关注日志和活动中重复出现的 `401`、`403`、`404`、`409` 和 `429` 响应。
- 保持服务端二进制、插件包、Docker 镜像、反向代理和主机系统及时更新。
- 打 tag 发版前确认 CI 通过。
- 检查每个 release 都包含 Linux amd64、Linux arm64、Windows x64、插件 zip、校验和和 GHCR Docker 镜像 tag。
