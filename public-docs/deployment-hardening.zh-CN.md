# PKV Sync 閮ㄧ讲鍔犲浐鎸囧崡

[English](./deployment-hardening.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./deployment-hardening.zh-Hant.md) | [鏃ユ湰瑾瀅(./deployment-hardening.ja.md) | [頃滉淡鞏碷(./deployment-hardening.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

鏈枃鍋囪閮ㄧ讲瀵硅薄鏄嚜宸便€佸搴€佸洟闃熸垨鍙俊鏈嬪弸浣跨敤鐨勫皬鍨嬭嚜鎵樼鏈嶅姟銆侾KV Sync 杩愮淮涓婃瘮杈冪畝鍗曪紝浣嗘湇鍔＄浼氫繚瀛樺彲璇荤殑浠撳簱鍐呭锛屽洜姝や富鏈哄拰澶囦唤鍗敓寰堥噸瑕併€?

## 濞佽儊妯″瀷

PKV Sync 涓嶆彁渚涚鍒扮鍔犲瘑銆備粨搴撳唴瀹瑰畨鍏ㄤ緷璧栧灞傛帶鍒讹細

1. HTTPS 浼犺緭鍔犲瘑
2. 閮ㄧ讲瀵嗛挜棰勮璇?
3. 鐢ㄦ埛鍚?瀵嗙爜鐧诲綍鍜屼娇鐢ㄦ椂缁湡鐨?bearer 璁惧 token
4. 鎸夌敤鎴峰拰绗旇搴撴墽琛屾巿鏉冩鏌?
5. Admin session 鍜?CSRF 淇濇姢
6. 鎿嶄綔绯荤粺鎴栦簯鍘傚晢纾佺洏鍔犲瘑
7. 鏈€灏忓寲鏆撮湶鏈嶅姟
8. 鍔犲瘑涓旂粡杩囨仮澶嶆祴璇曠殑澶囦唤

璇锋妸鏈嶅姟绔鐞嗗憳鍜屾湇鍔＄鏂囦欢绯荤粺瑙嗕负鍙互璁块棶浠撳簱鏄庢枃鍐呭鐨勫彲淇¤竟鐣屻€?

1.2.1 琛ヤ竵涔熸敹绱т簡鏆撮湶杈圭晫锛欸it HTTP Basic 澶辫触淇℃伅淇濇寔娉涘寲锛孧CP JSON 璇锋眰浣撲笂闄愪负 100 MiB锛宐lob 鍏冩暟鎹鏌ヤ細鎷掔粷绗﹀彿閾炬帴鐨?blob 璺緞锛岃€屼笉鏄窡闅忓畠浠€?

## 鎺ㄨ崘鎷撴墤

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

闄ら潪浣犳湁鏄庣‘鐨勯澶栫綉缁滄帶鍒跺眰锛屽惁鍒欎笉瑕佹妸 `pkvsyncd` 鐩存帴鏆撮湶鍒板叕缃戙€?

## 瀹夎鍓嶅噯澶?

鍑嗗锛?

- 鍩熷悕锛屼緥濡?`sync.example.com`
- 閫氳繃 `pkvsyncd genkey` 鐢熸垚鐨勯儴缃插瘑閽?
- `/etc/pkv-sync/config.toml`
- 鎸佷箙鍖栨暟鎹洰褰曪紝閫氬父鏄?`/var/lib/pkv-sync`
- 甯︽湁鏁?TLS 璇佷功鐨勫弽鍚戜唬鐞?

鏈嶅姟绔垎浜?URL 褰㈠紡濡備笅锛?

```text
https://sync.example.com/k_xxx/
```

璇蜂繚鎸佺瀵嗐€傞儴缃插瘑閽ユ槸 API 娴侀噺鐨勯璁よ瘉鍏ュ彛锛屼絾涓嶈兘鏇夸唬鐢ㄦ埛瀵嗙爜銆?

## 绯荤粺鐢ㄦ埛

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

灏?`config.toml` 鏀惧湪 `/etc/pkv-sync/config.toml`锛屽苟纭繚鍙湁鏈嶅姟鐢ㄦ埛鍜岀鐞嗗憳鍙互璇诲彇銆?

## 闃茬伀澧?

鍏稿瀷涓绘満鍙毚闇?SSH 鍜?HTTPS锛?

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

濡傛灉 Caddy 鎴栧叾浠?ACME HTTP-01 瀹㈡埛绔鐞嗚瘉涔︼紝杩橀渶瑕佸紑鏀?`80` 绔彛鐢ㄤ簬楠岃瘉鍜岃烦杞細

```bash
sudo ufw allow 80/tcp
```

瀹夸富鏈虹洿鎺ヨ繍琛屾椂锛岃 `pkvsyncd` 鍙洃鍚湰鏈猴細

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose 涓搴旂敤鐩戝惉瀹瑰櫒鎵€鏈夋帴鍙ｏ紱濡傛灉闇€瑕佸涓绘満璋冭瘯锛屽彧鎶婂涓绘満绔彛鍙戝竷鍒?localhost锛?

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose + Caddy

濡傛灉甯屾湜鐢?Caddy 鑷姩鐢宠鍜岀画鏈?HTTPS 璇佷功锛屼娇鐢ㄨ繖涓矾寰勩€?

1. 灏?DNS 鎸囧悜鏈嶅姟鍣細

   ```text
   sync.example.com A    <鏈嶅姟鍣?IPv4>
   sync.example.com AAAA <鏈嶅姟鍣?IPv6锛屽彲閫?
   ```

2. 鍦?`docker-compose.yml` 鍚岀洰褰曞垱寤?`config.toml`锛?

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 鏇挎崲涓?genkey 杈撳嚭
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

3. 鏇挎崲 `deploy/caddy/Caddyfile` 涓殑 `sync.example.com`銆?
4. 鍚姩锛?

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 鍏ㄦ柊鏁版嵁搴撻娆″惎鍔ㄥ悗锛屾墦寮€ setup wizard 鍒涘缓绗竴涓鐞嗗憳璐﹀彿锛?

   ```text
   https://sync.example.com/setup
   ```

   濡傛潯浠跺厑璁革紝璇锋妸 setup 闃舵鏀惧湪绉佺綉鎴栦复鏃跺弽鍚戜唬鐞?allowlist 鍚庡畬鎴愶紝瀹屾垚鍚庣珛鍒绘敹绱у叕缃戣闂€傛棩甯哥鐞嗗憳鐧诲綍浣跨敤 `https://sync.example.com/admin/login`銆?

澶囦唤 `./data`銆乣config.toml` 鍜?Caddy 鐨勫懡鍚嶅嵎銆?

鍗囩骇锛?

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

浠〃鐩橀粯璁ゆ瘡 24 灏忔椂妫€鏌ヤ竴娆?GitHub release锛涘彂鐜版柊鐗堟湰鏃朵細鏄剧ず鎻愮ず銆傚叏鏂版暟鎹簱棣栨鍚姩鏃讹紝`enabled` 鍜?`interval_seconds` 浼氬啓鍏ヨ繍琛屾椂璁剧疆锛涗箣鍚庡彲鍦?Admin WebUI Settings 涓慨鏀癸紝鏃犻渶閲嶅惎銆傛簮浠撳簱浠嶄繚鐣欎负闈欐€?`config.toml` 瀛楁锛屼緵绂荤嚎闀滃儚閮ㄧ讲浣跨敤锛?

```toml
[update_check]
enabled = true                          # 浠呬綔涓洪娆″惎鍔ㄧ瀛?
interval_seconds = 86400                # 浠呬綔涓洪娆″惎鍔ㄧ瀛?
repo = "cyberkurry/pkv-sync"            # 闈欐€佹煡璇㈢殑 GitHub 浠撳簱
```

鑻ヨ璁╃绾夸富鏈哄湪鍒濆鍖栧悗淇濇寔瀹夐潤锛岃鍦?Admin WebUI 杩愯鏃惰缃腑鍏抽棴鏇存柊妫€鏌ワ紝鎴栫敤 `enabled = false` 浣滀负鍏ㄦ柊閮ㄧ讲鐨勫垵濮嬬瀛愩€?

## public_host(admin POST 蹇呭)

鎶?`[server].public_host` 璁剧疆涓鸿繍缁村疄闄呰闂?admin 闈㈡澘浣跨敤鐨勫閮ㄤ富鏈哄悕锛堜笉鍚崗璁紝蹇呰鏃跺惈绔彛锛夛紝渚嬪 `sync.example.com` 鎴?`pkv.local:8443`銆俛dmin CSRF 妫€鏌ヤ緷鎹鍊艰绠楁湡鏈?Origin銆傞厤缃?`public_host` 鍚庯紝鏈熸湜 Origin 鍥哄畾涓?`https://<public_host>`锛涘弽鍚戜唬鐞嗕紶鍏ョ殑 `X-Forwarded-Proto` 涓嶄細鎶?admin CSRF 鏍￠獙闄嶇骇鍒板悗绔?HTTP銆?

濡傛灉 `public_host` 鐣欑┖,鎵€鏈?admin POST 閮戒細琚嫆缁?杩斿洖 `403 csrf validation failed`,骞舵墦涓€鏉?`tracing::warn` 鏃ュ織銆傝繖鏄?*鏈夋剰鐨?fail-closed 琛屼负**:鍙︿竴绉嶅仛娉?鍥為€€璇锋眰鑷甫鐨?`Host` 澶?浼氭妸閴存潈鑰﹀悎鍒版敾鍑昏€呭彲褰卞搷鐨?header,涓斿湪浠ｇ悊杞彂涓嶄竴鑷寸殑 Host 鏃朵細鍑洪敊銆?

`public_host` 鍚屾椂椹卞姩:

- 鐢熶骇椋庢牸鐨?admin cookie(璁剧疆鍚庡惎鐢?`Secure`銆乣SameSite=Strict`)
- admin "鍒嗕韩鏈嶅姟绔?URL" 閾炬帴浣跨敤 `https://` 鍓嶇紑
- `/api/plugin-manifest` 杩斿洖鐨勬彃浠惰祫婧?URL 浣跨敤 `https://` 澶栭儴涓绘満

鎻掍欢娓呭崟 URL 鐢熸垚涓嶄細淇′换瀹㈡埛绔紶鍏ョ殑 `X-Forwarded-Proto`銆傜敓浜х幆澧冭璁剧疆 `public_host`锛岃繖鏍锋彃浠惰嚜鏇存柊鎷垮埌鐨勮祫婧?URL 鎵嶄細绋冲畾鎸囧悜鐪熷疄澶栭儴涓绘満銆?

瀵?SSE 鏉ヨ,璇ヨ缃篃鑳藉府鍙嶅悜浠ｇ悊璇嗗埆闀胯繛鎺ヤ簨浠舵祦鑰屼笉鏄櫘閫氱煭璇锋眰銆?

## 瀹夊叏鍝嶅簲澶?

PKV Sync 浼氬湪鐢熶骇鏈嶅姟绔爤閲屾坊鍔犺繖浜涘搷搴斿ご:

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- 鍦ㄩ厤缃簡 `public_host` 鏃舵坊鍔?`Strict-Transport-Security: max-age=31536000; includeSubDomains`

璇疯 TLS 缁堟鍜?`public_host` 淇濇寔涓€鑷淬€傚彧鏈夊綋鏈嶅姟绔閰嶇疆涓?HTTPS 瀵瑰鍙戝竷鏃讹紝鎵嶄細鍙戦€?HSTS銆?

### 鍏充簬绔埌绔姞瀵?

PKV Sync 1.0 涓嶆槸绔埌绔姞瀵嗙殑锛氭湇鍔＄绠＄悊鍛樹互鍙婂叿澶囨湇鍔＄鏂囦欢绯荤粺璁块棶鏉冮檺鐨勪汉閮藉彲浠ヨ鍙栧悓姝ョ殑浠撳簱鍐呭銆傚師鐢熸寜绗旇搴?E2EE 鍒楀湪 1.x 璺嚎鍥句腑銆傚鏋滃綋鍓嶉渶瑕佸鏈嶅姟绔繚瀵嗭紝璇锋寜 [`git-crypt-howto.md`](./git-crypt-howto.md) 浣滀负杩囨浮鏈熺殑鎸夌瑪璁板簱鍔犲瘑灞傘€傝妯″紡涓嬫枃浠跺悕浠嶅鏈嶅姟绔彲瑙侊紝鍙湁鏂囦欢鍐呭鍦ㄥ鎴风鍔犲瘑銆?

## 鍙嶅悜浠ｇ悊娉ㄦ剰浜嬮」

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

浠撳簱鎻愪緵浜?`deploy/nginx/pkv-sync.conf`銆傚畠浼氭妸 HTTP 璺宠浆鍒?HTTPS锛岃缃?`client_max_body_size 110m`锛屾坊鍔犳爣鍑嗘祻瑙堝櫒鍔犲浐 header锛屽苟杞彂 PKV Sync 鐢ㄤ簬 Host 鍜屽鎴风 IP 澶勭悊鐨?header銆?

鏈€灏忕粨鏋勶細

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

浠撳簱鍦?`deploy/traefik/docker-compose.traefik.yml` 鎻愪緵浜?Traefik 绀轰緥銆傝鎶?`trusted_proxies` 璁剧疆涓?Traefik 浣跨敤鐨?Docker 缃戠粶 CIDR锛屽苟鏇挎崲绀轰緥鍩熷悕鍜?ACME 閭銆?

## trusted_proxies

鍙俊浠绘潵鑷弽鍚戜唬鐞嗙殑 `X-Forwarded-For`銆傚鏋滀唬鐞嗗拰搴旂敤杩愯鍦ㄥ悓涓€鍙颁富鏈猴細

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

濡傛灉浣跨敤 Docker bridge 缃戠粶锛?

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

涓嶈鍔犲叆瀹芥硾鍏綉缃戞銆傚鏋滃鎴风鍙互浼€?`X-Forwarded-For`锛岄檺娴佸拰瀹¤鏁版嵁閮戒細鍙樺急銆?

## 杩愯鏃跺畨鍏ㄨ缃?

浠?Admin WebUI 妫€鏌ヨ繖浜涜缃細

- 娉ㄥ唽妯″紡锛氱鏈夐儴缃插缓璁繚鎸?`disabled` 鎴?`invite_only`銆?
- 鐧诲綍闄愭祦闃堝€笺€佺獥鍙ｅ拰閿佸畾鏃堕暱銆?
- 璁よ瘉鍚屾 API 闄愭祦锛氭寜璺敱銆佹柟娉曘€佸鎴风 IP 鍜?bearer 璁惧 token 鍒嗘《锛屾瘡 60 绉掓渶澶?600 娆¤姹傘€?
- 澶辫触 bearer token 璁よ瘉闄愭祦锛氭寜瀹㈡埛绔?IP 鍒嗘《锛屾瘡 60 绉掓渶澶?120 娆″け璐ュ皾璇曘€?
- 鏈€澶ф枃浠跺ぇ灏忥紝榛樿 `100 MiB`銆?
- 鏀寔鐨勬枃鏈墿灞曞悕銆?
- 鏃跺尯锛岄粯璁?`Asia/Shanghai`銆?

娉ㄥ唽銆佺櫥褰曞け璐ュ拰澶辫触 bearer token 璁よ瘉閮戒細琚檺娴併€係etup銆佸叕寮€娉ㄥ唽銆佺敤鎴疯嚜鍔╀慨鏀瑰瘑鐮侊紝浠ュ強绠＄悊鍛樺垱寤烘垨閲嶇疆鐨勫瘑鐮侀兘蹇呴』鑷冲皯 12 涓瓧绗︼紝骞跺寘鍚ぇ鍐欏瓧姣嶃€佸皬鍐欏瓧姣嶅拰鏁板瓧锛汣LI 鍒涘缓鐨勭敤鎴蜂篃浠嶅簲浣跨敤寮哄瘑鐮併€?

Blob 涓婁紶璇锋眰浣撳彈 `max_file_size` 闄愬埗锛屽苟涓斿缁堜細琚‖ blob 涓婇檺澶逛綇锛堢敓浜х幆澧?`512 MiB`锛夈€備富 SSE 娴佸湪淇濇寔鎵撳紑鏃朵細澶嶆煡 bearer token锛汳CP 璇诲彇鍜屾悳绱㈠伐鍏蜂篃鏈夊搷搴斿ぇ灏忎笌鎬绘悳绱㈤绠楋紝閬垮厤澶у瀷绗旇搴撹灞曞紑鎴愭棤鐣?JSON 鍝嶅簲銆?

Pull/tree 閬嶅巻鍜?rollback 鍙揪鎬ф鏌ラ兘鏈夎竟鐣岋紱琚綋鍓嶅悓姝ヨ繃婊よ鍒欐嫆缁濈殑璺緞浼氫粠璇诲彇銆佸巻鍙层€乨iff 鍜?commit-list 鐣岄潰闅愯棌銆?

## 鎸囨爣绔偣

`/metrics` 鍙湁鍦ㄨ繍琛屾椂璁剧疆涓惎鐢?`enable_metrics=true` 鍚庢墠鍙敤銆傚嵆浣垮惎鐢紝瀹冧篃蹇呴』鍚屾椂閫氳繃閮ㄧ讲瀵嗛挜涓棿浠躲€佹彃浠?User-Agent guard 鍜岀鐞嗗憳 bearer token锛屼笉搴旂粫杩囧弽鍚戜唬鐞嗘垨棰濆鏆撮湶鍒板叕缃戙€?

## 澶囦唤

涓€璧峰浠斤細

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

澶嶅埗鏁版嵁搴撴椂浣跨敤 SQLite 鍦ㄧ嚎澶囦唤锛屾垨鍏堝仠姝㈡湇鍔°€傚敖閲忚鏁版嵁搴撱€丟it 绗旇搴撳拰 blobs 鏉ヨ嚜鍚屼竴鏃堕棿鐐广€?

鍐呯疆 backup/restore helper 涓嶄細璺熼殢 symlink銆俙vaults/` 鎴?`blobs/` 涓嬬殑 symlink 鏉＄洰浼氬湪澶囦唤鏃惰烦杩囷紝鍦ㄦ仮澶嶆竻鐞嗘椂鍙Щ闄ら摼鎺ユ湰韬紝涓嶄細瑙︾閾炬帴鐩爣銆?

restic 绀轰緥锛?

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

澶囦唤绂诲紑鏈哄櫒鍓嶅簲鍏堝姞瀵嗭紝骞跺畾鏈熸祴璇曟仮澶嶃€?

## 纾佺洏鍔犲瘑

灏介噺浣跨敤 LUKS銆丅itLocker銆丗ileVault 鎴栦簯鍘傚晢鎵樼纾佺洏鍔犲瘑銆傚鏋?VPS 鎻愪緵鍟嗘棤娉曞姞瀵嗘牴纾佺洏锛屽姞瀵嗙绾垮浠藉氨涓嶆槸鍙€夐」锛岃€屾槸蹇呰椤广€?

## Token 绠＄悊

璁惧 bearer token 浼氬湪璁よ瘉璇锋眰鏃剁画鏈燂紝杩炵画 90 澶╂湭浣跨敤鎵嶄細杩囨湡锛屽崟涓?token 鏈€闀挎湁鏁?365 澶╋紝涔熷彲浠ョ敱鐢ㄦ埛鎴栫鐞嗗憳鎾ら攢銆傚湪杩囨湡鎴栨挙閿€鍓嶏紝璇锋妸娲昏穬 token 褰撲綔鍑嵁澶勭悊銆?

Obsidian 浼氭妸鎻掍欢鐨勬椿璺?token銆侀儴缃插瘑閽ャ€佺櫥褰曠姸鎬佸拰绋冲畾璁惧韬唤淇濆瓨鍦ㄨ澶囨湰鍦板瓨鍌ㄤ腑銆傜瑪璁板簱鏈湴鎻掍欢 `data.json` 鍙繚鐣欓潪鏁忔劅鍋忓ソ鍜屽悓姝ョ储寮曪紱褰撳墠鐗堟湰鐨勫悓姝ョ储寮?key 涓嶅啀鍖呭惈閮ㄧ讲瀵嗛挜锛屾棫鐗堝甫鏁忔劅淇℃伅鐨勭储寮曢」浼氬湪涓嬫鍐欏叆鎻掍欢鏁版嵁鏃惰涓㈠純銆傝鎻愰啋鐢ㄦ埛淇濇姢 Obsidian 璁惧鏈湴瀛樺偍銆佸叡浜帇缂╁寘銆佷笉鍙俊鍚屾鐩爣銆佹槑鏂囧浠戒互鍙婃棫鐗堟湰鐣欎笅鐨?`data.json` 鍓湰銆傚鏋滆繖浜涘瓨鍌ㄥ彲鑳芥硠闇诧紝璇锋挙閿€鍙楀奖鍝嶇殑璁惧 token锛涘鏋滈儴缃插瘑閽ユ浘缁忔毚闇诧紝璇疯疆鎹㈤儴缃插瘑閽ャ€?

寤鸿锛?

- 浠?Admin WebUI 璁惧椤甸潰鎾ら攢涓㈠け璁惧銆?
- 濡傛灉鍙涪澶卞崟鍙拌澶囷紝浼樺厛鎾ら攢璇ヨ澶?token锛岃€屼笉鏄噸缃暣涓处鍙枫€?
- 鎬€鐤戣处鍙峰嚟鎹硠闇叉椂鍐嶈疆鎹㈢敤鎴峰瘑鐮併€?
- 渚嬭缁存姢鏃舵鏌ユ棫 token 鍜屽凡鎾ら攢 token銆?

## 娲诲姩鍜屾棩蹇?

PKV Sync 浼氳褰?push銆乸ull銆乧reate_vault 鍜?delete_vault 娲诲姩锛屽寘鎷敤鎴枫€佺瑪璁板簱銆佽澶囧悕銆佹枃浠舵暟銆佸ぇ灏忋€両P銆乁ser-Agent銆佽鎯呭拰鏃堕棿鎴炽€俙create_vault` 鍜?`delete_vault` 鏉ヨ嚜绠＄悊闈㈡澘銆佹彃浠跺拰 API 鐨勭瑪璁板簱鍒涘缓锛忓垹闄ゆ搷浣溿€傚彲浠ョ敤 Admin WebUI 鐨勬椿鍔ㄧ瓫閫夋鏌ョ敤鎴锋垨鎿嶄綔绫诲瀷銆?

鍏虫敞搴旂敤鍜屽弽鍚戜唬鐞嗘棩蹇椾腑閲嶅鍑虹幇鐨勶細

- `401`锛氬嚟鎹棤鏁堟垨宸茶繃鏈?
- `403`锛氳处鍙风鐢ㄦ垨鎿嶄綔琚姝?
- `404`锛氱敓浜т腑闂翠欢鎷掔粷閮ㄧ讲瀵嗛挜鎴?User-Agent
- `409`锛氬悓姝?head 涓嶅尮閰嶆垨璧勬簮閲嶅
- `429`锛氱櫥褰曘€佹敞鍐屻€佽璇佸悓姝?API 鎴?MCP HTTP 闄愭祦

## 鍙戝竷鍗敓

鐢熶骇鍗囩骇鍓嶏細

1. 闃呰 `CHANGELOG.md`銆?
2. 纭 release tag 涓庢湇鍔＄銆佹彃浠躲€丱penAPI銆丏ocker 鍜屾枃妗ｇ増鏈竴鑷淬€?
3. 妫€鏌?GitHub release 鍖呭惈 Linux amd64銆丩inux arm64銆乄indows x64銆佹彃浠?zip 鍜?`SHA256SUMS`銆?
4. 纭 GHCR 闀滃儚瀛樺湪瀵瑰簲 tag 鍜?`latest`銆?
5. 澶囦唤褰撳墠鏁版嵁銆?
6. 濡傛灉褰撳墠閮ㄧ讲鏄?0.x锛屽惎鍔?1.0 浜岃繘鍒舵垨闀滃儚鍓嶅厛闃呰 [`upgrade-notes-v1.0.zh-CN.md`](./upgrade-notes-v1.0.zh-CN.md)銆備笉瑕佹妸 1.0 鐩存帴鎸囧悜宸叉湁鐨?0.x `metadata.db`銆?
7. 浜岃繘鍒堕儴缃插厛杩愯 `pkvsyncd upgrade --dry-run` 棰勮 release 璧勪骇锛屽啀杩愯 `pkvsyncd upgrade --yes` 涓嬭浇骞舵牎楠屽綋鍓嶄簩杩涘埗鏃佽竟鐨?`pkvsyncd.new`銆傚彧鏈夋牎楠岄€氳繃鍚庯紝鎵嶅仠姝㈡湇鍔″苟鏇挎崲浜岃繘鍒躲€?
8. Docker 鎴?Kubernetes 閮ㄧ讲搴旀媺鍙栨垨淇敼闀滃儚 tag 骞堕噸鍚湇鍔℃垨 rollout锛屼笉瑕佸湪瀹瑰櫒鍐呮浛鎹簩杩涘埗銆?
9. 鐢ㄦ柊浜岃繘鍒舵垨鏂伴暅鍍忚繍琛?migrations銆?

PKV Sync 1.0 浣跨敤鍗曚釜 v1 SQLite 鍩虹嚎銆傚湪杩欐鍩虹嚎涔嬪悗锛屽凡鍙戝竷鐨?1.x migration 瀵瑰凡鏈?1.x 閮ㄧ讲淇濇寔杩藉姞寮忋€?
