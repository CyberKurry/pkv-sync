# PKV Sync 배포 강화 가이드

[English](./deployment-hardening.md) | [简体中文](./deployment-hardening.zh-CN.md) | [繁體中文](./deployment-hardening.zh-Hant.md) | [日本語](./deployment-hardening.ja.md) | 한국어

문서 버전: v1.1.0.

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

이 가이드는 본인, 가족, 팀 또는 신뢰하는 친구 그룹을 위한 소규모 자체 호스팅 배포를 가정합니다. PKV Sync는 운영이 단순하지만 서버에 읽을 수 있는 vault 내용을 저장하므로 호스트와 백업 위생이 중요합니다.

## 위협 모델

PKV Sync는 종단 간 암호화를 제공하지 않습니다. vault 내용 보호는 계층화된 제어에 의존합니다.

1. HTTPS transport encryption
2. Deployment key pre-authentication
3. Username/password login 및 사용 시 갱신되는 bearer device tokens
4. 사용자별 vault authorization checks
5. Admin session 및 CSRF protections
6. OS 또는 provider disk encryption
7. 노출 서비스 최소화
8. 암호화되고 복원 테스트된 backups

서버 관리자와 서버 파일 시스템은 평문 vault 내용을 신뢰할 수 있는 경계로 취급하세요.

## 권장 토폴로지

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

앞단에 명시적인 네트워크 제어 계층이 없으면 `pkvsyncd`를 인터넷에 직접 노출하지 마세요.

## 설치 입력값

준비 항목:

- `sync.example.com` 같은 도메인
- `pkvsyncd genkey`로 만든 deployment key
- `/etc/pkv-sync/config.toml`
- 영구 데이터 디렉터리. 보통 `/var/lib/pkv-sync`
- 유효한 TLS 인증서가 있는 reverse proxy

서버 공유 URL 형식:

```text
https://sync.example.com/k_xxx/
```

비공개로 유지하세요. deployment key는 API 트래픽의 사전 인증 관문이며 사용자 비밀번호를 대체하지 않습니다.

## 시스템 사용자

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

`config.toml`을 `/etc/pkv-sync/config.toml`에 저장하고 서비스 사용자와 관리자만 읽을 수 있게 하세요.

## 방화벽

일반적인 호스트에서는 SSH와 HTTPS만 노출합니다.

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

Caddy 또는 다른 ACME HTTP-01 클라이언트가 인증서를 관리한다면 검증과 리디렉션 트래픽을 위해 port `80`도 노출합니다.

```bash
sudo ufw allow 80/tcp
```

호스트에서 직접 실행할 때는 `pkvsyncd`를 localhost에 bind합니다.

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose에서는 앱을 모든 컨테이너 인터페이스에 bind하고, 호스트 디버깅이 필요할 때만 호스트 port를 localhost에 게시합니다.

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose With Caddy

Caddy가 HTTPS 인증서를 요청하고 갱신하게 하려면 이 경로를 사용하세요.

1. DNS를 서버로 지정합니다.

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. `docker-compose.yml` 옆에 `config.toml`을 만듭니다.

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 출력으로 바꾸세요
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

3. `deploy/caddy/Caddyfile`의 `sync.example.com`을 바꿉니다.
4. 스택을 시작합니다.

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 새 데이터베이스를 처음 시작한 뒤 setup wizard를 열어 첫 관리자 계정을 만듭니다.

   ```text
   https://sync.example.com/setup
   ```

   가능하면 setup 단계는 사설 네트워크 또는 임시 reverse-proxy allowlist 뒤에서 완료하고, 완료 후 즉시 공개 접근을 줄이세요. 일반 관리자 로그인에는 `https://sync.example.com/admin/login`을 사용합니다.

`./data`, `config.toml`, Caddy의 named volumes를 백업합니다.

업그레이드:

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

대시보드는 24시간마다 GitHub releases를 확인하고 새 PKV Sync 릴리스가 있으면 배너를 표시합니다. 새 데이터베이스의 첫 시작 때 `enabled`와 `interval_seconds`는 런타임 설정으로 seed됩니다. 이후에는 Admin WebUI Settings에서 재시작 없이 변경할 수 있습니다. 소스 저장소는 에어갭 mirror 배포를 위한 정적 `config.toml` 필드로 유지됩니다.

```toml
[update_check]
enabled = true                          # first-boot seed only
interval_seconds = 86400                # first-boot seed only
repo = "cyberkurry/pkv-sync"            # static GitHub repo to query
```

설정 후 에어갭 host를 조용히 유지하려면 Admin WebUI 런타임 설정에서 업데이트 확인을 끄거나, 새 배포의 seed로 `enabled = false`를 설정하세요.

## public_host(admin POST 필수)

`[server].public_host`를 scheme 없이, 운영자가 admin panel에 접근하는 외부에서 보이는 hostname(비표준이면 port 포함)으로 설정합니다. 예: `sync.example.com` 또는 `pkv.local:8443`. admin CSRF 검사는 이 값에서 예상 origin을 도출합니다. `public_host`가 설정된 경우 예상 origin은 `https://<public_host>`로 고정되며, reverse proxy가 보내는 `X-Forwarded-Proto`가 admin CSRF 검사를 backend HTTP로 downgrade하지 않습니다.

`public_host`가 비어 있으면 모든 admin POST가 `403 csrf validation failed`와 `tracing::warn` 로그 행으로 거부됩니다. 이는 의도적인 fail-closed 동작입니다. 대안으로 요청 자체의 `Host` header에 fallback하면 인증이 공격자가 영향을 줄 수 있는 header와 결합되고, proxy가 일관되지 않은 host를 전달할 때 깨집니다.

`public_host`는 다음도 구동합니다.

- 설정 시 프로덕션 스타일 admin cookies(`Secure`, `SameSite=Strict`)
- admin 안의 "share server URL" 링크에 대한 `https://` 생성
- `/api/plugin-manifest`가 반환하는 plugin asset URLs의 `https://` 외부 host

Plugin manifest URL 생성은 클라이언트가 보낸 `X-Forwarded-Proto`를 신뢰하지 않습니다. 프로덕션에서는 `public_host`를 설정해 self-update clients가 실제 외부 host를 가리키는 안정적인 asset URLs를 받도록 하세요.

SSE의 경우 같은 설정이 reverse proxy가 해당 route를 일반적인 짧은 요청이 아니라 keep-alive event stream으로 인식하는 데 도움이 됩니다.

## Security Response Headers

PKV Sync는 프로덕션 server stack에 다음 response headers를 추가합니다.

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- `public_host` 설정 시 `Strict-Transport-Security: max-age=31536000; includeSubDomains`

TLS termination과 `public_host`를 일치시키세요. HSTS는 server가 HTTPS public deployment로 설정된 경우에만 전송됩니다.

### 종단 간 암호화 안내

PKV Sync 1.0은 종단 간 암호화가 아닙니다. 서버 관리자와 서버 파일 시스템 접근 권한이 있는 누구나 동기화된 vault 내용을 읽을 수 있습니다. 네이티브 vault별 E2EE는 1.x 로드맵에 있습니다. 오늘 서버로부터의 기밀성이 필요한 운영자는 임시 vault별 암호화 계층으로 [`git-crypt-howto.md`](./git-crypt-howto.md)를 따르세요. 이 모드에서는 파일 이름이 서버에 그대로 보이며, 파일 내용만 클라이언트 측에서 암호화됩니다.

## Reverse Proxy Notes

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

저장소에는 `deploy/nginx/pkv-sync.conf`가 포함되어 있습니다. HTTP를 HTTPS로 리디렉션하고, `client_max_body_size 110m`를 설정하며, 표준 브라우저 hardening headers를 추가하고, PKV Sync가 host와 client IP 처리에 사용하는 headers를 전달합니다.

최소 형태:

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

저장소는 `deploy/traefik/docker-compose.traefik.yml`에 Traefik 예시를 제공합니다. `trusted_proxies`를 Traefik이 사용하는 Docker network CIDR로 설정하고 예시 도메인과 ACME email을 바꾸세요.

## trusted_proxies

reverse proxy에서 온 `X-Forwarded-For`만 신뢰하세요. proxy와 app이 같은 호스트에서 실행되는 경우:

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

Docker bridge networking을 사용하는 경우:

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

넓은 public range를 추가하지 마세요. 클라이언트가 `X-Forwarded-For`를 위조할 수 있으면 rate-limit와 audit data가 약해집니다.

## 런타임 보안 설정

Admin WebUI에서 확인하세요.

- Registration mode: private deployments에서는 `disabled` 또는 `invite_only`를 유지합니다.
- Login rate-limit threshold, window, lock duration.
- Maximum file size, 기본값 `100 MiB`.
- Supported text extensions.
- Timezone, 기본값 `Asia/Shanghai`.

등록과 로그인 실패는 rate limited입니다. Setup, 공개 등록, 사용자 self-service 비밀번호 변경, 그리고 관리자가 생성하거나 재설정하는 비밀번호는 12자 이상이며 대문자, 소문자, 숫자를 포함해야 합니다. CLI로 만든 사용자도 강한 비밀번호가 필요합니다.

인증된 동기화 API routes도 route, method, client IP, bearer token별로 60초당 600개 요청의 고정 창 제한을 받습니다. 실패한 bearer token 인증은 별도로 client IP별 60초당 120회까지 제한됩니다. limiter와 audit log가 실제 client IP를 보도록 `trusted_proxies`를 정확히 유지하세요.

Blob upload request body는 `max_file_size`로 제한되며 hard blob cap(프로덕션 `512 MiB`)으로도 항상 clamp됩니다. Main SSE streams는 열린 동안 bearer token을 재검증합니다. MCP read/search tools에는 response와 total-search budgets가 있어 큰 vault가 무제한 JSON response로 확장되지 않게 합니다.

Pull/tree traversal과 rollback reachability checks는 bounded입니다. 현재 동기화 필터에서 거부된 경로는 read, history, diff, commit-list surfaces에서 숨겨집니다.

## Prometheus Metrics

`/metrics`는 기본적으로 비활성화되어 있습니다. `enable_metrics` runtime setting이 true이면 endpoint는 Prometheus text exposition을 반환하지만, 모든 프로덕션 관문인 deployment key middleware, plugin User-Agent guard, admin bearer token이 계속 필요합니다.

scrape clients가 `X-PKVSync-Deployment-Key`, 허용된 PKV Sync User-Agent, `Authorization: Bearer <admin-token>`을 보내도록 설정하세요. metrics를 인증되지 않은 네트워크에 노출하지 마세요.

## 백업

다음을 함께 백업합니다.

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

데이터베이스를 복사할 때는 SQLite online backup을 사용하거나 서비스를 중지하세요. 가능하면 database, Git vault repositories, blobs가 같은 시점의 것이 되게 합니다.

내장 backup/restore helpers는 symlink를 따라가지 않습니다. `vaults/` 또는 `blobs/` 아래의 symlink entries는 backup 중 skip되고 restore cleanup 중에는 link 자체만 제거하며 target은 건드리지 않습니다.

restic 예시:

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

백업이 머신을 떠나기 전에 암호화하고 주기적으로 복원을 테스트하세요.

## 디스크 암호화

가능하면 LUKS, BitLocker, FileVault 또는 provider-managed disk encryption을 사용하세요. VPS 공급자가 root disk를 암호화할 수 없다면 암호화된 offsite backups는 선택 사항이 아니라 필수입니다.

## Token Hygiene

장치 bearer token은 인증된 사용 시 갱신되고, 90일 동안 유휴이면 만료되며, 각 token에는 365일의 절대 수명이 있고, 사용자 또는 관리자가 철회할 수 있습니다. 만료되거나 철회될 때까지 활성 token을 자격 증명으로 취급하세요.

Obsidian은 플러그인의 활성 token과 deployment key를 vault-local plugin data file인 `<vault>/.obsidian/plugins/pkv-sync/data.json`에 저장합니다. 사용자에게 이 파일을 공유 아카이브, 신뢰할 수 없는 동기화 대상, 평문 백업에 넣지 말라고 안내하세요. 파일이 유출되었을 수 있으면 영향을 받은 장치 token을 철회하세요.

권장 방식:

- Admin WebUI device pages에서 분실한 장치를 철회합니다.
- 한 장치만 잃어버렸다면 전체 계정 재설정보다 해당 장치 token 철회를 우선합니다.
- 자격 증명 침해가 의심될 때 사용자 비밀번호를 rotate합니다.
- 정기 유지보수 중 오래된 token과 철회된 token을 검토합니다.

## 활동과 로그

PKV Sync는 동기화, vault 수명 주기, 읽기 전용 탐색 활동을 user, vault, action, device name, file count, size, IP, User-Agent, details, timestamp와 함께 기록합니다. vault 수명 주기 행에는 Admin WebUI, 플러그인 또는 API 작업의 `create_vault`와 `delete_vault`가 포함됩니다. Admin WebUI activity filters로 users 또는 action types를 확인할 수 있습니다.

애플리케이션과 reverse-proxy logs에서 반복되는 다음을 감시하세요.

- `401`: invalid or expired credentials
- `403`: disabled account or forbidden operation
- `404`: rejected deployment key/User-Agent in production middleware
- `409`: sync head mismatch or duplicate resource
- `429`: login, registration, authenticated sync API, or MCP HTTP rate limit

## Release Hygiene

프로덕션 업그레이드 전:

1. `CHANGELOG.md`를 읽습니다.
2. release tag가 server, plugin, OpenAPI, Docker, docs versions와 일치하는지 확인합니다.
3. GitHub release에 Linux amd64, Linux arm64, Windows x64, plugin zip, `SHA256SUMS`가 포함되어 있는지 확인합니다.
4. GHCR image가 해당 tag와 `latest`에 존재하는지 확인합니다.
5. 현재 data를 백업합니다.
6. 현재 배포가 0.x라면 1.0 binary 또는 image를 시작하기 전에 [`upgrade-notes-v1.0.ko.md`](./upgrade-notes-v1.0.ko.md)를 읽으세요. 1.0을 기존 0.x `metadata.db`에 연결하지 마세요.
7. 새 binary로 migrations를 실행합니다.

PKV Sync 1.0은 단일 v1 SQLite baseline을 사용합니다. 이 baseline 이후 게시되는 1.x migrations는 기존 1.x 배포에 대해 append-only입니다.
