# PKV Sync

**自架你的 Obsidian 筆記庫。** PKV Sync 跑在你自己的伺服器上，把手機、平板、桌機的 Obsidian 筆記庫保持同步。一份二進位、一個 SQLite 資料庫、每個筆記庫一個 bare git 倉庫——不用叢集、不用 S3、不用任何託管雲。裝好，把 Obsidian 指過去，筆記就同步了。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

文件版本：v1.0.13。

[English](./README.md) | [简体中文](./README.zh-CN.md) | 繁體中文 | [日本語](./README.ja.md) | [한국어](./README.ko.md)

## 特性

- **多使用者、多筆記庫**同步，依裝置簽發 token，每個筆記庫帶 push 鎖與冪等重試。
- **即時推送**。小修改透過 SSE 在亞秒級落地；輪詢做為兜底保險。
- **Git 即真相**。每個筆記庫都是一個 bare git 倉庫，單檔歷史、unified diff、單檔還原開箱即用——外掛端和管理後台都能用。
- **衝突安全**。外掛不會默默覆蓋本地修改，衝突會以 `.conflict-*` 檔案呈現，一鍵「保留本地」或「採納遠端」。
- **五語言管理後台**（English、简中、繁中、日本語、한국어）：使用者、裝置 token、筆記庫、邀請碼、活動日誌、blob 垃圾回收，並對破壞性的筆記庫和使用者操作彈出確認。
- **AI 可讀**。MCP 可透過 stdio、獨立 Streamable HTTP，或 `pkvsyncd serve` 內嵌的 `/mcp` 路由暴露讀寫工具。
- **預設有邊界**。管理員建立／重設密碼使用 setup 同級強密碼策略；token 明文只展示一次；上傳和 MCP 回應都有大小上限；即時 SSE 串流會複查已撤銷 token。
- **刻意做得無聊**。單一二進位、單一 SQLite 中繼資料庫、每庫一個 bare git 倉、每個附件一個內容定址 blob。

## 用 Docker Compose 快速上手

這是推薦路徑。`deploy/caddy/` 裡的 Caddy 透過 Let's Encrypt 自動簽發 HTTPS；PKV Sync 在 compose 內網監聽 `127.0.0.1:6710`，公網完全看不到明文 HTTP。

你需要：一個網域（例如 `sync.example.com`），A／AAAA 記錄指向伺服器；公網能連到 `80` 和 `443` 連接埠（80 用於 ACME HTTP-01 驗證）。

1. 產生部署金鑰：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 在 `docker-compose.yml` 旁放一份 `config.toml`：

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 替換為 genkey 輸出
   public_host    = "sync.example.com"   # 必填，管理端 POST 才能通

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge 網段

   [mcp]
   embed_in_serve = false                # true 會在本服務上掛載 /mcp
   ```

3. 編輯 `deploy/caddy/Caddyfile`，把 `sync.example.com` 換成你的真實網域。

4. 把整套服務拉起來：

   ```bash
   docker compose up -d
   ```

   瀏覽器打開 `https://sync.example.com/setup`，建立第一個管理員帳號。

5. 在 Obsidian 裡把 `pkv-sync-plugin.zip` 解壓到 `<vault>/.obsidian/plugins/pkv-sync/`，啟用外掛，從管理後台複製分享 URL 貼進去，登入或註冊，選一個筆記庫。

之後升級就是 `docker compose pull && docker compose up -d`。如果要原生安裝、調反向代理（Caddy／Nginx／Traefik）、了解 `public_host` 的語義、做備份還原或磁碟加密，請看[部署加固指南](./public-docs/deployment-hardening.zh-Hant.md)。

## MCP 部署模式

PKV Sync 提供兩種 MCP Streamable HTTP 部署方式。內嵌模式需要明確開啟：設定 `[mcp].embed_in_serve = true` 後，`pkvsyncd serve` 會在主服務端口掛載 `/mcp`，復用同一套 TLS 終止、反向代理、部署金鑰和 bearer 權杖校驗。獨立模式保留原有單獨進程：`pkvsyncd mcp --transport http --bind 127.0.0.1:6711`，適合隔離 MCP、專用監聽位址或獨立擴縮容。

## Obsidian 外掛

本地檔案就是真相——外掛直接讀寫你磁碟上的 Obsidian 筆記庫，不存在代理檔案系統那種東西。外掛設定和當前的裝置 token 都存在 `<vault>/.obsidian/plugins/pkv-sync/data.json`，請把這個檔案當成敏感資料。裝置 token 在使用時會自動續期，90 天無活動後失效，且單個 token 最長有效 365 天；在同一裝置重新登入會換掉舊 token。

日常使用——命令面板、檔案歷史、並排 diff、衝突解決、`.obsidian` 選擇性同步、裝置管理、外掛自更新——都寫在[使用者手冊](./public-docs/user-manual.zh-Hant.md)裡。

## 關於加密

PKV Sync 1.0 **暫不**提供原生端到端加密——伺服器能讀到筆記內容。原生的按庫 E2EE 在 1.x 路線圖上，會以可選模式上線，因為加密會換掉伺服器那些讓 Git-native PKV 真正有用的功能（歷史 diff、三方自動合併、SSE 內嵌推送、MCP 讀寫）。

在原生 E2EE 落地前，如果你需要端到端加密，可以在筆記庫上疊一層 [`git-crypt`](https://github.com/AGWA/git-crypt)：被標記的路徑會以密文 blob 形式到達伺服器，伺服器無法解密。檔名仍以明文形式存在於伺服器（對大多數威脅模型來說可接受）。持有金鑰的客戶端依然可以用標準 `git clone` 和 `pkvsyncd materialize`。

正式部署還應該跑在 HTTPS 後面、把 `trusted_proxies` 收緊、給資料碟加密、給備份加密——具體看[部署加固指南](./public-docs/deployment-hardening.zh-Hant.md)。

## 你在找……

| 主題 | 文件 |
| --- | --- |
| 外掛日常使用 | [使用者手冊](./public-docs/user-manual.zh-Hant.md) |
| 伺服器管理與執行時設定 | [管理員手冊](./public-docs/admin-manual.zh-Hant.md) |
| 所有 CLI 命令與參數 | [CLI 參考](./public-docs/cli-reference.zh-Hant.md) |
| 從 0.x 升級到 1.0 | [1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md) |
| 反向代理、TLS、備份、加固 | [部署加固](./public-docs/deployment-hardening.zh-Hant.md) |
| HTTP API 契約 | [OpenAPI 規範](./public-docs/openapi.yaml) |
| MCP 安裝與工具列表 | [MCP 操作指南](./public-docs/mcp-howto.zh-Hant.md) |
| 從 Obsidian Sync 遷移 | [遷移指南](./public-docs/migrate-from-obsidian-sync.zh-Hant.md) |
| 安全漏洞通報 | [SECURITY.md](./SECURITY.md) |
| 發布紀錄 | [CHANGELOG.md](./CHANGELOG.md) |

## 狀態

PKV Sync 1.0.13 是目前穩定 patch 版本。本版包含最新安全與效能實作：註冊和修改密碼使用 setup 同級強密碼策略，過濾路徑不會從讀取/歷史/diff API 洩露，rollback 與 pull 工作量有邊界，backup/restore helper 不跟隨 symlink，blob 引用中繼資料修復更快。

PKV Sync 1.0 是第一個穩定版。公開 REST API、CLI、儲存布局、外掛包、Docker 映像作為一組同步發版，遵循 semver：1.X.Y 在公開表面保持向後相容，OpenAPI 規範是這個相容契約的權威來源。0.x 建立的 SQLite 資料庫**不支援**就地升級到 1.0.0——請依[1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md)操作。

每個 GitHub release 會發布 Linux amd64／arm64 二進位、Windows x64 二進位、多架構 GHCR Docker 映像、Obsidian 外掛 zip 包，以及 `SHA256SUMS`。

## 開發自檢

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 在 Linux 和 Windows 上跑完整 Rust 矩陣，加上外掛的 test／typecheck／build／package、Docker 構建，以及發布二進位的冒煙測試。

## 授權條款

AGPL-3.0-only。詳見 [LICENSE](./LICENSE)。
