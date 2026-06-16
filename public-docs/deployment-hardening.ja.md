# PKV Sync 銉囥儣銉偆銉°兂銉堝挤鍖栥偓銈ゃ儔

[English](./deployment-hardening.md) | [绠€浣撲腑鏂嘳(./deployment-hardening.zh-CN.md) | [绻侀珨涓枃](./deployment-hardening.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./deployment-hardening.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

銇撱伄鏂囨浉銇姊扮炕瑷炽伀銈堛倠鍒濈増銇с仚銆傚叕闁嬪墠銇儘銈ゃ儐銈ｃ儢瑭辫€呫伀銈堛倠銉儞銉ャ兗銈掓帹濂ㄣ仐銇俱仚銆?

銇撱伄銈偆銉夈伅銆佽嚜鍒嗐€佸鏃忋€併儊銉笺儬銆併伨銇熴伅淇￠牸銇с亶銈嬪弸浜恒偘銉兗銉楀悜銇戙伄灏忚妯°偦銉儠銉涖偣銉堛儑銉椼儹銈ゃ儭銉炽儓銈掓兂瀹氥仐銇︺亜銇俱仚銆侾KV Sync 銇亱鐢ㄤ笂銈枫兂銉椼儷銇с仚銇屻€併偟銉笺儛銉间笂銇銇垮彇銈婂彲鑳姐仾 vault 鍐呭銈掍繚瀛樸仚銈嬨仧銈併€併儧銈广儓銇ㄣ儛銉冦偗銈儍銉椼伄琛涚敓绠＄悊銇岄噸瑕併仹銇欍€?

## 鑴呭▉銉儑銉?

PKV Sync 銇偍銉炽儔銉勩兗銈ㄣ兂銉夋殫鍙峰寲銈掓彁渚涖仐銇俱仜銈撱€倂ault 鍐呭銇繚璀枫伅灞ょ姸銇埗寰°伀渚濆瓨銇椼伨銇欍€?

1. HTTPS transport encryption
2. Deployment key pre-authentication
3. Username/password login 銇ㄣ€佷娇鐢ㄦ檪銇洿鏂般仌銈屻倠 bearer device tokens
4. 銉︺兗銈躲兗銇斻仺銇?vault authorization checks
5. Admin session 銇?CSRF protections
6. OS 銇俱仧銇?provider disk encryption
7. 鍏枊銈点兗銉撱偣銇渶灏忓寲
8. 鏆楀彿鍖栥仌銈屻€佸京鍏冦儐銈广儓娓堛伩銇?backups

銈点兗銉愩兗绠＄悊鑰呫仺銈点兗銉愩兗銉曘偂銈ゃ儷銈枫偣銉嗐儬銇€佸钩鏂?vault 鍐呭銈掍俊闋笺仐銇︽壉銇堛倠澧冪晫銇ㄣ仐銇﹁€冦亪銇︺亸銇犮仌銇勩€?

1.2.1 patch 銇с伅闇插嚭闈伄澧冪晫銈傜窢銈併仸銇勩伨銇欍€侴it HTTP Basic 銇け鏁椼伅姹庣敤銉°儍銈汇兗銈搞伀銇倞銆丮CP JSON body 銇笂闄愩伅 100 MiB 銇с€乥lob metadata checks 銇偡銉炽儨銉儍銈儶銉炽偗銇曘倢銇?blob paths 銈?follow 銇涖仛鎷掑惁銇椼伨銇欍€?

## 鎺ㄥエ銉堛儩銉偢銉?

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

鍓嶆銇槑绀虹殑銇儘銉冦儓銉兗銈埗寰″堡銇屻仾銇勯檺銈娿€乣pkvsyncd` 銈掔洿鎺ャ偆銉炽偪銉笺儘銉冦儓銇稿叕闁嬨仐銇亜銇с亸銇犮仌銇勩€?

## 銈ゃ兂銈广儓銉笺儷鍏ュ姏

婧栧倷銇欍倠銈傘伄:

- `sync.example.com` 銇仼銇儔銉°偆銉?
- `pkvsyncd genkey` 銇т綔鎴愩仐銇?deployment key
- `/etc/pkv-sync/config.toml`
- 姘哥稓銉囥兗銈裤儑銈ｃ儸銈儓銉€備竴鑸殑銇伅 `/var/lib/pkv-sync`
- 鏈夊姽銇?TLS 瑷兼槑鏇搞倰鎸併仱 reverse proxy

銈点兗銉愩兗鍏辨湁 URL 銇銇舰寮忋仹銇欍€?

```text
https://sync.example.com/k_xxx/
```

銇撱倢銇潪鍏枊銇仐銇︺亸銇犮仌銇勩€俤eployment key 銇?API 銉堛儵銉曘偅銉冦偗銇簨鍓嶈獚瑷笺偛銉笺儓銇с亗銈娿€併儲銉笺偠銉笺儜銈广儻銉笺儔銇唬鏇裤仹銇亗銈娿伨銇涖倱銆?

## 銈枫偣銉嗐儬銉︺兗銈躲兗

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

`config.toml` 銈?`/etc/pkv-sync/config.toml` 銇繚瀛樸仐銆併偟銉笺儞銈广儲銉笺偠銉笺仺绠＄悊鑰呫仩銇戙亴瑾倎銈嬨倛銇嗐伀銇椼仸銇忋仩銇曘亜銆?

## 銉曘偂銈ゃ偄銈︺偐銉笺儷

涓€鑸殑銇儧銈广儓銇с伅 SSH 銇?HTTPS 銇犮亼銈掑叕闁嬨仐銇俱仚銆?

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

Caddy 銇俱仧銇垾銇?ACME HTTP-01 銈儵銈ゃ偄銉炽儓銇岃鏄庢浉銈掔鐞嗐仚銈嬪牬鍚堛伅銆佹瑷笺仺銉儉銈ゃ儸銈儓鐢ㄣ伀 port `80` 銈傚叕闁嬨仐銇俱仚銆?

```bash
sudo ufw allow 80/tcp
```

銉涖偣銉堜笂銇х洿鎺ュ疅琛屻仚銈嬪牬鍚堛€乣pkvsyncd` 銇?localhost 銇?bind 銇椼伨銇欍€?

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose 銇с伅銈儣銉倰鍏ㄣ偝銉炽儐銉娿偆銉炽偪銉笺儠銈с偆銈广伀 bind 銇椼€併儧銈广儓銉囥儛銉冦偘銇屽繀瑕併仾銇ㄣ亶銇犮亼銉涖偣銉?port 銈?localhost 銇叕闁嬨仐銇俱仚銆?

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose With Caddy

Caddy 銇?HTTPS 瑷兼槑鏇搞伄鍙栧緱銇ㄦ洿鏂般倰浠汇仜銇熴亜鍫村悎銇亾銇墜闋嗐倰浣裤亜銇俱仚銆?

1. DNS 銈掋偟銉笺儛銉笺伕鍚戙亼銇俱仚銆?

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. `docker-compose.yml` 銇殻銇?`config.toml` 銈掍綔鎴愩仐銇俱仚銆?

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 銇嚭鍔涖伀缃亶鎻涖亪銈?
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

3. `deploy/caddy/Caddyfile` 銇?`sync.example.com` 銈掔疆銇嶆彌銇堛伨銇欍€?
4. 銈广偪銉冦偗銈掕捣鍕曘仐銇俱仚銆?

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 鏂拌銉囥兗銈裤儥銉笺偣銇垵鍥炶捣鍕曞緦銆乻etup wizard 銈掗枊銇勩仸鏈€鍒濄伄绠＄悊鑰呫偄銈偊銉炽儓銈掍綔鎴愩仐銇俱仚銆?

   ```text
   https://sync.example.com/setup
   ```

   鍙兘銇с亗銈屻伆 setup 涓伅銉椼儵銈ゃ儥銉笺儓銉嶃儍銉堛儻銉笺偗銇俱仧銇竴鏅傜殑銇?reverse-proxy allowlist 銇儗寰屻伀缃亶銆佸畬浜嗗緦銇欍亹銇叕闁嬨偄銈偦銈广倰绶犮倎銇︺亸銇犮仌銇勩€傞€氬父銇鐞嗚€呫偟銈ゃ兂銈ゃ兂銇伅 `https://sync.example.com/admin/login` 銈掍娇鐢ㄣ仐銇俱仚銆?

`./data`銆乣config.toml`銆丆addy 銇?named volumes 銈掋儛銉冦偗銈儍銉椼仐銇俱仚銆?

銈儍銉椼偘銉兗銉?

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

銉€銉冦偡銉ャ儨銉笺儔銇?24 鏅傞枔銇斻仺銇?GitHub releases 銈掔⒑瑾嶃仐銆佹柊銇椼亜 PKV Sync release 銇屽埄鐢ㄥ彲鑳姐仾銇ㄣ亶銇?banner 銈掕〃绀恒仐銇俱仚銆傛柊銇椼亜銉囥兗銈裤儥銉笺偣銇垵鍥炶捣鍕曟檪銆乣enabled` 銇?`interval_seconds` 銇儵銉炽偪銈ゃ儬瑷畾銇?seed 銇曘倢銇俱仚銆傘仢銇緦銇?Admin WebUI Settings 銇嬨倝鍐嶈捣鍕曘仾銇椼仹澶夋洿銇с亶銇俱仚銆傘偨銉笺偣銉儩銈搞儓銉伅銆併偍銈偖銉ｃ儍銉?mirror 銉囥儣銉偆銉°兂銉堢敤銇潤鐨勩仾 `config.toml` 銉曘偅銉笺儷銉夈伄銇俱伨銇с仚銆?

```toml
[update_check]
enabled = true                          # first-boot seed only
interval_seconds = 86400                # first-boot seed only
repo = "cyberkurry/pkv-sync"            # static GitHub repo to query
```

銈汇儍銉堛偄銉冦儣寰屻伀銈ㄣ偄銈儯銉冦儣 host 銈掗潤銇嬨伀淇濄仱銇伅銆丄dmin WebUI 銇儵銉炽偪銈ゃ儬瑷畾銇ф洿鏂扮⒑瑾嶃倰鐒″姽鍖栥仚銈嬨亱銆佹柊瑕忋儑銉椼儹銈ゃ伄 seed 銇ㄣ仐銇?`enabled = false` 銈掕ō瀹氥仐銇︺亸銇犮仌銇勩€?

## public_host锛坅dmin POST 銇繀闋堬級

`[server].public_host` 銇伅銆乻cheme 銈掑惈銈併仛銆侀亱鐢ㄨ€呫亴 admin panel 銇偄銈偦銈广仚銈嬪閮ㄣ亱銈夎銇堛倠 hostname锛堟婧栧銇倝 port 銈傦級銈掕ō瀹氥仐銇俱仚銆備緥: `sync.example.com` 銇俱仧銇?`pkv.local:8443`銆俛dmin CSRF 銉併偋銉冦偗銇亾銇€ゃ亱銈夋湡寰呫仌銈屻倠 origin 銈掑皫鍑恒仐銇俱仚銆俙public_host` 銇岃ō瀹氥仌銈屻仸銇勩倠鍫村悎銆佹湡寰呫仌銈屻倠 origin 銇?`https://<public_host>` 銇浐瀹氥仌銈屻€乺everse proxy 銇岄€併倠 `X-Forwarded-Proto` 銇倛銇ｃ仸 admin CSRF 銉併偋銉冦偗銇?backend HTTP 銇?downgrade 銇曘倢銈嬨亾銇ㄣ伅銇傘倞銇俱仜銈撱€?

`public_host` 銇岀┖銇牬鍚堛€併仚銇广仸銇?admin POST 銇?`403 csrf validation failed` 銇?`tracing::warn` 銉偘琛屻仹鎷掑惁銇曘倢銇俱仚銆傘亾銈屻伅鎰忓洺鐨勩仾 fail-closed 鍕曚綔銇с仚銆備唬鏇裤仺銇椼仸銉偗銈ㄣ偣銉堣嚜韬伄 `Host` header 銇儠銈┿兗銉儛銉冦偗銇欍倠銇ㄣ€佽獚瑷笺亴鏀绘拑鑰呫伄褰遍熆銈掑彈銇戙倠 header 銇ㄧ祼銇炽仱銇嶃€乸roxy 銇屼竴璨仐銇亜 host 銈掕虎閫併仐銇熴仺銇嶃伀澹娿倢銇俱仚銆?

`public_host` 銇銈傚埗寰°仐銇俱仚銆?

- 瑷畾鏅傘伄鏈暘棰?admin cookies锛坄Secure`銆乣SameSite=Strict`锛?
- admin 鍐呫伄 "share server URL" 銉兂銈仹銇?`https://` 鐢熸垚
- `/api/plugin-manifest` 銇岃繑銇?plugin asset URLs 銇?`https://` 澶栭儴 host

Plugin manifest 銇?URL 鐢熸垚銇€併偗銉┿偆銈兂銉堛亴閫併倠 `X-Forwarded-Proto` 銈掍俊闋笺仐銇俱仜銈撱€傛湰鐣挵澧冦仹銇?`public_host` 銈掕ō瀹氥仐銆乻elf-update clients 銇屽疅闅涖伄澶栭儴 host 銈掓寚銇欏畨瀹氥仐銇?asset URLs 銈掑彈銇戝彇銈屻倠銈堛亞銇仐銇︺亸銇犮仌銇勩€?

SSE 銇с伅銆佸悓銇樿ō瀹氥亴 reverse proxy 銇銇椼仸銆併仢銇?route 銇岄€氬父銇煭鍛姐儶銈偍銈广儓銇с伅銇亸 keep-alive event stream 銇с亗銈嬨亾銇ㄣ倰瑾嶈瓨銇曘仜銈嬪姪銇戙伀銇倞銇俱仚銆?

## Security Response Headers

PKV Sync 銇湰鐣?server stack 銇銇?response headers 銈掕拷鍔犮仐銇俱仚銆?

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- `public_host` 瑷畾鏅傘伄 `Strict-Transport-Security: max-age=31536000; includeSubDomains`

TLS termination 銇?`public_host` 銈掍竴鑷淬仌銇涖仸銇忋仩銇曘亜銆侶STS 銇?server 銇?HTTPS public deployment 銇ㄣ仐銇﹁ō瀹氥仌銈屻仸銇勩倠鍫村悎銇伄銇块€佷俊銇曘倢銇俱仚銆?

### 銈ㄣ兂銉夈儎銉笺偍銉炽儔鏆楀彿鍖栥伀銇ゃ亜銇?

PKV Sync 1.0 銇偍銉炽儔銉勩兗銈ㄣ兂銉夋殫鍙峰寲銇с伅銇傘倞銇俱仜銈撱€傘偟銉笺儛銉肩鐞嗚€呫亰銈堛伋銈点兗銉愩兗銉曘偂銈ゃ儷銈枫偣銉嗐儬銇偄銈偦銈广仹銇嶃倠鑰呫伅銆佸悓鏈熴仌銈屻仧 vault 鍐呭銈掕銇垮彇銈屻伨銇欍€傘儘銈ゃ儐銈ｃ儢銇?vault 銇斻仺銇?E2EE 銇?1.x 銉兗銉夈優銉冦儣銇惈銇俱倢銇俱仚銆傜従鏅傜偣銇с偟銉笺儛銉笺伀瀵俱仚銈嬫瀵嗘€с亴蹇呰銇亱鐢ㄨ€呫伅銆佹毇瀹氱殑銇?vault 銇斻仺銇殫鍙峰寲灞ゃ仺銇椼仸 [`git-crypt-howto.md`](./git-crypt-howto.md) 銇緭銇ｃ仸銇忋仩銇曘亜銆傘亾銇儮銉笺儔銇с伅銉曘偂銈ゃ儷鍚嶃伅寮曘亶缍氥亶銈点兗銉愩兗銇嬨倝瑕嬨亪銇俱仚銆傘偗銉┿偆銈兂銉堝伌銇ф殫鍙峰寲銇曘倢銈嬨伄銇儠銈°偆銉唴瀹广伄銇裤仹銇欍€?

## Reverse Proxy Notes

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

銉儩銈搞儓銉伀銇?`deploy/nginx/pkv-sync.conf` 銇屻亗銈娿伨銇欍€侶TTP 銈?HTTPS 銇搞儶銉€銈ゃ儸銈儓銇椼€乣client_max_body_size 110m` 銈掕ō瀹氥仐銆佹婧栫殑銇儢銉┿偊銈躲兗 hardening headers 銈掕拷鍔犮仐銆丳KV Sync 銇?host 銇?client IP 銇嚘鐞嗐伀浣裤亞 headers 銈掕虎閫併仐銇俱仚銆?

鏈€灏忔鎴?

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

銉儩銈搞儓銉伀銇?`deploy/traefik/docker-compose.traefik.yml` 銇?Traefik 銇緥銇屻亗銈娿伨銇欍€俙trusted_proxies` 銈?Traefik 銇屼娇銇?Docker network CIDR 銇ō瀹氥仐銆佷緥銇儔銉°偆銉炽仺 ACME email 銈掔疆銇嶆彌銇堛仸銇忋仩銇曘亜銆?

## trusted_proxies

`X-Forwarded-For` 銇?reverse proxy 銇嬨倝銇倐銇仩銇戙倰淇￠牸銇椼伨銇欍€俻roxy 銇?app 銇屽悓銇樸儧銈广儓銇у嫊銇忓牬鍚?

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

Docker bridge networking 銈掍娇銇嗗牬鍚?

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

搴冦亜 public range 銈掕拷鍔犮仐銇亜銇с亸銇犮仌銇勩€傘偗銉┿偆銈兂銉堛亴 `X-Forwarded-For` 銈掑伣瑁呫仹銇嶃倠銇ㄣ€乺ate-limit 銇?audit data 銇屽急銇忋仾銈娿伨銇欍€?

## 瀹熻鏅傘偦銈儱銉儐銈ｈō瀹?

Admin WebUI 銇嬨倝纰鸿獚銇椼伨銇欍€?

- Registration mode: private deployments 銇с伅 `disabled` 銇俱仧銇?`invite_only` 銈掔董鎸併仐銇俱仚銆?
- Login rate-limit threshold銆亀indow銆乴ock duration銆?
- Maximum file size銆傛棦瀹氥伅 `100 MiB`銆?
- Supported text extensions銆?
- Timezone銆傛棦瀹氥伅 `Asia/Shanghai`銆?

鐧婚尣銇ㄣ儹銈般偆銉冲け鏁椼伅 rate limited 銇с仚銆係etup銆佸叕闁嬬櫥閷层€併儲銉笺偠銉艰嚜韬伄銉戙偣銉兗銉夊鏇淬€併亰銈堛伋绠＄悊鑰呫亴浣滄垚銇俱仧銇儶銈汇儍銉堛仚銈嬨儜銈广儻銉笺儔銇€?2 鏂囧瓧浠ヤ笂銇уぇ鏂囧瓧銆佸皬鏂囧瓧銆佹暟瀛椼倰鍚個蹇呰銇屻亗銈娿伨銇欍€侰LI 浣滄垚銉︺兗銈躲兗銇倐寮峰姏銇儜銈广儻銉笺儔銇屽繀瑕併仹銇欍€?

瑾嶈娓堛伩鍚屾湡 API routes 銈傘€乺oute銆乵ethod銆乧lient IP銆乥earer token 銇斻仺銇?60 绉掋亗銇熴倞 600 銉偗銈ㄣ偣銉堛伄鍥哄畾銈︺偅銉炽儔銈︺仹鍒堕檺銇曘倢銇俱仚銆傚け鏁椼仐銇?bearer token 瑾嶈銇垾閫?client IP 銇斻仺銇?60 绉掋亗銇熴倞 120 鍥炪伨銇с伀鍒堕檺銇曘倢銇俱仚銆俵imiter 銇?audit log 銇屽疅 client IP 銈掕銈夈倢銈嬨倛銇嗐€乣trusted_proxies` 銈掓纰恒伀淇濄仯銇︺亸銇犮仌銇勩€?

Blob upload request body 銇?`max_file_size` 銇у埗闄愩仌銈屻€併仌銈夈伀 hard blob cap锛坧roduction 銇с伅 `512 MiB`锛夈仹甯搞伀 clamp 銇曘倢銇俱仚銆侻ain SSE streams 銇枊銇勩仸銇勩倠闁?bearer token 銈掑啀妞滆銇椼伨銇欍€侻CP read/search tools 銇伅 response 銇?total-search budgets 銇屻亗銈娿€佸ぇ銇嶃仾 vault 銇岀劇鍒堕檺銇?JSON response 銇睍闁嬨仌銈屻仾銇勩倛銇嗐伀銇椼仸銇勩伨銇欍€?

Pull/tree traversal 銇?rollback reachability checks 銇?bounded 銇с仚銆傜従鍦ㄣ伄鍚屾湡銉曘偅銉偪銉笺仹鎷掑惁銇曘倢銇熴儜銈广伅銆乺ead銆乭istory銆乨iff銆乧ommit-list surfaces 銇嬨倝闅犮仌銈屻伨銇欍€?

## Prometheus Metrics

`/metrics` 銇棦瀹氥仹鐒″姽銇с仚銆俙enable_metrics` runtime setting 銇?true 銇牬鍚堛€乪ndpoint 銇?Prometheus text exposition 銈掕繑銇椼伨銇欍亴銆佹湰鐣敤銇仚銇广仸銇?gate銆併仱銇俱倞 deployment key middleware銆乸lugin User-Agent guard銆乤dmin bearer token 銇屽紩銇嶇稓銇嶅繀瑕併仹銇欍€?

scrape clients 銇伅 `X-PKVSync-Deployment-Key`銆佽ū鍙仌銈屻仧 PKV Sync User-Agent銆乣Authorization: Bearer <admin-token>` 銈掗€佷俊銇曘仜銇俱仚銆俶etrics 銈掓湭瑾嶈銉嶃儍銉堛儻銉笺偗銇稿叕闁嬨仐銇亜銇с亸銇犮仌銇勩€?

## 銉愩儍銈偄銉冦儣

娆°倰銇俱仺銈併仸銉愩儍銈偄銉冦儣銇椼伨銇欍€?

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

銉囥兗銈裤儥銉笺偣銈掋偝銉斻兗銇欍倠鍫村悎銇?SQLite online backup 銈掍娇銇嗐亱銆併偟銉笺儞銈广倰鍋滄銇椼仸銇忋仩銇曘亜銆傚彲鑳姐仾闄愩倞銆乨atabase銆丟it vault repositories銆乥lobs 銈掑悓銇樻檪鐐广亱銈夊彇寰椼仐銇俱仚銆?

绲勩伩杈笺伩銇?backup/restore helpers 銇?symlink 銈掋仧銇┿倞銇俱仜銈撱€俙vaults/` 銇俱仧銇?`blobs/` 閰嶄笅銇?symlink entries 銇?backup 鏅傘伀 skip 銇曘倢銆乺estore cleanup 鏅傘伀銇?link 鑷綋銇犮亼銈掑墛闄ゃ仐銆乼arget 銇伅瑙︺倢銇俱仜銈撱€?

restic 銇緥:

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

銉愩儍銈偄銉冦儣銇屻優銈枫兂銈掗洟銈屻倠鍓嶃伀鏆楀彿鍖栥仐銆佸畾鏈熺殑銇京鍏冦倰銉嗐偣銉堛仐銇︺亸銇犮仌銇勩€?

## 銉囥偅銈广偗鏆楀彿鍖?

鍒╃敤鍙兘銇倝 LUKS銆丅itLocker銆丗ileVault銆併伨銇熴伅 provider-managed disk encryption 銈掍娇銇ｃ仸銇忋仩銇曘亜銆俈PS 銉椼儹銉愩偆銉€銉笺亴 root disk 銈掓殫鍙峰寲銇с亶銇亜鍫村悎銆佹殫鍙峰寲銇曘倢銇?offsite backups 銇换鎰忋仹銇仾銇忓繀闋堛伀銇倞銇俱仚銆?

## Token Hygiene

瑁呯疆 bearer token 銇獚瑷兼笀銇夸娇鐢ㄦ檪銇洿鏂般仌銈屻€?0 鏃ラ枔銈偆銉夈儷銇ф湡闄愬垏銈屻伀銇倞銆佸悇 token 銇伅 365 鏃ャ伄绲跺鏈夊姽鏈熼檺銇屻亗銈娿€併儲銉笺偠銉笺伨銇熴伅绠＄悊鑰呫亴鍙栥倞娑堛仜銇俱仚銆傛湡闄愬垏銈屻伨銇熴伅鍙栥倞娑堛仐銇俱仹銆併偄銈儐銈ｃ儢 token 銇硣鏍兼儏鍫便仺銇椼仸鎵便仯銇︺亸銇犮仌銇勩€?

Obsidian 銇儣銉┿偘銈ゃ兂銇偄銈儐銈ｃ儢 token銆乨eployment key銆併儹銈般偆銉崇姸鎱嬨€佸畨瀹氥仐銇熻缃?ID 銈掋儑銉愩偆銈广儹銉笺偒銉偣銉堛儸銉笺偢銇繚瀛樸仐銇俱仚銆俈ault-local 銇儣銉┿偘銈ゃ兂 `data.json` 銇潪姗熷瘑銇ō瀹氥仺鍚屾湡銈ゃ兂銉囥儍銈偣銇犮亼銈掍繚鎸併仐銇俱仚銆傜従鍦ㄣ伄銉撱儷銉夈仹銇悓鏈熴偆銉炽儑銉冦偗銈广伄 key 銇?deployment key 銈掑惈銈併仛銆佸彜銇勬瀵嗘儏鍫卞叆銈娿伄銈ゃ兂銉囥儍銈偣闋呯洰銇鍥炪儣銉┿偘銈ゃ兂銉囥兗銈裤倰鏇搞亶杈笺個銇ㄣ亶銇牬妫勩仌銈屻伨銇欍€侽bsidian 銇儑銉愩偆銈广儹銉笺偒銉偣銉堛儸銉笺偢銆佸叡鏈夈偄銉笺偒銈ゃ儢銆佷俊闋笺仹銇嶃仾銇勫悓鏈熷厛銆佸钩鏂囥儛銉冦偗銈儍銉椼€佸彜銇?`data.json` 銇偝銉斻兗銈掍繚璀枫仚銈嬨倛銇嗐儲銉笺偠銉笺伕浼濄亪銇︺亸銇犮仌銇勩€傘亾銈屻倝銇屾紡銇堛亜銇椼仧鍙兘鎬с亴銇傘倠鍫村悎銇€佸奖闊裤倰鍙椼亼銇熻缃?token 銈掑彇銈婃秷銇椼€乨eployment key 銇岄湶鍑恒仐銇熷牬鍚堛伅 deployment key 銈傘儹銉笺儐銉笺偡銉с兂銇椼伨銇欍€?

鎺ㄥエ閬嬬敤:

- Admin WebUI device pages 銇嬨倝绱涘け瑁呯疆銈掑彇銈婃秷銇椼伨銇欍€?
- 1 鍙般伄瑁呯疆銇犮亼銈掔礇澶便仐銇熷牬鍚堛伅銆併偄銈偊銉炽儓鍏ㄤ綋銇儶銈汇儍銉堛倛銈娿€併仢銇缃?token 銇彇銈婃秷銇椼倰鍎厛銇椼伨銇欍€?
- 璩囨牸鎯呭牨銇镜瀹炽亴鐤戙倧銈屻倠鍫村悎銇儲銉笺偠銉笺儜銈广儻銉笺儔銈?rotate 銇椼伨銇欍€?
- 瀹氭湡銉°兂銉嗐儕銉炽偣銇у彜銇?token 銇ㄥ彇銈婃秷銇楁笀銇?token 銈掔⒑瑾嶃仐銇俱仚銆?

## 銈偗銉嗐偅銉撱儐銈ｃ仺銉偘

PKV Sync 銇悓鏈熴€乿ault 銉┿偆銉曘偟銈ゃ偗銉€佽銇垮彇銈婂皞鐢ㄩ柌瑕с偄銈儐銈ｃ儞銉嗐偅銈掋€乽ser銆乿ault銆乤ction銆乨evice name銆乫ile count銆乻ize銆両P銆乁ser-Agent銆乨etails銆乼imestamp 銇ㄣ仺銈傘伀瑷橀尣銇椼伨銇欍€倂ault 銉┿偆銉曘偟銈ゃ偗銉銇伅 Admin WebUI銆併儣銉┿偘銈ゃ兂銆丄PI 鎿嶄綔銇嬨倝銇?`create_vault` 銇?`delete_vault` 銇屽惈銇俱倢銇俱仚銆侫dmin WebUI 銇?activity filters 銇?users 銇俱仧銇?action types 銈掔⒑瑾嶃仹銇嶃伨銇欍€?

銈儣銉偙銉笺偡銉с兂銇?reverse-proxy logs 銇х拱銈婅繑銇楃櫤鐢熴仚銈嬫銈掔洠瑕栥仐銇俱仚銆?

- `401`: invalid or expired credentials
- `403`: disabled account or forbidden operation
- `404`: rejected deployment key/User-Agent in production middleware
- `409`: sync head mismatch or duplicate resource
- `429`: login, registration, authenticated sync API, or MCP HTTP rate limit

## Release Hygiene

鏈暘銈儍銉椼偘銉兗銉夊墠:

1. `CHANGELOG.md` 銈掕銇裤伨銇欍€?
2. release tag 銇?server銆乸lugin銆丱penAPI銆丏ocker銆乨ocs versions 銇ㄤ竴鑷淬仚銈嬨亾銇ㄣ倰纰鸿獚銇椼伨銇欍€?
3. GitHub release 銇?Linux amd64銆丩inux arm64銆乄indows x64銆乸lugin zip銆乣SHA256SUMS` 銇屽惈銇俱倢銈嬨亾銇ㄣ倰纰鸿獚銇椼伨銇欍€?
4. GHCR image 銇?tag 銇?`latest` 銇瓨鍦ㄣ仚銈嬨亾銇ㄣ倰纰鸿獚銇椼伨銇欍€?
5. 鐝惧湪銇?data 銈掋儛銉冦偗銈儍銉椼仐銇俱仚銆?
6. 鐝惧湪銇儑銉椼儹銈ゃ儭銉炽儓銇?0.x 銇牬鍚堛€?.0 binary 銇俱仧銇?image 銈掕捣鍕曘仚銈嬪墠銇?[`upgrade-notes-v1.0.ja.md`](./upgrade-notes-v1.0.ja.md) 銈掕銈撱仹銇忋仩銇曘亜銆?.0 銈掓棦瀛樸伄 0.x `metadata.db` 銇悜銇戙仾銇勩仹銇忋仩銇曘亜銆?
7. 鏂般仐銇?binary 銇?migrations 銈掑疅琛屻仐銇俱仚銆?

PKV Sync 1.0 銇崢涓€銇?v1 SQLite baseline 銈掍娇鐢ㄣ仐銇俱仚銆傘亾銇?baseline 浠ュ緦銆佸叕闁嬫笀銇裤伄 1.x migrations 銇棦瀛樸伄 1.x 銉囥儣銉偆銉°兂銉堛伀瀵俱仐銇?append-only 銇с仚銆?
