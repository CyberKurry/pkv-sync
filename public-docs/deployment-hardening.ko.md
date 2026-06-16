# PKV Sync 氚绊彫 臧曧檾 臧€鞚措摐

[English](./deployment-hardening.md) | [绠€浣撲腑鏂嘳(./deployment-hardening.zh-CN.md) | [绻侀珨涓枃](./deployment-hardening.zh-Hant.md) | [鏃ユ湰瑾瀅(./deployment-hardening.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.3.

鞚?氍胳劀電?旮瓣硠 氩堨棴鞙茧 毵岆摖 齑堦赴 氩勳爠鞛呺媹雼? 瓿店皽 鞝勳棎 鞗愳柎氙?瓴€韱犽ゼ 甓岇灔頃╇媹雼?

鞚?臧€鞚措摐電?氤胳澑, 臧€臁? 韺€ 霕愲姅 鞁犽頃橂姅 旃滉惮 攴鸽９鞚?鞙勴暅 靻岅窚氇?鞛愳泊 順胳姢韺?氚绊彫毳?臧€鞝曧暕雼堧嫟. PKV Sync電?鞖挫榿鞚?雼垳頃橃毵?靹滊矂鞐?鞚届潉 靾?鞛堧姅 vault 雮挫毄鞚?鞝€鞛ロ晿氙€搿?順胳姢韸胳檧 氚膘梾 鞙勳儩鞚?欷戩殧頃╇媹雼?

## 鞙勴槕 氇嵏

PKV Sync電?膦呺嫧 臧?鞎旐樃頇旊ゼ 鞝滉车頃橃 鞎婌姷雼堧嫟. vault 雮挫毄 氤错樃電?瓿勳傅頇旊悳 鞝滌柎鞐?鞚橃〈頃╇媹雼?

1. HTTPS transport encryption
2. Deployment key pre-authentication
3. Username/password login 氚?靷毄 鞁?臧膘嫚霅橂姅 bearer device tokens
4. 靷毄鞛愲硠 vault authorization checks
5. Admin session 氚?CSRF protections
6. OS 霕愲姅 provider disk encryption
7. 雲胳稖 靹滊箘鞀?斓滌唽頇?
8. 鞎旐樃頇旊悩瓿?氤奠洂 韰岇姢韸鸽悳 backups

靹滊矂 甏€毽瀽鞕€ 靹滊矂 韺岇澕 鞁滌姢韰滌潃 韽夒 vault 雮挫毄鞚?鞁犽頃?靾?鞛堧姅 瓴疥硠搿?旆笁頃橃劯鞖?

1.2.1 patch電?雲胳稖 瓴疥硠霃?雿?臁办瀰雼堧嫟. Git HTTP Basic 鞁ろ尐電?鞚茧皹 氅旍嫓歆€搿?觳橂Μ霅橁碃, MCP JSON body 靸來暅鞚€ 100 MiB鞚措┌, blob metadata checks電?鞁臣毽?毵來伂霅?blob paths毳?霐半澕臧€歆€ 鞎婈碃 瓯半秬頃╇媹雼?

## 甓岇灔 韱犿彺搿滌

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

鞎炿嫧鞐?氇呾嫓鞝侅澑 雱ろ姼鞗岉伂 鞝滌柎 瓿勳傅鞚?鞐嗢溂氅?`pkvsyncd`毳?鞚疙劙雱缝棎 歆侅爲 雲胳稖頃橃 毵堨劯鞖?

## 靹れ箻 鞛呺牓臧?

欷€牍?頃:

- `sync.example.com` 臧欖潃 霃勲鞚?
- `pkvsyncd genkey`搿?毵岆摖 deployment key
- `/etc/pkv-sync/config.toml`
- 鞓侁惮 雿办澊韯?霐旊爥韯半Μ. 氤错喌 `/var/lib/pkv-sync`
- 鞙犿毃頃?TLS 鞚胳靹滉皜 鞛堧姅 reverse proxy

靹滊矂 瓿奠湢 URL 順曥嫕:

```text
https://sync.example.com/k_xxx/
```

牍勱车臧滊 鞙犾頃橃劯鞖? deployment key電?API 韸鸽灅頂届潣 靷爠 鞚胳 甏€氍胳澊氅?靷毄鞛?牍勲皜氩堩樃毳?雽€觳错晿歆€ 鞎婌姷雼堧嫟.

## 鞁滌姢韰?靷毄鞛?

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

`config.toml`鞚?`/etc/pkv-sync/config.toml`鞐?鞝€鞛ロ晿瓿?靹滊箘鞀?靷毄鞛愳檧 甏€毽瀽毵?鞚届潉 靾?鞛堦矊 頃橃劯鞖?

## 氚╉檾氩?

鞚茧皹鞝侅澑 順胳姢韸胳棎靹滊姅 SSH鞕€ HTTPS毵?雲胳稖頃╇媹雼?

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

Caddy 霕愲姅 雼るジ ACME HTTP-01 韥措澕鞚挫柛韸戈皜 鞚胳靹滊ゼ 甏€毽暅雼る┐ 瓴€歃濌臣 毽敂霠夓厴 韸鸽灅頂届潉 鞙勴暣 port `80`霃?雲胳稖頃╇媹雼?

```bash
sudo ufw allow 80/tcp
```

順胳姢韸胳棎靹?歆侅爲 鞁ろ枆頃?霑岆姅 `pkvsyncd`毳?localhost鞐?bind頃╇媹雼?

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose鞐愳劀電?鞎膘潉 氇摖 旎厡鞚措剤 鞚疙劙韼橃澊鞀れ棎 bind頃橁碃, 順胳姢韸?霐旊矂旯呾澊 頃勳殧頃?霑岆 順胳姢韸?port毳?localhost鞐?瓴岇嫓頃╇媹雼?

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose With Caddy

Caddy臧€ HTTPS 鞚胳靹滊ゼ 鞖旍箔頃橁碃 臧膘嫚頃橁矊 頃橂牑氅?鞚?瓴诫毳?靷毄頃橃劯鞖?

1. DNS毳?靹滊矂搿?歆€鞝曧暕雼堧嫟.

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. `docker-compose.yml` 鞓嗢棎 `config.toml`鞚?毵岆摥雼堧嫟.

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 於滊牓鞙茧 氚旉靖靹胳殧
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

3. `deploy/caddy/Caddyfile`鞚?`sync.example.com`鞚?氚旉繅雼堧嫟.
4. 鞀ろ儩鞚?鞁滌瀾頃╇媹雼?

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 靸?雿办澊韯半矤鞚挫姢毳?觳橃潓 鞁滌瀾頃?霋?setup wizard毳?鞐挫柎 觳?甏€毽瀽 瓿勳爼鞚?毵岆摥雼堧嫟.

   ```text
   https://sync.example.com/setup
   ```

   臧€電ロ晿氅?setup 雼硠電?靷劋 雱ろ姼鞗岉伂 霕愲姅 鞛勳嫓 reverse-proxy allowlist 霋れ棎靹?鞕勲頃橁碃, 鞕勲 頉?歃夓嫓 瓿店皽 鞝戧芳鞚?欷勳澊靹胳殧. 鞚茧皹 甏€毽瀽 搿滉犯鞚胳棎電?`https://sync.example.com/admin/login`鞚?靷毄頃╇媹雼?

`./data`, `config.toml`, Caddy鞚?named volumes毳?氚膘梾頃╇媹雼?

鞐呹犯霠堨澊霌?

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

雽€鞁滊炒霌滊姅 24鞁滉皠毵堧嫟 GitHub releases毳?頇曥澑頃橁碃 靸?PKV Sync 毽措Μ鞀り皜 鞛堨溂氅?氚半剤毳?響滌嫓頃╇媹雼? 靸?雿办澊韯半矤鞚挫姢鞚?觳?鞁滌瀾 霑?`enabled`鞕€ `interval_seconds`電?霟绊儉鞛?靹れ爼鞙茧 seed霅╇媹雼? 鞚错泟鞐愲姅 Admin WebUI Settings鞐愳劀 鞛嫓鞛?鞐嗢澊 氤€瓴巾暊 靾?鞛堨姷雼堧嫟. 靻岇姢 鞝€鞛レ唽電?鞐愳柎臧?mirror 氚绊彫毳?鞙勴暅 鞝曥爜 `config.toml` 頃勲摐搿?鞙犾霅╇媹雼?

```toml
[update_check]
enabled = true                          # first-boot seed only
interval_seconds = 86400                # first-boot seed only
repo = "cyberkurry/pkv-sync"            # static GitHub repo to query
```

靹れ爼 頉?鞐愳柎臧?host毳?臁办毄頌?鞙犾頃橂牑氅?Admin WebUI 霟绊儉鞛?靹れ爼鞐愳劀 鞐呺嵃鞚错姼 頇曥澑鞚?雭勱卑雮? 靸?氚绊彫鞚?seed搿?`enabled = false`毳?靹れ爼頃橃劯鞖?

## public_host(admin POST 頃勳垬)

`[server].public_host`毳?scheme 鞐嗢澊, 鞖挫榿鞛愱皜 admin panel鞐?鞝戧芳頃橂姅 鞕鸽秬鞐愳劀 氤挫澊電?hostname(牍勴憸欷€鞚措┐ port 韽暔)鞙茧 靹れ爼頃╇媹雼? 鞓? `sync.example.com` 霕愲姅 `pkv.local:8443`. admin CSRF 瓴€靷姅 鞚?臧掛棎靹?鞓堨儊 origin鞚?霃勳稖頃╇媹雼? `public_host`臧€ 靹れ爼霅?瓴届毎 鞓堨儊 origin鞚€ `https://<public_host>`搿?瓿犾爼霅橂┌, reverse proxy臧€ 氤措偞電?`X-Forwarded-Proto`臧€ admin CSRF 瓴€靷ゼ backend HTTP搿?downgrade頃橃 鞎婌姷雼堧嫟.

`public_host`臧€ 牍勳柎 鞛堨溂氅?氇摖 admin POST臧€ `403 csrf validation failed`鞕€ `tracing::warn` 搿滉犯 頄夓溂搿?瓯半秬霅╇媹雼? 鞚措姅 鞚橂弰鞝侅澑 fail-closed 霃欖瀾鞛呺媹雼? 雽€鞎堨溂搿?鞖旍箔 鞛愳泊鞚?`Host` header鞐?fallback頃橂┐ 鞚胳鞚?瓿店博鞛愱皜 鞓來枼鞚?欷?靾?鞛堧姅 header鞕€ 瓴绊暕霅橁碃, proxy臧€ 鞚缄磤霅橃 鞎婌潃 host毳?鞝勲嫭頃?霑?旯雼堧嫟.

`public_host`電?雼れ潓霃?甑彊頃╇媹雼?

- 靹れ爼 鞁?頂勲雿曥厴 鞀ろ儉鞚?admin cookies(`Secure`, `SameSite=Strict`)
- admin 鞎堨潣 "share server URL" 毵來伂鞐?雽€頃?`https://` 靸濎劚
- `/api/plugin-manifest`臧€ 氚橅櫂頃橂姅 plugin asset URLs鞚?`https://` 鞕鸽秬 host

Plugin manifest URL 靸濎劚鞚€ 韥措澕鞚挫柛韸戈皜 氤措偢 `X-Forwarded-Proto`毳?鞁犽頃橃 鞎婌姷雼堧嫟. 頂勲雿曥厴鞐愳劀電?`public_host`毳?靹れ爼頃?self-update clients臧€ 鞁れ牅 鞕鸽秬 host毳?臧€毽偆電?鞎堨爼鞝侅澑 asset URLs毳?氚涬弰搿?頃橃劯鞖?

SSE鞚?瓴届毎 臧欖潃 靹れ爼鞚?reverse proxy臧€ 頃措嫻 route毳?鞚茧皹鞝侅澑 歆ъ潃 鞖旍箔鞚?鞎勲媹霛?keep-alive event stream鞙茧 鞚胳嫕頃橂姅 雿?霃勳泙鞚?霅╇媹雼?

## Security Response Headers

PKV Sync電?頂勲雿曥厴 server stack鞐?雼れ潓 response headers毳?於旉皜頃╇媹雼?

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- `public_host` 靹れ爼 鞁?`Strict-Transport-Security: max-age=31536000; includeSubDomains`

TLS termination瓿?`public_host`毳?鞚检箻鞁滍偆靹胳殧. HSTS電?server臧€ HTTPS public deployment搿?靹れ爼霅?瓴届毎鞐愲 鞝勳啞霅╇媹雼?

### 膦呺嫧 臧?鞎旐樃頇?鞎堧偞

PKV Sync 1.0鞚€ 膦呺嫧 臧?鞎旐樃頇旉皜 鞎勲嫏雼堧嫟. 靹滊矂 甏€毽瀽鞕€ 靹滊矂 韺岇澕 鞁滌姢韰?鞝戧芳 甓岉暅鞚?鞛堧姅 雸勱惮雮?霃欔赴頇旊悳 vault 雮挫毄鞚?鞚届潉 靾?鞛堨姷雼堧嫟. 雱れ澊韹半笇 vault氤?E2EE電?1.x 搿滊摐毵奠棎 鞛堨姷雼堧嫟. 鞓る姌 靹滊矂搿滊秬韯办潣 旮半皜靹膘澊 頃勳殧頃?鞖挫榿鞛愲姅 鞛勳嫓 vault氤?鞎旐樃頇?瓿勳傅鞙茧 [`git-crypt-howto.md`](./git-crypt-howto.md)毳?霐半ゴ靹胳殧. 鞚?氇摐鞐愳劀電?韺岇澕 鞚措鞚?靹滊矂鞐?攴鸽寑搿?氤挫澊氅? 韺岇澕 雮挫毄毵?韥措澕鞚挫柛韸?旄§棎靹?鞎旐樃頇旊惄雼堧嫟.

## Reverse Proxy Notes

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

鞝€鞛レ唽鞐愲姅 `deploy/nginx/pkv-sync.conf`臧€ 韽暔霅橃柎 鞛堨姷雼堧嫟. HTTP毳?HTTPS搿?毽敂霠夓厴頃橁碃, `client_max_body_size 110m`毳?靹れ爼頃橂┌, 響滌 敫岆澕鞖办爛 hardening headers毳?於旉皜頃橁碃, PKV Sync臧€ host鞕€ client IP 觳橂Μ鞐?靷毄頃橂姅 headers毳?鞝勲嫭頃╇媹雼?

斓滌唽 順曧儨:

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

鞝€鞛レ唽電?`deploy/traefik/docker-compose.traefik.yml`鞐?Traefik 鞓堨嫓毳?鞝滉车頃╇媹雼? `trusted_proxies`毳?Traefik鞚?靷毄頃橂姅 Docker network CIDR搿?靹れ爼頃橁碃 鞓堨嫓 霃勲鞚戈臣 ACME email鞚?氚旉靖靹胳殧.

## trusted_proxies

reverse proxy鞐愳劀 鞓?`X-Forwarded-For`毵?鞁犽頃橃劯鞖? proxy鞕€ app鞚?臧欖潃 順胳姢韸胳棎靹?鞁ろ枆霅橂姅 瓴届毎:

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

Docker bridge networking鞚?靷毄頃橂姅 瓴届毎:

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

雱撿潃 public range毳?於旉皜頃橃 毵堨劯鞖? 韥措澕鞚挫柛韸戈皜 `X-Forwarded-For`毳?鞙勳“頃?靾?鞛堨溂氅?rate-limit鞕€ audit data臧€ 鞎巾暣歆戨媹雼?

## 霟绊儉鞛?氤挫晥 靹れ爼

Admin WebUI鞐愳劀 頇曥澑頃橃劯鞖?

- Registration mode: private deployments鞐愳劀電?`disabled` 霕愲姅 `invite_only`毳?鞙犾頃╇媹雼?
- Login rate-limit threshold, window, lock duration.
- Maximum file size, 旮半掣臧?`100 MiB`.
- Supported text extensions.
- Timezone, 旮半掣臧?`Asia/Shanghai`.

霌彪瓿?搿滉犯鞚?鞁ろ尐電?rate limited鞛呺媹雼? Setup, 瓿店皽 霌彪, 靷毄鞛?self-service 牍勲皜氩堩樃 氤€瓴? 攴鸽Μ瓿?甏€毽瀽臧€ 靸濎劚頃橁卑雮?鞛劋鞝曧晿電?牍勲皜氩堩樃電?12鞛?鞚挫儊鞚措┌ 雽€氍胳瀽, 靻岆鞛? 靾瀽毳?韽暔頃挫暭 頃╇媹雼? CLI搿?毵岆摖 靷毄鞛愲弰 臧曧暅 牍勲皜氩堩樃臧€ 頃勳殧頃╇媹雼?

鞚胳霅?霃欔赴頇?API routes霃?route, method, client IP, bearer token氤勲 60齑堧嫻 600臧?鞖旍箔鞚?瓿犾爼 彀?鞝滍暅鞚?氚涭姷雼堧嫟. 鞁ろ尐頃?bearer token 鞚胳鞚€ 氤勲弰搿?client IP氤?60齑堧嫻 120須岅箤歆€ 鞝滍暅霅╇媹雼? limiter鞕€ audit log臧€ 鞁れ牅 client IP毳?氤措弰搿?`trusted_proxies`毳?鞝曧檿頌?鞙犾頃橃劯鞖?

Blob upload request body電?`max_file_size`搿?鞝滍暅霅橂┌ hard blob cap(頂勲雿曥厴 `512 MiB`)鞙茧霃?頃儊 clamp霅╇媹雼? Main SSE streams電?鞐措Π 霃欖晥 bearer token鞚?鞛瞼歃濏暕雼堧嫟. MCP read/search tools鞐愲姅 response鞕€ total-search budgets臧€ 鞛堨柎 韥?vault臧€ 氍挫牅頃?JSON response搿?頇曥灔霅橃 鞎婈矊 頃╇媹雼?

Pull/tree traversal瓿?rollback reachability checks電?bounded鞛呺媹雼? 順勳灛 霃欔赴頇?頃勴劙鞐愳劀 瓯半秬霅?瓴诫電?read, history, diff, commit-list surfaces鞐愳劀 靾波歆戨媹雼?

## Prometheus Metrics

`/metrics`電?旮半掣鞝侅溂搿?牍勴櫆靹表檾霅橃柎 鞛堨姷雼堧嫟. `enable_metrics` runtime setting鞚?true鞚措┐ endpoint電?Prometheus text exposition鞚?氚橅櫂頃橃毵? 氇摖 頂勲雿曥厴 甏€氍胳澑 deployment key middleware, plugin User-Agent guard, admin bearer token鞚?瓿勳啀 頃勳殧頃╇媹雼?

scrape clients臧€ `X-PKVSync-Deployment-Key`, 項堨毄霅?PKV Sync User-Agent, `Authorization: Bearer <admin-token>`鞚?氤措偞霃勲 靹れ爼頃橃劯鞖? metrics毳?鞚胳霅橃 鞎婌潃 雱ろ姼鞗岉伂鞐?雲胳稖頃橃 毵堨劯鞖?

## 氚膘梾

雼れ潓鞚?頃粯 氚膘梾頃╇媹雼?

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

雿办澊韯半矤鞚挫姢毳?氤奠偓頃?霑岆姅 SQLite online backup鞚?靷毄頃橁卑雮?靹滊箘鞀るゼ 欷戩頃橃劯鞖? 臧€電ロ晿氅?database, Git vault repositories, blobs臧€ 臧欖潃 鞁滌爯鞚?瓴冹澊 霅橁矊 頃╇媹雼?

雮挫灔 backup/restore helpers電?symlink毳?霐半澕臧€歆€ 鞎婌姷雼堧嫟. `vaults/` 霕愲姅 `blobs/` 鞎勲灅鞚?symlink entries電?backup 欷?skip霅橁碃 restore cleanup 欷戩棎電?link 鞛愳泊毵?鞝滉卑頃橂┌ target鞚€ 瓯措摐毽 鞎婌姷雼堧嫟.

restic 鞓堨嫓:

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

氚膘梾鞚?毹胳嫚鞚?霒犽倶旮?鞝勳棎 鞎旐樃頇旐晿瓿?欤缄赴鞝侅溂搿?氤奠洂鞚?韰岇姢韸疙晿靹胳殧.

## 霐旍姢韥?鞎旐樃頇?

臧€電ロ晿氅?LUKS, BitLocker, FileVault 霕愲姅 provider-managed disk encryption鞚?靷毄頃橃劯鞖? VPS 瓿店笁鞛愱皜 root disk毳?鞎旐樃頇旐暊 靾?鞐嗠嫟氅?鞎旐樃頇旊悳 offsite backups電?靹犿儩 靷暛鞚?鞎勲媹霛?頃勳垬鞛呺媹雼?

## Token Hygiene

鞛レ箻 bearer token鞚€ 鞚胳霅?靷毄 鞁?臧膘嫚霅橁碃, 90鞚?霃欖晥 鞙犿湸鞚措┐ 毵岆霅橂┌, 臧?token鞐愲姅 365鞚检潣 鞝堧寑 靾橂獏鞚?鞛堦碃, 靷毄鞛?霕愲姅 甏€毽瀽臧€ 觳犿殞頃?靾?鞛堨姷雼堧嫟. 毵岆霅橁卑雮?觳犿殞霅?霑岅箤歆€ 頇滌劚 token鞚?鞛愱博 歃濍獏鞙茧 旆笁頃橃劯鞖?

Obsidian鞚€ 頂岆煬攴胳澑鞚?頇滌劚 token, deployment key, 搿滉犯鞚?靸來儨, 鞎堨爼鞝侅澑 鞛レ箻 ID毳?旮瓣赴 搿滌滑 鞝€鞛レ唽鞐?鞝€鞛ロ暕雼堧嫟. Vault-local 頂岆煬攴胳澑 `data.json`鞚€ 氙缄皭頃橃 鞎婌潃 靹れ爼瓿?霃欔赴頇?鞚鸽嵄鞀る 氤搓磤頃╇媹雼? 順勳灛 牍岆摐電?霃欔赴頇?鞚鸽嵄鞀?key鞐?deployment key毳?韽暔頃橃 鞎婌溂氅? 鞚挫爠 氩勳爠鞚?氙缄皭 鞝曤炒臧€ 韽暔霅?鞚鸽嵄鞀?頃鞚€ 雼れ潓 頂岆煬攴胳澑 雿办澊韯?鞊瓣赴 霑?韽愱赴霅╇媹雼? 靷毄鞛愳棎瓴?Obsidian 旮瓣赴 搿滌滑 鞝€鞛レ唽, 瓿奠湢 鞎勳勾鞚措笇, 鞁犽頃?靾?鞐嗠姅 霃欔赴頇?雽€靸? 韽夒 氚膘梾, 鞚挫爠 `data.json` 靷掣鞚?氤错樃頃橂澕瓿?鞎堧偞頃橃劯鞖? 鞚措煬頃?鞝€鞛レ唽臧€ 鞙犾稖霅橃棃鞚?靾?鞛堨溂氅?鞓來枼鞚?氚涭潃 鞛レ箻 token鞚?觳犿殞頃橁碃, deployment key臧€ 雲胳稖霅橃棃雼る┐ deployment key霃?甑愳泊頃橃劯鞖?

甓岇灔 氚╈嫕:

- Admin WebUI device pages鞐愳劀 攵勳嫟頃?鞛レ箻毳?觳犿殞頃╇媹雼?
- 頃?鞛レ箻毵?鞛冹柎氩勲牳雼る┐ 鞝勳泊 瓿勳爼 鞛劋鞝曤炒雼?頃措嫻 鞛レ箻 token 觳犿殞毳?鞖办劆頃╇媹雼?
- 鞛愱博 歃濍獏 旃暣臧€ 鞚橃嫭霅?霑?靷毄鞛?牍勲皜氩堩樃毳?rotate頃╇媹雼?
- 鞝曣赴 鞙犾氤挫垬 欷?鞓る灅霅?token瓿?觳犿殞霅?token鞚?瓴€韱犿暕雼堧嫟.

## 頇滊彊瓿?搿滉犯

PKV Sync電?霃欔赴頇? vault 靾橂獏 欤缄赴, 鞚疥赴 鞝勳毄 韮愳儔 頇滊彊鞚?user, vault, action, device name, file count, size, IP, User-Agent, details, timestamp鞕€ 頃粯 旮半頃╇媹雼? vault 靾橂獏 欤缄赴 頄夓棎電?Admin WebUI, 頂岆煬攴胳澑 霕愲姅 API 鞛戩梾鞚?`create_vault`鞕€ `delete_vault`臧€ 韽暔霅╇媹雼? Admin WebUI activity filters搿?users 霕愲姅 action types毳?頇曥澑頃?靾?鞛堨姷雼堧嫟.

鞎犿攲毽紑鞚挫厴瓿?reverse-proxy logs鞐愳劀 氚橂车霅橂姅 雼れ潓鞚?臧愳嫓頃橃劯鞖?

- `401`: invalid or expired credentials
- `403`: disabled account or forbidden operation
- `404`: rejected deployment key/User-Agent in production middleware
- `409`: sync head mismatch or duplicate resource
- `429`: login, registration, authenticated sync API, or MCP HTTP rate limit

## Release Hygiene

頂勲雿曥厴 鞐呹犯霠堨澊霌?鞝?

1. `CHANGELOG.md`毳?鞚届姷雼堧嫟.
2. release tag臧€ server, plugin, OpenAPI, Docker, docs versions鞕€ 鞚检箻頃橂姅歆€ 頇曥澑頃╇媹雼?
3. GitHub release鞐?Linux amd64, Linux arm64, Windows x64, plugin zip, `SHA256SUMS`臧€ 韽暔霅橃柎 鞛堧姅歆€ 頇曥澑頃╇媹雼?
4. GHCR image臧€ 頃措嫻 tag鞕€ `latest`鞐?臁挫灛頃橂姅歆€ 頇曥澑頃╇媹雼?
5. 順勳灛 data毳?氚膘梾頃╇媹雼?
6. 順勳灛 氚绊彫臧€ 0.x霛茧┐ 1.0 binary 霕愲姅 image毳?鞁滌瀾頃橁赴 鞝勳棎 [`upgrade-notes-v1.0.ko.md`](./upgrade-notes-v1.0.ko.md)毳?鞚届溂靹胳殧. 1.0鞚?旮办〈 0.x `metadata.db`鞐?鞐瓣舶頃橃 毵堨劯鞖?
7. 靸?binary搿?migrations毳?鞁ろ枆頃╇媹雼?

PKV Sync 1.0鞚€ 雼澕 v1 SQLite baseline鞚?靷毄頃╇媹雼? 鞚?baseline 鞚错泟 瓴岇嫓霅橂姅 1.x migrations電?旮办〈 1.x 氚绊彫鞐?雽€頃?append-only鞛呺媹雼?
