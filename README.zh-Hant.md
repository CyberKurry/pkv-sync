# PKV Sync

**自架你的 Obsidian 筆記庫。** PKV Sync 跑在你自己的伺服器上，讓手機、平板、桌機的 Obsidian 筆記庫保持同步。一個執行檔、一個 SQLite 資料庫、每個筆記庫一個 bare git 儲存庫——不用叢集、不用 S3、也不用任何託管雲端。裝好之後，把 Obsidian 指過去，筆記就跟著同步了。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

文件版本：v1.6.0。

[English](./README.md) · [简体中文](./README.zh-CN.md) · 繁體中文 · [日本語](./README.ja.md) · [한국어](./README.ko.md)

## 特性

- **多使用者、多筆記庫。**透過驗證過的裝置進行同步，每個筆記庫都有推送鎖定（push lock）與冪等重試。
- **即時推送。**小修改透過 Server-Sent Events（SSE）在不到一秒內就定位；輪詢則留作後備機制。
- **Git 就是唯一事實來源。**每個筆記庫都是一個 bare git 儲存庫，因此單檔歷程、統一 diff、單檔還原都開箱即用——外掛端和管理後台都吃得到。
- **衝突不吃掉你的檔案。**外掛絕不默默覆寫本機修改，衝突會以 `.conflict-*` 檔案呈現，一鍵就能選「保留本機」或「採用遠端」。
- **五種語言的管理後台**（English、简中、繁中、日本語、한국어）：管理使用者、裝置權杖（device token）、筆記庫、邀請碼、活動日誌與 blob GC；針對筆記庫和使用者的破壞性操作都會跳出確認對話框。
- **AI 讀得到的筆記庫。**MCP 透過 stdio 或 `pkvsyncd serve` 內建的 `/mcp` 路由（預設開啟）提供讀寫工具。
- **預設從嚴。**管理員建立的密碼遵循與 setup 同等級的強密碼原則，權杖密鑰只顯示一次，上傳與 MCP 回應都有大小上限，即時 SSE 串流也會重新查驗已撤銷的權杖。
- **刻意做得無聊。**單一執行檔、單一 SQLite 中繼資料庫、每庫一個 bare git 儲存庫、每個附件一個內容定址 blob。

## 用 Docker Compose 快速開始

這是建議的做法。`deploy/caddy/` 裡的 Caddy 透過 Let's Encrypt 處理對外的 HTTPS；PKV Sync 只在 compose 內部網路監聽 `127.0.0.1:6710`，來自公網的明文 HTTP 根本碰不到它。

你需要一個網域（例如 `sync.example.com`），把 A/AAAA 記錄指向伺服器，並讓網際網路能連到 `80` 和 `443` 連接埠（ACME HTTP-01 驗證需要用到 80）。

1. 產生部署金鑰（deployment key）：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 在 `docker-compose.yml` 旁邊放一份 `config.toml`：

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 換成 genkey 的輸出
   public_host    = "sync.example.com"   # 必填，管理端 POST 才會動

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge 網段

   [mcp]
   embed_in_serve = true                 # 預設值；設為 false 可停用
   ```

3. 編輯 `deploy/caddy/Caddyfile`，把 `sync.example.com` 換成你實際的網域。

4. 把整套服務啟動起來：

   ```bash
   docker compose up -d
   ```

   在瀏覽器開啟 `https://sync.example.com/setup`，建立第一個管理員帳號。

5. 在 Obsidian 安裝 `pkv-sync-plugin.zip`（解壓縮到 `<vault>/.obsidian/plugins/pkv-sync/`），啟用外掛，貼上管理後台裡的分享 URL，登入或註冊後選一個筆記庫。

之後更新只要 `docker compose pull && docker compose up -d`。原生安裝、反向代理調校（Caddy / Nginx / Traefik）、`public_host` 的語意、備份還原與磁碟加密，請參閱[部署強化指南](./public-docs/deployment-hardening.zh-Hant.md)。

## MCP 部署模式

MCP HTTP 只提供內建方式：`[mcp].embed_in_serve` 預設為 `true`，`pkvsyncd serve` 會在主伺服器埠掛載 `/mcp`，共用同一組 TLS 終結、反向代理、部署金鑰與 bearer 權杖驗證。設成 `embed_in_serve = false` 即可停用。獨立執行的 `pkvsyncd mcp --transport http --bind` 行程已在這個版本移除——既有的獨立部署請改指向主伺服器的 `/mcp`。本機的 AI 工具也可以走 stdio 傳輸：`pkvsyncd mcp --vault <id>`。

## Obsidian 外掛

本機檔案才是唯一事實來源——外掛直接讀寫你磁碟上一般的 Obsidian 筆記庫，沒有代理檔案系統這回事。非敏感的外掛設定與同步索引存在 `<vault>/.obsidian/plugins/pkv-sync/data.json`；登入狀態、目前作用中的 bearer 裝置權杖、部署金鑰與固定的裝置身分，則存在 Obsidian 的裝置本機儲存區。請把 Obsidian 裝置本機儲存區、明文備份，以及舊版留下的外掛 `data.json` 副本都當成機敏資料。裝置權杖在使用時會自動更新，閒置 90 天後失效，單一權杖最長壽命為 365 天；在同一裝置重新登入則會輪替掉舊權杖。

日常功能——命令面板、檔案歷程、並排 diff、衝突處理、`.obsidian` 選擇性同步、裝置管理、外掛自動更新——都寫在[使用者手冊](./public-docs/user-manual.zh-Hant.md)裡。

## 加密現況

PKV Sync 1.0 **尚不**提供原生端對端加密——伺服器看得到筆記內容。原生的每庫 E2EE 已列在 1.x 路線圖上，將以選用模式推出，因為加密會犧牲伺服器端那些讓 Git-native PKV 真正好用的功能（歷程 diff、三方自動合併、SSE 內嵌推送、MCP 讀寫）。

等不到原生 E2EE 又需要端對端加密的話，可以在筆記庫上加一層 [`git-crypt`](https://github.com/AGWA/git-crypt)：標記到的路徑會以密文 blob 的形式送到伺服器，伺服器解不開。檔名在伺服器上仍是明文（對多數威脅模型而言可以接受）。持有金鑰的用戶端照樣能用標準 `git clone` 與 `pkvsyncd materialize`。

正式環境的部署還應該只透過 HTTPS 對外、收緊 `trusted_proxies`、加密資料硬碟與備份——詳見[部署強化指南](./public-docs/deployment-hardening.zh-Hant.md)。

## 你在找……

| 主題 | 文件 |
| --- | --- |
| 外掛日常使用 | [使用者手冊](./public-docs/user-manual.zh-Hant.md) |
| 伺服器管理與執行設定 | [管理員手冊](./public-docs/admin-manual.zh-Hant.md) |
| 所有 CLI 命令與參數 | [CLI 參考](./public-docs/cli-reference.zh-Hant.md) |
| 從 0.x 升級到 1.0 | [1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md) |
| 反向代理、TLS、備份、強化 | [部署強化](./public-docs/deployment-hardening.zh-Hant.md) |
| HTTP API 契約 | [OpenAPI 規範](./public-docs/openapi.yaml) |
| MCP 設定與工具清單 | [MCP 使用指南](./public-docs/mcp-howto.zh-Hant.md) |
| LLM 維運的 Wiki 工作流程 | [LLM Wiki 使用指南](./public-docs/llm-wiki-howto.zh-Hant.md) |
| 從 Obsidian Sync 轉移 | [轉移指南](./public-docs/migrate-from-obsidian-sync.zh-Hant.md) |
| 安全漏洞通報 | [SECURITY.md](./SECURITY.md) |
| 發布歷程 | [CHANGELOG.md](./CHANGELOG.md) |

## 狀態

PKV Sync 1.6.0 讓 MCP 成為一級公民：伺服器同時支援新的 2026-07-28 無狀態協定修訂版與舊用戶端（雙世代），新增 `subscriptions/listen` 推送，並預設將 MCP 內建到主伺服器（獨立的 `mcp --transport http` 部署已移除）。無法設定 `public_host` 的管理部署有了 `allow_insecure_admin` 這條逃生出口，健康探測修正之後，自我升級總算真正生效，不再默默回滾。同步正確性的修正也一併到位：auto-merge 不再漏掉提交（本機頭不再越過從未拉取過的合併基準），即時套用也不再默默覆寫或救回本機檔案。

PKV Sync 1.5.0 交付了 2026-08-16 全儲存庫稽核的完整修正，外加一波效能調校：blob 保留現在跟著 git 物件的存活狀態走，回滾與單檔歷程還原的附件不再遺失；Windows 上有問題的路徑元件在進入 `materialize` 前就會被拒絕；特權更新器拒絕降級，Docker 自我升級覆疊也已強化；註冊與 blob 上傳路徑補上了缺少的速率限制；常駐 SSE 連線不再卡住優雅關機；外掛可同步 allowlist 內的 `.obsidian` 路徑，首次同步也改以記憶體受限的批次串流進行（針對行動裝置）；批次 git 讀取搭配單次複製的索引更新，讓 MCP 搜尋與大型筆記庫的同步都更快。

PKV Sync 1.0 是第一個穩定版。公開 REST API、CLI、儲存布局、外掛套件、Docker 映像檔依同一套 semver 一起發版：1.X.Y 在公開介面上維持向下相容，OpenAPI 規範是這份相容契約的權威依據。0.x 建立的 SQLite 資料庫**無法**就地升級到 1.0.0——請依照[1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md)操作。

每次 GitHub Release 都會發布 Linux amd64/arm64 二進位檔、Windows x64 二進位檔、多架構 GHCR Docker 映像檔、Obsidian 外掛 zip 包，以及 `SHA256SUMS`。

## 開發

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

cd plugin
npm run typecheck
npm exec vitest run
npm run build
```

CI 會在 Linux 與 Windows 上跑完整 Rust 矩陣，加上外掛的測試、型別檢查、建置與打包、Docker 建置，以及發布二進位檔的煙霧測試。

## 授權條款

AGPL-3.0-only。詳見 [LICENSE](./LICENSE)。
