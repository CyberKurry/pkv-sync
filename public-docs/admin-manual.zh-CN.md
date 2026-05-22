# PKV Sync 管理员手册

[English](./admin-manual.md) | 简体中文 | [繁體中文](./admin-manual.zh-Hant.md) | [日本語](./admin-manual.ja.md) | [한국어](./admin-manual.ko.md)

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

5. 全新数据库首次启动后，在浏览器打开 `/setup`，创建第一个管理员账号。PKV Sync 不再把随机管理员密码输出到 stderr 或容器日志。
6. setup 完成后，日常管理员登录使用 `/admin/login`。

已发布后的 migration 应保持追加式管理。对于已有部署，不要压缩或编辑已经发布的 migration 文件。

## Admin Web 面板

打开：

```text
https://sync.example.com/admin/login
```

管理后台包含：

- 仪表盘：系统、存储、笔记库、用户、最近活动指标，以及有新版 PKV Sync 时的更新提示
- 用户列表，支持搜索和状态筛选
- 用户详情页：重置密码、启用/禁用、管理员权限控制和 token 查看
- 全局设备 token 页面，可列出、创建和撤销 token
- 笔记库卡片：所有者、文件数、大小、上次同步、元数据修复、删除操作和按笔记库同步设置
- 只读笔记库文件浏览器，支持文件预览、单文件历史时间线和 unified diff 渲染
- 邀请码创建，可选过期时间，活跃邀请码列表，以及删除未使用邀请码
- 运行时设置，分为 General、Security、Sync & Storage、Network
- 活动日志，支持按用户和动作真实筛选 push/pull 以及笔记库生命周期记录
- Blob 垃圾回收触发
- 英文和简体中文语言切换

时间戳、持续时间、字节大小、运行时长和活动数据都会以人类可读形式显示。默认时区是 `Asia/Shanghai`，可在设置中修改。

## 更新通知

PKV Sync 默认每 24 小时检查一次 GitHub release。发现新的服务端版本时，仪表盘会显示提示，包含当前版本、最新版本、发行说明链接和简短摘要。

离线部署可以在静态配置中关闭检查：

```toml
[update_check]
enabled = false
```

更新检查只提供信息。PKV Sync 不会自动替换正在运行的服务端二进制或容器镜像。

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

设备 bearer token 会在认证请求时续期，连续 90 天未使用才会过期。用户可以撤销自己的 token，管理员可以撤销任意用户的 token。

运维注意事项：

- Token 明文只在创建时展示一次。
- 数据库只保存 SHA-256 token hash。
- 每次认证请求都会把 token 过期时间延长到该请求时间之后 90 天，并且不会缩短更晚的过期时间。
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

### 按笔记库同步设置

在 **Vaults** 页面点击某个笔记库卡片上的 **Settings**，可以编辑该笔记库的 `extra_sync_globs` allowlist。它控制哪些隐藏路径，包括选定的 `.obsidian` 配置文件，可以参与同步。

新笔记库会自动获得推荐起步 allowlist。已有笔记库保持空配置，直到管理员或笔记库所有者应用起步清单。**Apply starter allowlist** 会写入推荐清单，包括主题、CSS snippets、快捷键、应用偏好、外观偏好和已启用插件列表。

### 只读文件历史

在 **Vaults** 页面点击某个笔记库卡片上的 **Browse files**。文件浏览器会列出当前 HEAD 中的文件、大小以及文本/二进制类型。打开文件后，文本文件会显示只读预览，并提供 **History** 和 **Diff with previous** 链接。

历史页会列出该文件相关的提交，并提供“查看该提交时的文件”和对应 diff 的链接。diff 页会按行渲染 unified diff，并用颜色区分新增、删除和 hunk。二进制文件只显示元数据，不渲染二进制 diff 内容。

浏览文件、历史和 diff 会记录 `view_commit`、`view_history` 和 `view_diff` 活动。Admin history 中提供笔记库 rollback 控制；请在确认目标提交后再使用，因为 rollback 会从选定历史点创建新的笔记库状态。

## 邀请码和注册

可从 **Settings** 配置注册模式：

- `disabled`：只允许管理员创建账号
- `invite_only`：用户使用邀请码注册
- `open`：任何拥有部署 URL 的人都可以注册

创建邀请码时可以填写未来过期时间。Admin WebUI 使用人类可读日期时间输入，内部仍存储 Unix 秒。已使用邀请码不能通过 admin API 删除，应保留用于审计历史。

只有在短时间窗口或具备额外监控和限流的公开部署中，才建议使用 `open`。

## 运行时设置

设置页编辑保存在 SQLite 中的配置值,改动对新请求立即生效(保存时刷新内存缓存)。

**通用** — 服务名称、默认时区、`enable_metrics` 指标开关。开启后 `/metrics` 可用，但仍需要部署密钥中间件、插件 User-Agent guard 和管理员 bearer token。

**安全** — 注册模式(`disabled` / `invite_only` / `open`)、登录失败阈值、失败窗口和锁定时长。登录速率限制器同时统计已失败次数和在途密码验证,并发暴力尝试无法绕过阈值。认证同步 API 路由另有固定窗口限流：按路由、方法、客户端 IP 和 bearer 设备 token 分桶，每 60 秒最多 600 次请求。

**同步与存储**
- 最大文件大小(默认 `100 MiB`)
- 支持的文本扩展名 — 列表外的文件按二进制 blob 处理
- 额外 exclude glob — 管理员可调,补充内置的 `.obsidian/`、`.trash/`、`.conflict-*`、`.git/` 排除清单
- 历史界面和 diff 端点开关
- **Push 去抖**(`push_debounce_ms`,默认 `250`):本地编辑稳定到推送之间的延迟。变小可缩短端到端延迟,变大可每次 push 合并更多按键
- **SSE 内联内容上限**(`inline_content_max_bytes`,默认 `8192`,上限 `65536`):此尺寸以内的文本变更随 SSE 事件直接下发,接收端插件无需再 pull;超过则降级走 pull
- **SSE 心跳**(`sse_heartbeat_seconds`,默认 `30`):事件流保活,避免空闲 SSE 连接被反向代理切断。并发 SSE 订阅默认按用户限制为 16，并保留 1024 的全局上限。
- **Git smart HTTP**(`enable_git_smart_http`,默认关):开启后授权设备可 `git clone https://_:<token>@host/git/<vault-id>`。服务器还需要 `PATH` 中有 `git` 二进制;公开的 `/api/config` 能力两个条件都满足才显示为可用

## 活动日志

活动日志记录 push、pull、create_vault、delete_vault、view_commit、view_history、view_diff 等同步、笔记库生命周期与只读浏览操作，包括：

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

`create_vault` 和 `delete_vault` 来自管理面板、插件和 API 的笔记库创建／删除操作。

## 分享服务端 URL

分享服务端或 Admin WebUI 提供的 URL：

```text
https://sync.example.com/k_xxx/
```

请把它视为敏感信息。它不是用户密码，但包含部署密钥，是插件 API 流量的第一道预认证入口。

## 升级 PKV Sync

二进制部署可先运行 `pkvsyncd upgrade --dry-run` 预览最新 release、目标资产和旁路写入路径。运行 `pkvsyncd upgrade --yes` 会把校验后的 release 二进制下载到当前可执行文件旁边的 `pkvsyncd.new`（Windows 为 `pkvsyncd.new.exe`）。命令会根据 `SHA256SUMS` 校验 SHA-256，并打印 systemd／手动替换步骤；它不会热替换正在运行的进程。

使用 `pkvsyncd upgrade --version 0.9.1` 可以指定 release。若命令找不到匹配资产或校验和，请手动从 GitHub release 下载，并自行校验 `SHA256SUMS`。

Docker 和 Kubernetes 部署应通过拉取或修改容器镜像 tag 升级，然后重启服务或 rollout。upgrade CLI 检测到容器环境时，会输出镜像升级指引，不写入旁路二进制。

## 维护清单

- 使用 `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` 生成运维快照。输出目录必须不存在或为空；命令会用 `VACUUM INTO` 快照 SQLite，复制 `vaults/`、`blobs/` 和存在时的 `config.toml`，并写入带 pkvsyncd 版本、组件哈希、大小和数量的 `MANIFEST.json`。
- 使用 `pkvsyncd restore --input <backup-dir> --data-dir <dir>` 恢复到不存在或为空的数据目录。只有确认目标可以先清空时才加 `--force`；恢复会先校验 manifest 哈希，复制完成后自动运行 verify。
- 维护后或主机存储异常后运行 `pkvsyncd verify [--data-dir <dir>]`。它会检查被引用的 blob 文件，报告孤立 blob，用 `git2` 校验笔记库 git 仓库，并在缺失、损坏或 git 错误时返回失败。`--no-fail` 会保留报告但强制返回成功退出码。
- 大量删除附件后运行 blob 垃圾回收。
- 维护前检查仪表盘更新提示或 GitHub release。
- 关注日志和活动中重复出现的 `401`、`403`、`404`、`409` 和 `429` 响应。
- 保持服务端二进制、插件包、Docker 镜像、反向代理和主机系统及时更新。
- 打 tag 发版前确认 CI 通过。
- 检查每个 release 都包含 Linux amd64、Linux arm64、Windows x64、插件 zip、校验和和 GHCR Docker 镜像 tag。
