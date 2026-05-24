# PKV Sync

自託管 Obsidian 筆記庫同步：Rust 服務端、SQLite 中繼資料、Git 文字歷史、內容定址附件儲存，以及桌面／行動端 Obsidian 外掛。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | [简体中文](./README.zh-CN.md) | 繁體中文 | [日本語](./README.ja.md) | [한국어](./README.ko.md)

## 狀態

PKV Sync 1.0 是第一個穩定版。公開 REST API、CLI 表面、儲存布局、外掛包、Docker 映像和公開文件會一起按版本維護。

PKV Sync **暫未提供**原生端到端加密。服務端可以讀取同步的筆記內容和附件。原生 per-vault E2EE 計畫作為 1.x 路線圖項目落地，並以「按 vault 可選啟用的隱私模式」形式發布，而非全域預設。需要今天就做內容側加密的使用者，可以參考 [`git-crypt`](./public-docs/git-crypt-howto.zh-Hant.md) 過渡方案；路徑和檔名仍會是明文。

## 穩定性與版本

PKV Sync 從 v1.0.0 起遵循語義化版本：

- **Major（X.0.0）**：可能包含對公開 HTTP API、儲存布局或 CLI 表面的不相容變更，遷移說明會寫入 `public-docs/upgrade-notes-vX.0.md`。
- **Minor（1.X.0）**：向後相容的功能新增。已有 endpoint、CLI 參數和儲存格式繼續可用。
- **Patch（1.0.X）**：bug 修復和安全補丁。不破壞公開 API、儲存、CLI 或外掛相容性。

公開 REST API 契約是 [`public-docs/openapi.yaml`](./public-docs/openapi.yaml)。Admin Web UI 表單處理器以及沒有列在其中的其他路由屬於內部實作細節。MCP 行為記錄在 [`public-docs/mcp-howto.zh-Hant.md`](./public-docs/mcp-howto.zh-Hant.md)。

PKV Sync 1.0 有意重置 SQLite migration 基線。全新的 1.x 資料庫從 `server/migrations/0001_initial.sql` 開始，之後 1.x migration 保持追加式。由 0.x 建立的 SQLite 資料庫**不支援原地升級**到 1.0.0；請按 [`public-docs/upgrade-notes-v1.0.zh-Hant.md`](./public-docs/upgrade-notes-v1.0.zh-Hant.md) 操作。

安全披露流程見 [`SECURITY.zh-Hant.md`](./SECURITY.zh-Hant.md)。

## 亮點

- **多使用者、多筆記庫** Obsidian 同步，帶按筆記庫 push 鎖和冪等 push。
- **即時推送**：透過 Server-Sent Events 投遞 commit 事件，小文字變更（預設 ≤ 8 KiB）直接內嵌在事件裡，外掛無需再 pull。
- **Git 原生**：每個筆記庫在磁碟上就是一個 bare git repository。檔案歷史、unified diff、單檔恢復和可選只讀 `git clone` 都可用。
- **AI 可讀寫筆記庫**：`pkvsyncd mcp` 透過 stdio 或 Streamable HTTP 暴露 MCP 讀寫工具。
- **選擇性 `.obsidian` 同步**：新筆記庫預設帶起步 allowlist；外掛程式碼和外掛設定仍需使用者主動 opt-in。
- **衝突安全**：SSE inline apply 不覆蓋本地未同步修改；衝突會落盤為 `.conflict-*` 檔案，並可從外掛命令面板解決。
- **Admin Web UI**：使用者、裝置 token、筆記庫、邀請碼、執行時設定、活動日誌、blob 垃圾回收和更新提示。
- **安全基線**：Argon2id 密碼雜湊、登入速率限制、嚴格 CSRF、bearer 裝置 token 使用時續期、同裝置重新登入會替換舊 token。
- Linux amd64／arm64、Windows x64 二進位，以及多架構 GHCR Docker 映像。

完整操作請看 [管理員手冊](./public-docs/admin-manual.zh-Hant.md)、[使用者手冊](./public-docs/user-manual.zh-Hant.md) 和 [部署加固指南](./public-docs/deployment-hardening.zh-Hant.md)。

## 儲存布局

```text
data_dir/
  metadata.db        SQLite 中繼資料
  vaults/<vault-id>/ 每個遠端筆記庫的 bare Git repository
  blobs/<sha256>     內容定址二進位 blob
```

`metadata.db` 儲存使用者、筆記庫、裝置 token、邀請碼、執行時設定、同步活動、blob 引用和冪等記錄。Git 歷史是版本化檔案狀態的事實來源；blob 檔案在被引用期間保留，過寬限期後由 GC 清理。請用 `pkvsyncd backup` 快照資料根目錄和對應的 `config.toml`。

## 發布資產

GitHub Release 提供 Linux amd64／arm64、Windows x64、`pkv-sync-plugin.zip` 和 `SHA256SUMS`。Docker 映像發布到 GHCR：

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v1.0.0
```

## 快速開始：Docker Compose

推薦使用 `docker-compose.yml` 搭配 `deploy/caddy/`。Caddy 申請並續簽 HTTPS 憑證；PKV Sync 在 compose 網路內監聽 `127.0.0.1:6710`。

1. 將 DNS 指向伺服器，例如 `sync.example.com`。
2. 執行 `docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey` 產生部署金鑰。
3. 在 `config.toml` 設定 `deployment_key`、`public_host = "sync.example.com"`、`data_dir` 和 `db_path`。
4. 更新 `deploy/caddy/Caddyfile`，再執行 `docker compose up -d`。
5. 全新資料庫首次啟動後，開啟 `https://sync.example.com/setup` 建立第一個管理員。
6. 進入 Admin Web UI 建立使用者和筆記庫，安裝 `pkv-sync-plugin.zip`，再把分享 URL 貼到 Obsidian 外掛。

`public_host` 是生產部署必備設定。未設定時，admin POST 會因 CSRF fail-closed 被拒絕。

## 升級

1.x 部署的資料庫 migration 會在啟動時自動套用，並在 v1 基線之後保持追加式。0.x SQLite 資料庫不能原地升級到 1.0.0；請先閱讀 [1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md)。

二進位部署可以使用：

```bash
pkvsyncd upgrade [--dry-run] [--yes] [--version 1.0.0]
```

Docker 和 Kubernetes 部署應拉取或修改映像 tag，而不是在容器內替換二進位。

## 服務端 CLI

常用命令：

```bash
pkvsyncd genkey
pkvsyncd -c /etc/pkv-sync/config.toml migrate up
pkvsyncd -c /etc/pkv-sync/config.toml serve
pkvsyncd -c /etc/pkv-sync/config.toml user add alice [--admin]
pkvsyncd -c /etc/pkv-sync/config.toml materialize <vault-id> --output <dir>
pkvsyncd -c /etc/pkv-sync/config.toml backup --output <dir> [--data-dir <dir>] [--gzip]
pkvsyncd -c /etc/pkv-sync/config.toml restore --input <backup-dir> --data-dir <dir> [--force]
pkvsyncd -c /etc/pkv-sync/config.toml verify [--data-dir <dir>] [--no-fail]
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

HTTP MCP 模式要求每個 `/mcp` 請求同時帶 bearer token 和 `X-PKVSync-Deployment-Key`。

## Obsidian 外掛

將 `pkv-sync-plugin.zip` 解壓到 `<vault>/.obsidian/plugins/pkv-sync/`，在 Obsidian 啟用 **PKV Sync**，貼上 Admin Web UI 的分享 URL，登入或註冊並選擇遠端筆記庫。

外掛直接讀寫本地 Obsidian vault。`<vault>/.obsidian/plugins/pkv-sync/data.json` 包含 bearer 裝置 token 和部署金鑰，請視為敏感檔案。外掛設定頁支援從連線中的 PKV Sync server 檢查並下載捆綁外掛更新，下載後會驗證 SHA-256。

## HTTP API

所有 `/api/*` 路由都要求部署金鑰 header；認證路由還需要 bearer 裝置 token。認證同步 API 固定視窗限流，SSE 支援 `Last-Event-ID` replay 並有使用者級和全域訂閱上限。完整契約見 [`public-docs/openapi.yaml`](./public-docs/openapi.yaml)。

`/metrics` 預設關閉。啟用後仍需要部署金鑰、PKV Sync User-Agent 和管理員 bearer token。

## 文件

- [部署加固](./public-docs/deployment-hardening.zh-Hant.md)
- [管理員手冊](./public-docs/admin-manual.zh-Hant.md)
- [使用者手冊](./public-docs/user-manual.zh-Hant.md)
- [1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md)
- [安全政策](./SECURITY.zh-Hant.md)
- [OpenAPI 規範](./public-docs/openapi.yaml)
- [Changelog](./CHANGELOG.md)

## 授權

AGPL-3.0-only。詳見 [LICENSE](./LICENSE)。
