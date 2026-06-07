# PKV Sync 部署加固指南

[English](./deployment-hardening.md) | [简体中文](./deployment-hardening.zh-CN.md) | 繁體中文 | [日本語](./deployment-hardening.ja.md) | [한국어](./deployment-hardening.ko.md)

文件版本：v1.0.14。

本文假設部署對象是自己、家庭、團隊或可信朋友使用的小型自託管服務。PKV Sync 運維上比較簡單，但服務端會保存可讀的倉庫內容，因此主機和備份衛生很重要。

## 威脅模型

PKV Sync 不提供端到端加密。保護倉庫內容依賴多層控制：

1. HTTPS 傳輸加密
2. 部署金鑰預認證
3. 使用者名稱/密碼登入和使用時續期的 bearer 裝置 token
4. 按使用者和筆記庫執行授權檢查
5. Admin session 和 CSRF 保護
6. 作業系統或雲端供應商磁碟加密
7. 最小化暴露服務
8. 加密且經過恢復測試的備份

請把服務端管理員和服務端檔案系統視為可以存取倉庫明文內容的可信邊界。

## 推薦拓撲

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

除非你有明確的額外網路控制層，否則不要把 `pkvsyncd` 直接暴露到公網。

## 安裝前準備

準備：

- 網域名稱，例如 `sync.example.com`
- 透過 `pkvsyncd genkey` 產生的部署金鑰
- `/etc/pkv-sync/config.toml`
- 持久化資料目錄，通常是 `/var/lib/pkv-sync`
- 帶有有效 TLS 憑證的反向代理

服務端分享 URL 形式如下：

```text
https://sync.example.com/k_xxx/
```

請保持私密。部署金鑰是 API 流量的預認證入口，但不能取代使用者密碼。

## 系統使用者

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

將 `config.toml` 放在 `/etc/pkv-sync/config.toml`，並確保只有服務使用者和管理員可以讀取。

## 防火牆

典型主機只暴露 SSH 和 HTTPS：

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

如果 Caddy 或其他 ACME HTTP-01 用戶端管理憑證，還需要開放 `80` 連接埠用於驗證和跳轉流量：

```bash
sudo ufw allow 80/tcp
```

在宿主機直接執行時，讓 `pkvsyncd` 只監聽本機：

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose 中讓應用監聽容器所有介面；如果需要宿主機偵錯，只把宿主機連接埠發布到 localhost：

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose + Caddy

如果希望用 Caddy 自動申請和續期 HTTPS 憑證，使用這個路徑。

1. 將 DNS 指向伺服器：

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. 在 `docker-compose.yml` 同目錄建立 `config.toml`：

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 替換為 genkey 輸出
   public_host = "sync.example.com"

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]

   [logging]
   level = "info"
   format = "json"
   ```

3. 替換 `deploy/caddy/Caddyfile` 中的 `sync.example.com`。
4. 啟動：

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 全新資料庫首次啟動後，開啟 setup wizard 建立第一個管理員帳號：

   ```text
   https://sync.example.com/setup
   ```

   如條件允許，請把 setup 階段放在私有網路或臨時反向代理 allowlist 後完成，完成後立刻收緊公開存取。日常管理員登入使用 `https://sync.example.com/admin/login`。

備份 `./data`、`config.toml` 和 Caddy 的命名卷。

升級：

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

儀表板每 24 小時檢查一次 GitHub releases，發現較新的 PKV Sync 版本時會顯示橫幅。全新資料庫首次啟動時，`enabled` 和 `interval_seconds` 會寫入執行階段設定；之後可在 Admin WebUI Settings 中修改，無需重啟。來源倉庫仍保留為靜態 `config.toml` 欄位，供離線鏡像部署使用：

```toml
[update_check]
enabled = true                          # 僅作為首次啟動種子
interval_seconds = 86400                # 僅作為首次啟動種子
repo = "cyberkurry/pkv-sync"            # 靜態查詢的 GitHub 倉庫
```

若要讓離線主機在初始化後保持安靜，請在 Admin WebUI 執行階段設定中關閉更新檢查，或用 `enabled = false` 作為全新部署的初始種子。

## public_host（admin POST 必備）

將 `[server].public_host` 設定為運維實際存取 admin 面板使用的外部主機名稱（不含協定，必要時含連接埠），例如 `sync.example.com` 或 `pkv.local:8443`。admin CSRF 檢查依據該值計算期望 Origin。設定 `public_host` 後，期望 Origin 固定為 `https://<public_host>`；反向代理傳入的 `X-Forwarded-Proto` 不會把 admin CSRF 校驗降級到後端 HTTP。

如果 `public_host` 留空，所有 admin POST 都會被拒絕，返回 `403 csrf validation failed`，並打一條 `tracing::warn` 日誌。這是有意的 fail-closed 行為：另一種做法是回退請求自帶的 `Host` header，但會把鑑權耦合到攻擊者可影響的 header，且在代理轉發不一致的 Host 時會出錯。

`public_host` 同時驅動：

- 生產風格的 admin cookie（設定後啟用 `Secure`、`SameSite=Strict`）
- admin 中「share server URL」連結使用 `https://` 前綴
- `/api/plugin-manifest` 返回的外掛資源 URL 使用 `https://` 外部主機

外掛清單 URL 產生不會信任用戶端傳入的 `X-Forwarded-Proto`。生產環境請設定 `public_host`，這樣外掛自更新拿到的資源 URL 才會穩定指向真實外部主機。

對 SSE 來說，該設定也能幫助反向代理識別長連線事件流而不是普通短請求。

## 安全回應標頭

PKV Sync 會在生產服務端棧中加入這些回應標頭：

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- 在設定了 `public_host` 時加入 `Strict-Transport-Security: max-age=31536000; includeSubDomains`

請讓 TLS 終止和 `public_host` 保持一致。只有當服務端被設定為 HTTPS 對外發布時，才會發送 HSTS。

### 關於端到端加密

PKV Sync 1.0 不提供端到端加密：伺服器系統管理員以及任何可存取伺服器檔案系統的人都能讀取已同步的筆記庫內容。原生的按筆記庫 E2EE 已列入 1.x 規劃。今天就需要對伺服器保密的維運者，可依 [`git-crypt-howto.md`](./git-crypt-howto.md) 套用按筆記庫的過渡性加密層。該模式下檔名仍對伺服器可見，只有檔案內容會在用戶端加密。

## 反向代理注意事項

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

倉庫提供了 `deploy/nginx/pkv-sync.conf`。它會把 HTTP 跳轉到 HTTPS，設定 `client_max_body_size 110m`，加入標準瀏覽器加固 header，並轉發 PKV Sync 用於 Host 和用戶端 IP 處理的 header。

最小結構：

```nginx
server {
  listen 80;
  server_name sync.example.com;
  return 301 https://$host$request_uri;
}

server {
  listen 443 ssl http2;
  server_name sync.example.com;

  ssl_certificate /etc/letsencrypt/live/sync.example.com/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/sync.example.com/privkey.pem;

  client_max_body_size 110m;

  add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
  add_header X-Content-Type-Options "nosniff" always;
  add_header X-Frame-Options "DENY" always;
  add_header Referrer-Policy "same-origin" always;

  location / {
    proxy_pass http://127.0.0.1:6710;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
  }
}
```

### Traefik

倉庫在 `deploy/traefik/docker-compose.traefik.yml` 提供了 Traefik 範例。請將 `trusted_proxies` 設定為 Traefik 使用的 Docker 網路 CIDR，並替換範例網域和 ACME 電子郵件。

## trusted_proxies

只信任來自反向代理的 `X-Forwarded-For`。如果代理和應用執行在同一台主機：

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

如果使用 Docker bridge 網路：

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

不要加入寬泛公網網段。如果用戶端可以偽造 `X-Forwarded-For`，限流和稽核資料都會變弱。

## 執行階段安全設定

從 Admin WebUI 檢查這些設定：

- 註冊模式：私有部署建議保持 `disabled` 或 `invite_only`。
- 登入限流閾值、視窗和鎖定時長。
- 最大檔案大小，預設 `100 MiB`。
- 支援的文字副檔名。
- 時區，預設 `Asia/Shanghai`。

註冊和登入失敗會被限流。Setup、公開註冊、使用者自助修改密碼，以及管理員建立或重設的密碼都必須至少 12 個字元，並包含大寫字母、小寫字母和數字；CLI 建立的使用者也仍應使用強密碼。

認證同步 API 路由也按路由、方法、用戶端 IP 和 bearer token 固定視窗限流，每 60 秒最多 600 次請求。失敗的 bearer token 認證會另按用戶端 IP 限流，每 60 秒最多 120 次失敗嘗試。保持 `trusted_proxies` 準確，讓限流器和稽核日誌看到真實用戶端 IP。

Blob 上傳請求 body 受 `max_file_size` 限制，並且一律會被硬 blob 上限限制（生產環境 `512 MiB`）。主 SSE 串流在保持開啟時會複查 bearer token；MCP 讀取和搜尋工具也有回應大小與總搜尋預算，避免大型筆記庫被展開成無界 JSON 回應。

Pull/tree 遍歷和 rollback 可達性檢查都有邊界；被目前同步過濾規則拒絕的路徑會從讀取、歷史、diff 和 commit-list 介面隱藏。

## Prometheus Metrics

`/metrics` 預設停用。當 `enable_metrics` 執行階段設定為 true 時，端點會返回 Prometheus text exposition，並且仍需要每個生產閘門：部署金鑰中介軟體、外掛 User-Agent guard 和管理員 bearer token。

設定 scrape 用戶端傳送 `X-PKVSync-Deployment-Key`、接受的 PKV Sync User-Agent，以及 `Authorization: Bearer <admin-token>`。不要把 metrics 暴露給未認證網路。

## 備份

一起備份：

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

複製資料庫時使用 SQLite 線上備份，或先停止服務。盡量讓資料庫、Git 筆記庫和 blobs 來自同一時間點。

內建 backup/restore helper 不會跟隨 symlink。`vaults/` 或 `blobs/` 下的 symlink 條目會在備份時跳過，在恢復清理時只移除連結本身，不會觸碰連結目標。

restic 範例：

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

備份離開機器前應先加密，並定期測試恢復。

## 磁碟加密

盡量使用 LUKS、BitLocker、FileVault 或雲端供應商託管磁碟加密。如果 VPS 供應商無法加密根磁碟，加密離線備份就不是可選項，而是必要項。

## Token 衛生

裝置 bearer token 會在認證使用時續期，連續 90 天未使用才會過期，單個 token 最長有效 365 天，也可以由使用者或管理員撤銷。在過期或撤銷前，請把活躍 token 當作憑證處理。

Obsidian 會把外掛的活躍 token 和部署金鑰保存於筆記庫本機外掛資料檔案 `<vault>/.obsidian/plugins/pkv-sync/data.json` 中。請提醒使用者不要把該檔案放進分享壓縮包、不可信同步目標或明文備份裡。如果懷疑檔案已經洩露，請撤銷受影響的裝置 token。

建議：

- 從 Admin WebUI 裝置頁面撤銷遺失裝置。
- 如果只遺失單台裝置，優先撤銷該裝置 token，而不是重設整個帳號。
- 懷疑帳號憑證洩露時再輪換使用者密碼。
- 例行維護時檢查舊 token 和已撤銷 token。

## 活動和日誌

PKV Sync 會記錄同步、筆記庫生命週期和唯讀瀏覽活動，包括使用者、筆記庫、動作、裝置名稱、檔案數、大小、IP、User-Agent、詳情和時間戳。筆記庫生命週期行包括來自 Admin WebUI、外掛或 API 操作的 `create_vault` 和 `delete_vault`。可用 Admin WebUI 的活動篩選檢查使用者或動作類型。

關注應用和反向代理日誌中重複出現的：

- `401`：憑證無效或已過期
- `403`：帳號停用或操作被禁止
- `404`：生產中介軟體拒絕部署金鑰或 User-Agent
- `409`：同步 head 不匹配或資源重複
- `429`：登入、註冊、認證同步 API 或 MCP HTTP 限流

## 發布衛生

生產升級前：

1. 閱讀 `CHANGELOG.md`。
2. 確認 release tag 與服務端、外掛、OpenAPI、Docker 和文件版本一致。
3. 檢查 GitHub release 包含 Linux amd64、Linux arm64、Windows x64、外掛 zip 和 `SHA256SUMS`。
4. 確認 GHCR 映像存在對應 tag 和 `latest`。
5. 備份目前資料。
6. 如果目前部署是 0.x，啟動 1.0 二進位或映像前先閱讀 [`upgrade-notes-v1.0.zh-Hant.md`](./upgrade-notes-v1.0.zh-Hant.md)。不要把 1.0 直接指向既有的 0.x `metadata.db`。
7. 用新二進位執行 migrations。

PKV Sync 1.0 使用單一 v1 SQLite 基線。在這次基線之後，已發布的 1.x migration 對既有 1.x 部署保持追加式。
