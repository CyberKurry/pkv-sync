# PKV Sync デプロイメント強化ガイド

[English](./deployment-hardening.md) | [简体中文](./deployment-hardening.zh-CN.md) | [繁體中文](./deployment-hardening.zh-Hant.md) | 日本語 | [한국어](./deployment-hardening.ko.md)

ドキュメントバージョン: v1.4.2。

この文書は機械翻訳による初版です。公開前にネイティブ話者によるレビューを推奨します。

このガイドは、自分、家族、チーム、または信頼できる友人グループ向けの小規模セルフホストデプロイメントを想定しています。PKV Sync は運用上シンプルですが、サーバー上に読み取り可能な vault 内容を保存するため、ホストとバックアップの衛生管理が重要です。

## 脅威モデル

PKV Sync はエンドツーエンド暗号化を提供しません。vault 内容の保護は層状の制御に依存します。

1. HTTPS transport encryption
2. Deployment key pre-authentication
3. Username/password login と、使用時に更新される bearer device tokens
4. ユーザーごとの vault authorization checks
5. Admin session と CSRF protections
6. OS または provider disk encryption
7. 公開サービスの最小化
8. 暗号化され、復元テスト済みの backups

サーバー管理者とサーバーファイルシステムは、平文 vault 内容を信頼して扱える境界として考えてください。

1.2.1 patch では露出面の境界も締めています。Git HTTP Basic の失敗は汎用メッセージになり、MCP JSON body の上限は 100 MiB で、blob metadata checks はシンボリックリンクされた blob paths を follow せず拒否します。

## 推奨トポロジー

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

前段に明示的なネットワーク制御層がない限り、`pkvsyncd` を直接インターネットへ公開しないでください。

## インストール入力

準備するもの:

- `sync.example.com` などのドメイン
- `pkvsyncd genkey` で作成した deployment key
- `/etc/pkv-sync/config.toml`
- 永続データディレクトリ。一般的には `/var/lib/pkv-sync`
- 有効な TLS 証明書を持つ reverse proxy

サーバー共有 URL は次の形式です。

```text
https://sync.example.com/k_xxx/
```

これは非公開にしてください。deployment key は API トラフィックの事前認証ゲートであり、ユーザーパスワードの代替ではありません。

## システムユーザー

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

`config.toml` を `/etc/pkv-sync/config.toml` に保存し、サービスユーザーと管理者だけが読めるようにしてください。

## ファイアウォール

一般的なホストでは SSH と HTTPS だけを公開します。

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

Caddy または別の ACME HTTP-01 クライアントが証明書を管理する場合は、検証とリダイレクト用に port `80` も公開します。

```bash
sudo ufw allow 80/tcp
```

ホスト上で直接実行する場合、`pkvsyncd` は localhost に bind します。

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose ではアプリを全コンテナインターフェイスに bind し、ホストデバッグが必要なときだけホスト port を localhost に公開します。

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose With Caddy

Caddy に HTTPS 証明書の取得と更新を任せたい場合はこの手順を使います。

1. DNS をサーバーへ向けます。

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. `docker-compose.yml` の隣に `config.toml` を作成します。

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey の出力に置き換える
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

3. `deploy/caddy/Caddyfile` の `sync.example.com` を置き換えます。
4. スタックを起動します。

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 新規データベースの初回起動後、setup wizard を開いて最初の管理者アカウントを作成します。

   ```text
   https://sync.example.com/setup
   ```

   可能であれば setup 中はプライベートネットワークまたは一時的な reverse-proxy allowlist の背後に置き、完了後すぐに公開アクセスを締めてください。通常の管理者サインインには `https://sync.example.com/admin/login` を使用します。

`./data`、`config.toml`、Caddy の named volumes をバックアップします。

アップグレード:

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

ダッシュボードは 24 時間ごとに GitHub releases を確認し、新しい PKV Sync release が利用可能なときに banner を表示します。新しいデータベースの初回起動時、`enabled` と `interval_seconds` はランタイム設定に seed されます。その後は Admin WebUI Settings から再起動なしで変更できます。ソースリポジトリは、エアギャップ mirror デプロイメント用の静的な `config.toml` フィールドのままです。

```toml
[update_check]
enabled = true                          # first-boot seed only
interval_seconds = 86400                # first-boot seed only
repo = "cyberkurry/pkv-sync"            # static GitHub repo to query
```

セットアップ後にエアギャップ host を静かに保つには、Admin WebUI のランタイム設定で更新確認を無効化するか、新規デプロイの seed として `enabled = false` を設定してください。

## public_host（admin POST に必須）

`[server].public_host` には、scheme を含めず、運用者が admin panel にアクセスする外部から見える hostname（標準外なら port も）を設定します。例: `sync.example.com` または `pkv.local:8443`。admin CSRF チェックはこの値から期待される origin を導出します。`public_host` が設定されている場合、期待される origin は `https://<public_host>` に固定され、reverse proxy が送る `X-Forwarded-Proto` によって admin CSRF チェックが backend HTTP へ downgrade されることはありません。

`public_host` が空の場合、すべての admin POST は `403 csrf validation failed` と `tracing::warn` ログ行で拒否されます。これは意図的な fail-closed 動作です。代替としてリクエスト自身の `Host` header にフォールバックすると、認証が攻撃者の影響を受ける header と結びつき、proxy が一貫しない host を転送したときに壊れます。

`public_host` は次も制御します。

- 設定時の本番風 admin cookies（`Secure`、`SameSite=Strict`）
- admin 内の "share server URL" リンクでの `https://` 生成
- `/api/plugin-manifest` が返す plugin asset URLs の `https://` 外部 host

Plugin manifest の URL 生成は、クライアントが送る `X-Forwarded-Proto` を信頼しません。本番環境では `public_host` を設定し、self-update clients が実際の外部 host を指す安定した asset URLs を受け取れるようにしてください。

SSE では、同じ設定が reverse proxy に対して、その route が通常の短命リクエストではなく keep-alive event stream であることを認識させる助けになります。

## Security Response Headers

PKV Sync は本番 server stack に次の response headers を追加します。

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- `public_host` 設定時の `Strict-Transport-Security: max-age=31536000; includeSubDomains`

TLS termination と `public_host` を一致させてください。HSTS は server が HTTPS public deployment として設定されている場合にのみ送信されます。

### エンドツーエンド暗号化について

PKV Sync 1.0 はエンドツーエンド暗号化ではありません。サーバー管理者およびサーバーファイルシステムにアクセスできる者は、同期された vault 内容を読み取れます。ネイティブな vault ごとの E2EE は 1.x ロードマップに含まれます。現時点でサーバーに対する機密性が必要な運用者は、暫定的な vault ごとの暗号化層として [`git-crypt-howto.md`](./git-crypt-howto.md) に従ってください。このモードではファイル名は引き続きサーバーから見えます。クライアント側で暗号化されるのはファイル内容のみです。

## Reverse Proxy Notes

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

リポジトリには `deploy/nginx/pkv-sync.conf` があります。HTTP を HTTPS へリダイレクトし、`client_max_body_size 110m` を設定し、標準的なブラウザー hardening headers を追加し、PKV Sync が host と client IP の処理に使う headers を転送します。

最小構成:

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

リポジトリには `deploy/traefik/docker-compose.traefik.yml` に Traefik の例があります。`trusted_proxies` を Traefik が使う Docker network CIDR に設定し、例のドメインと ACME email を置き換えてください。

## trusted_proxies

`X-Forwarded-For` は reverse proxy からのものだけを信頼します。proxy と app が同じホストで動く場合:

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

Docker bridge networking を使う場合:

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

広い public range を追加しないでください。クライアントが `X-Forwarded-For` を偽装できると、rate-limit と audit data が弱くなります。

## 実行時セキュリティ設定

Admin WebUI から確認します。

- Registration mode: private deployments では `disabled` または `invite_only` を維持します。
- Login rate-limit threshold、window、lock duration。
- Maximum file size。既定は `100 MiB`。
- Supported text extensions。
- Timezone。既定は `Asia/Shanghai`。

登録とログイン失敗は rate limited です。Setup、公開登録、ユーザー自身のパスワード変更、および管理者が作成またはリセットするパスワードは、12 文字以上で大文字、小文字、数字を含む必要があります。CLI 作成ユーザーにも強力なパスワードが必要です。

認証済み同期 API routes も、route、method、client IP、bearer token ごとに 60 秒あたり 600 リクエストの固定ウィンドウで制限されます。失敗した bearer token 認証は別途 client IP ごとに 60 秒あたり 120 回までに制限されます。limiter と audit log が実 client IP を見られるよう、`trusted_proxies` を正確に保ってください。

Blob upload request body は `max_file_size` で制限され、さらに hard blob cap（production では `512 MiB`）で常に clamp されます。Main SSE streams は開いている間 bearer token を再検証します。MCP read/search tools には response と total-search budgets があり、大きな vault が無制限の JSON response に展開されないようにしています。

Pull/tree traversal と rollback reachability checks は bounded です。現在の同期フィルターで拒否されたパスは、read、history、diff、commit-list surfaces から隠されます。

## Prometheus Metrics

`/metrics` は既定で無効です。`enable_metrics` runtime setting が true の場合、endpoint は Prometheus text exposition を返しますが、本番用のすべての gate、つまり deployment key middleware、plugin User-Agent guard、admin bearer token が引き続き必要です。

scrape clients には `X-PKVSync-Deployment-Key`、許可された PKV Sync User-Agent、`Authorization: Bearer <admin-token>` を送信させます。metrics を未認証ネットワークへ公開しないでください。

## バックアップ

次をまとめてバックアップします。

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

データベースをコピーする場合は SQLite online backup を使うか、サービスを停止してください。可能な限り、database、Git vault repositories、blobs を同じ時点から取得します。

組み込みの backup/restore helpers は symlink をたどりません。`vaults/` または `blobs/` 配下の symlink entries は backup 時に skip され、restore cleanup 時には link 自体だけを削除し、target には触れません。

restic の例:

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

バックアップがマシンを離れる前に暗号化し、定期的に復元をテストしてください。

## ディスク暗号化

利用可能なら LUKS、BitLocker、FileVault、または provider-managed disk encryption を使ってください。VPS プロバイダーが root disk を暗号化できない場合、暗号化された offsite backups は任意ではなく必須になります。

## Token Hygiene

装置 bearer token は認証済み使用時に更新され、90 日間アイドルで期限切れになり、各 token には 365 日の絶対有効期限があり、ユーザーまたは管理者が取り消せます。期限切れまたは取り消しまで、アクティブ token は資格情報として扱ってください。

Obsidian はプラグインのアクティブ token、deployment key、ログイン状態、安定した装置 ID をデバイスローカルストレージに保存します。Vault-local のプラグイン `data.json` は非機密の設定と同期インデックスだけを保持します。現在のビルドでは同期インデックスの key に deployment key を含めず、古い機密情報入りのインデックス項目は次回プラグインデータを書き込むときに破棄されます。Obsidian のデバイスローカルストレージ、共有アーカイブ、信頼できない同期先、平文バックアップ、古い `data.json` のコピーを保護するようユーザーへ伝えてください。これらが漏えいした可能性がある場合は、影響を受けた装置 token を取り消し、deployment key が露出した場合は deployment key もローテーションします。

推奨運用:

- Admin WebUI device pages から紛失装置を取り消します。
- 1 台の装置だけを紛失した場合は、アカウント全体のリセットより、その装置 token の取り消しを優先します。
- 資格情報の侵害が疑われる場合にユーザーパスワードを rotate します。
- 定期メンテナンスで古い token と取り消し済み token を確認します。

## アクティビティとログ

PKV Sync は同期、vault ライフサイクル、読み取り専用閲覧アクティビティを、user、vault、action、device name、file count、size、IP、User-Agent、details、timestamp とともに記録します。vault ライフサイクル行には Admin WebUI、プラグイン、API 操作からの `create_vault` と `delete_vault` が含まれます。Admin WebUI の activity filters で users または action types を確認できます。

アプリケーションと reverse-proxy logs で繰り返し発生する次を監視します。

- `401`: invalid or expired credentials
- `403`: disabled account or forbidden operation
- `404`: rejected deployment key/User-Agent in production middleware
- `409`: sync head mismatch or duplicate resource
- `429`: login, registration, authenticated sync API, or MCP HTTP rate limit

## Release Hygiene

本番アップグレード前:

1. `CHANGELOG.md` を読みます。
2. release tag が server、plugin、OpenAPI、Docker、docs versions と一致することを確認します。
3. GitHub release に Linux amd64、Linux arm64、Windows x64、plugin zip、`SHA256SUMS` が含まれることを確認します。
4. GHCR image が tag と `latest` に存在することを確認します。
5. 現在の data をバックアップします。
6. 現在のデプロイメントが 0.x の場合、1.0 binary または image を起動する前に [`upgrade-notes-v1.0.ja.md`](./upgrade-notes-v1.0.ja.md) を読んでください。1.0 を既存の 0.x `metadata.db` に向けないでください。
7. 新しい binary で migrations を実行します。

PKV Sync 1.0 は単一の v1 SQLite baseline を使用します。この baseline 以後、公開済みの 1.x migrations は既存の 1.x デプロイメントに対して append-only です。
