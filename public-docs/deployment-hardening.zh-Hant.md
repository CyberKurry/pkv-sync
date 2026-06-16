# PKV Sync 閮ㄧ讲鍔犲浐鎸囧崡

[English](./deployment-hardening.md) | [绠€浣撲腑鏂嘳(./deployment-hardening.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./deployment-hardening.ja.md) | [頃滉淡鞏碷(./deployment-hardening.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

鏈枃鍋囪ō閮ㄧ讲灏嶈薄鏄嚜宸便€佸搴€佸湗闅婃垨鍙俊鏈嬪弸浣跨敤鐨勫皬鍨嬭嚜瑷楃鏈嶅嫏銆侾KV Sync 閬嬬董涓婃瘮杓冪啊鍠紝浣嗘湇鍕欑鏈冧繚瀛樺彲璁€鐨勫€夊韩鍏у锛屽洜姝や富姗熷拰鍌欎唤琛涚敓寰堥噸瑕併€?

## 濞佽剠妯″瀷

PKV Sync 涓嶆彁渚涚鍒扮鍔犲瘑銆備繚璀峰€夊韩鍏у渚濊炒澶氬堡鎺у埗锛?

1. HTTPS 鍌宠几鍔犲瘑
2. 閮ㄧ讲閲戦懓闋愯獚璀?
3. 浣跨敤鑰呭悕绋?瀵嗙⒓鐧诲叆鍜屼娇鐢ㄦ檪绾屾湡鐨?bearer 瑁濈疆 token
4. 鎸変娇鐢ㄨ€呭拰绛嗚搴煼琛屾巿娆婃鏌?
5. Admin session 鍜?CSRF 淇濊
6. 浣滄キ绯荤当鎴栭洸绔緵鎳夊晢纾佺鍔犲瘑
7. 鏈€灏忓寲鏆撮湶鏈嶅嫏
8. 鍔犲瘑涓旂稉閬庢仮寰╂脯瑭︾殑鍌欎唤

璜嬫妸鏈嶅嫏绔鐞嗗摗鍜屾湇鍕欑妾旀绯荤当瑕栫偤鍙互瀛樺彇鍊夊韩鏄庢枃鍏у鐨勫彲淇￠倞鐣屻€?

1.2.1 淇涔熸敹绶婁簡鏆撮湶閭婄晫锛欸it HTTP Basic 澶辨晽璩囪▕淇濇寔娉涘寲锛孧CP JSON request body 涓婇檺鐐?100 MiB锛宐lob 涓辜璩囨枡妾㈡煡鏈冩嫆绲曠铏熼€ｇ祼鐨?blob 璺緫锛岃€屼笉鏄窡闅ㄥ畠鍊戙€?

## 鎺ㄨ枽鎷撴挷

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

闄ら潪浣犳湁鏄庣⒑鐨勯澶栫恫璺帶鍒跺堡锛屽惁鍓囦笉瑕佹妸 `pkvsyncd` 鐩存帴鏆撮湶鍒板叕缍层€?

## 瀹夎鍓嶆簴鍌?

婧栧倷锛?

- 缍插煙鍚嶇ū锛屼緥濡?`sync.example.com`
- 閫忛亷 `pkvsyncd genkey` 鐢㈢敓鐨勯儴缃查噾閼?
- `/etc/pkv-sync/config.toml`
- 鎸佷箙鍖栬硣鏂欑洰閷勶紝閫氬父鏄?`/var/lib/pkv-sync`
- 甯舵湁鏈夋晥 TLS 鎲戣瓑鐨勫弽鍚戜唬鐞?

鏈嶅嫏绔垎浜?URL 褰㈠紡濡備笅锛?

```text
https://sync.example.com/k_xxx/
```

璜嬩繚鎸佺瀵嗐€傞儴缃查噾閼版槸 API 娴侀噺鐨勯爯瑾嶈瓑鍏ュ彛锛屼絾涓嶈兘鍙栦唬浣跨敤鑰呭瘑纰笺€?

## 绯荤当浣跨敤鑰?

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

灏?`config.toml` 鏀惧湪 `/etc/pkv-sync/config.toml`锛屼甫纰轰繚鍙湁鏈嶅嫏浣跨敤鑰呭拰绠＄悊鍝″彲浠ヨ畝鍙栥€?

## 闃茬伀鐗?

鍏稿瀷涓绘鍙毚闇?SSH 鍜?HTTPS锛?

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

濡傛灉 Caddy 鎴栧叾浠?ACME HTTP-01 鐢ㄦ埗绔鐞嗘啈璀夛紝閭勯渶瑕侀枊鏀?`80` 閫ｆ帴鍩犵敤鏂奸璀夊拰璺宠綁娴侀噺锛?

```bash
sudo ufw allow 80/tcp
```

鍦ㄥ涓绘鐩存帴鍩疯鏅傦紝璁?`pkvsyncd` 鍙洠鑱芥湰姗燂細

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose 涓畵鎳夌敤鐩ｈ伣瀹瑰櫒鎵€鏈変粙闈紱濡傛灉闇€瑕佸涓绘鍋甸尟锛屽彧鎶婂涓绘閫ｆ帴鍩犵櫦甯冨埌 localhost锛?

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose + Caddy

濡傛灉甯屾湜鐢?Caddy 鑷嫊鐢宠珛鍜岀簩鏈?HTTPS 鎲戣瓑锛屼娇鐢ㄩ€欏€嬭矾寰戙€?

1. 灏?DNS 鎸囧悜浼烘湇鍣細

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. 鍦?`docker-compose.yml` 鍚岀洰閷勫缓绔?`config.toml`锛?

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 鏇挎彌鐐?genkey 杓稿嚭
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

3. 鏇挎彌 `deploy/caddy/Caddyfile` 涓殑 `sync.example.com`銆?
4. 鍟熷嫊锛?

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 鍏ㄦ柊璩囨枡搴娆″暉鍕曞緦锛岄枊鍟?setup wizard 寤虹珛绗竴鍊嬬鐞嗗摗甯宠櫉锛?

   ```text
   https://sync.example.com/setup
   ```

   濡傛浠跺厑瑷憋紝璜嬫妸 setup 闅庢鏀惧湪绉佹湁缍茶矾鎴栬嚚鏅傚弽鍚戜唬鐞?allowlist 寰屽畬鎴愶紝瀹屾垚寰岀珛鍒绘敹绶婂叕闁嬪瓨鍙栥€傛棩甯哥鐞嗗摗鐧诲叆浣跨敤 `https://sync.example.com/admin/login`銆?

鍌欎唤 `./data`銆乣config.toml` 鍜?Caddy 鐨勫懡鍚嶅嵎銆?

鍗囩礆锛?

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

鍎€琛ㄦ澘姣?24 灏忔檪妾㈡煡涓€娆?GitHub releases锛岀櫦鐝捐純鏂扮殑 PKV Sync 鐗堟湰鏅傛渻椤ず姗箙銆傚叏鏂拌硣鏂欏韩棣栨鍟熷嫊鏅傦紝`enabled` 鍜?`interval_seconds` 鏈冨鍏ュ煼琛岄殠娈佃ō瀹氾紱涔嬪緦鍙湪 Admin WebUI Settings 涓慨鏀癸紝鐒￠渶閲嶅暉銆備締婧愬€夊韩浠嶄繚鐣欑偤闈滄厠 `config.toml` 娆勪綅锛屼緵闆㈢窔閺″儚閮ㄧ讲浣跨敤锛?

```toml
[update_check]
enabled = true                          # 鍍呬綔鐐洪娆″暉鍕曠ó瀛?
interval_seconds = 86400                # 鍍呬綔鐐洪娆″暉鍕曠ó瀛?
repo = "cyberkurry/pkv-sync"            # 闈滄厠鏌ヨ鐨?GitHub 鍊夊韩
```

鑻ヨ璁撻洟绶氫富姗熷湪鍒濆鍖栧緦淇濇寔瀹夐潨锛岃珛鍦?Admin WebUI 鍩疯闅庢瑷畾涓棞闁夋洿鏂版鏌ワ紝鎴栫敤 `enabled = false` 浣滅偤鍏ㄦ柊閮ㄧ讲鐨勫垵濮嬬ó瀛愩€?

## public_host锛坅dmin POST 蹇呭倷锛?

灏?`[server].public_host` 瑷畾鐐洪亱缍闅涘瓨鍙?admin 闈㈡澘浣跨敤鐨勫閮ㄤ富姗熷悕绋憋紙涓嶅惈鍗斿畾锛屽繀瑕佹檪鍚€ｆ帴鍩狅級锛屼緥濡?`sync.example.com` 鎴?`pkv.local:8443`銆俛dmin CSRF 妾㈡煡渚濇摎瑭插€艰▓绠楁湡鏈?Origin銆傝ō瀹?`public_host` 寰岋紝鏈熸湜 Origin 鍥哄畾鐐?`https://<public_host>`锛涘弽鍚戜唬鐞嗗偝鍏ョ殑 `X-Forwarded-Proto` 涓嶆渻鎶?admin CSRF 鏍￠闄嶇礆鍒板緦绔?HTTP銆?

濡傛灉 `public_host` 鐣欑┖锛屾墍鏈?admin POST 閮芥渻琚嫆绲曪紝杩斿洖 `403 csrf validation failed`锛屼甫鎵撲竴姊?`tracing::warn` 鏃ヨ獙銆傞€欐槸鏈夋剰鐨?fail-closed 琛岀偤锛氬彟涓€绋仛娉曟槸鍥為€€璜嬫眰鑷付鐨?`Host` header锛屼絾鏈冩妸閼戞瑠鑰﹀悎鍒版敾鎿婅€呭彲褰遍熆鐨?header锛屼笖鍦ㄤ唬鐞嗚綁鐧间笉涓€鑷寸殑 Host 鏅傛渻鍑洪尟銆?

`public_host` 鍚屾檪椹呭嫊锛?

- 鐢熺敘棰ㄦ牸鐨?admin cookie锛堣ō瀹氬緦鍟熺敤 `Secure`銆乣SameSite=Strict`锛?
- admin 涓€宻hare server URL銆嶉€ｇ祼浣跨敤 `https://` 鍓嶇洞
- `/api/plugin-manifest` 杩斿洖鐨勫鎺涜硣婧?URL 浣跨敤 `https://` 澶栭儴涓绘

澶栨帥娓呭柈 URL 鐢㈢敓涓嶆渻淇′换鐢ㄦ埗绔偝鍏ョ殑 `X-Forwarded-Proto`銆傜敓鐢㈢挵澧冭珛瑷畾 `public_host`锛岄€欐ǎ澶栨帥鑷洿鏂版嬁鍒扮殑璩囨簮 URL 鎵嶆渻绌╁畾鎸囧悜鐪熷澶栭儴涓绘銆?

灏?SSE 渚嗚锛岃┎瑷畾涔熻兘骞姪鍙嶅悜浠ｇ悊璀樺垾闀烽€ｇ窔浜嬩欢娴佽€屼笉鏄櫘閫氱煭璜嬫眰銆?

## 瀹夊叏鍥炴噳妯欓牠

PKV Sync 鏈冨湪鐢熺敘鏈嶅嫏绔＇涓姞鍏ラ€欎簺鍥炴噳妯欓牠锛?

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy: default-src 'self'; base-uri 'self'; frame-ancestors 'none'; object-src 'none'; form-action 'self'; img-src 'self' data:; style-src 'self'`
- 鍦ㄨō瀹氫簡 `public_host` 鏅傚姞鍏?`Strict-Transport-Security: max-age=31536000; includeSubDomains`

璜嬭畵 TLS 绲傛鍜?`public_host` 淇濇寔涓€鑷淬€傚彧鏈夌暥鏈嶅嫏绔瑷畾鐐?HTTPS 灏嶅鐧煎竷鏅傦紝鎵嶆渻鐧奸€?HSTS銆?

### 闂滄柤绔埌绔姞瀵?

PKV Sync 1.0 涓嶆彁渚涚鍒扮鍔犲瘑锛氫己鏈嶅櫒绯荤当绠＄悊鍝′互鍙婁换浣曞彲瀛樺彇浼烘湇鍣ㄦ獢妗堢郴绲辩殑浜洪兘鑳借畝鍙栧凡鍚屾鐨勭瓎瑷樺韩鍏у銆傚師鐢熺殑鎸夌瓎瑷樺韩 E2EE 宸插垪鍏?1.x 瑕忓妰銆備粖澶╁氨闇€瑕佸皪浼烘湇鍣ㄤ繚瀵嗙殑缍亱鑰咃紝鍙緷 [`git-crypt-howto.md`](./git-crypt-howto.md) 濂楃敤鎸夌瓎瑷樺韩鐨勯亷娓℃€у姞瀵嗗堡銆傝┎妯″紡涓嬫獢鍚嶄粛灏嶄己鏈嶅櫒鍙锛屽彧鏈夋獢妗堝収瀹规渻鍦ㄧ敤鎴剁鍔犲瘑銆?

## 鍙嶅悜浠ｇ悊娉ㄦ剰浜嬮爡

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

鍊夊韩鎻愪緵浜?`deploy/nginx/pkv-sync.conf`銆傚畠鏈冩妸 HTTP 璺宠綁鍒?HTTPS锛岃ō瀹?`client_max_body_size 110m`锛屽姞鍏ユ婧栫€忚鍣ㄥ姞鍥?header锛屼甫杞夌櫦 PKV Sync 鐢ㄦ柤 Host 鍜岀敤鎴剁 IP 铏曠悊鐨?header銆?

鏈€灏忕祼妲嬶細

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

鍊夊韩鍦?`deploy/traefik/docker-compose.traefik.yml` 鎻愪緵浜?Traefik 绡勪緥銆傝珛灏?`trusted_proxies` 瑷畾鐐?Traefik 浣跨敤鐨?Docker 缍茶矾 CIDR锛屼甫鏇挎彌绡勪緥缍插煙鍜?ACME 闆诲瓙閮典欢銆?

## trusted_proxies

鍙俊浠讳締鑷弽鍚戜唬鐞嗙殑 `X-Forwarded-For`銆傚鏋滀唬鐞嗗拰鎳夌敤鍩疯鍦ㄥ悓涓€鍙颁富姗燂細

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

濡傛灉浣跨敤 Docker bridge 缍茶矾锛?

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

涓嶈鍔犲叆瀵硾鍏恫缍叉銆傚鏋滅敤鎴剁鍙互鍋介€?`X-Forwarded-For`锛岄檺娴佸拰绋芥牳璩囨枡閮芥渻璁婂急銆?

## 鍩疯闅庢瀹夊叏瑷畾

寰?Admin WebUI 妾㈡煡閫欎簺瑷畾锛?

- 瑷诲唺妯″紡锛氱鏈夐儴缃插缓璀颁繚鎸?`disabled` 鎴?`invite_only`銆?
- 鐧诲叆闄愭祦闁惧€笺€佽绐楀拰閹栧畾鏅傞暦銆?
- 鏈€澶ф獢妗堝ぇ灏忥紝闋愯ō `100 MiB`銆?
- 鏀彺鐨勬枃瀛楀壇妾斿悕銆?
- 鏅傚崁锛岄爯瑷?`Asia/Shanghai`銆?

瑷诲唺鍜岀櫥鍏ュけ鏁楁渻琚檺娴併€係etup銆佸叕闁嬭ɑ鍐娿€佷娇鐢ㄨ€呰嚜鍔╀慨鏀瑰瘑纰硷紝浠ュ強绠＄悊鍝″缓绔嬫垨閲嶈ō鐨勫瘑纰奸兘蹇呴爤鑷冲皯 12 鍊嬪瓧鍏冿紝涓﹀寘鍚ぇ瀵瓧姣嶃€佸皬瀵瓧姣嶅拰鏁稿瓧锛汣LI 寤虹珛鐨勪娇鐢ㄨ€呬篃浠嶆噳浣跨敤寮峰瘑纰笺€?

瑾嶈瓑鍚屾 API 璺敱涔熸寜璺敱銆佹柟娉曘€佺敤鎴剁 IP 鍜?bearer token 鍥哄畾瑕栫獥闄愭祦锛屾瘡 60 绉掓渶澶?600 娆¤珛姹傘€傚け鏁楃殑 bearer token 瑾嶈瓑鏈冨彟鎸夌敤鎴剁 IP 闄愭祦锛屾瘡 60 绉掓渶澶?120 娆″け鏁楀槜瑭︺€備繚鎸?`trusted_proxies` 婧栫⒑锛岃畵闄愭祦鍣ㄥ拰绋芥牳鏃ヨ獙鐪嬪埌鐪熷鐢ㄦ埗绔?IP銆?

Blob 涓婂偝璜嬫眰 body 鍙?`max_file_size` 闄愬埗锛屼甫涓斾竴寰嬫渻琚‖ blob 涓婇檺闄愬埗锛堢敓鐢㈢挵澧?`512 MiB`锛夈€備富 SSE 涓叉祦鍦ㄤ繚鎸侀枊鍟熸檪鏈冭鏌?bearer token锛汳CP 璁€鍙栧拰鎼滃皨宸ュ叿涔熸湁鍥炴噳澶у皬鑸囩附鎼滃皨闋愮畻锛岄伩鍏嶅ぇ鍨嬬瓎瑷樺韩琚睍闁嬫垚鐒＄晫 JSON 鍥炴噳銆?

Pull/tree 閬嶆鍜?rollback 鍙仈鎬ф鏌ラ兘鏈夐倞鐣岋紱琚洰鍓嶅悓姝ラ亷婵捐鍓囨嫆绲曠殑璺緫鏈冨緸璁€鍙栥€佹鍙层€乨iff 鍜?commit-list 浠嬮潰闅辫棌銆?

## Prometheus Metrics

`/metrics` 闋愯ō鍋滅敤銆傜暥 `enable_metrics` 鍩疯闅庢瑷畾鐐?true 鏅傦紝绔粸鏈冭繑鍥?Prometheus text exposition锛屼甫涓斾粛闇€瑕佹瘡鍊嬬敓鐢㈤枠闁€锛氶儴缃查噾閼颁腑浠嬭粺楂斻€佸鎺?User-Agent guard 鍜岀鐞嗗摗 bearer token銆?

瑷畾 scrape 鐢ㄦ埗绔偝閫?`X-PKVSync-Deployment-Key`銆佹帴鍙楃殑 PKV Sync User-Agent锛屼互鍙?`Authorization: Bearer <admin-token>`銆備笉瑕佹妸 metrics 鏆撮湶绲︽湭瑾嶈瓑缍茶矾銆?

## 鍌欎唤

涓€璧峰倷浠斤細

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

瑜囪＝璩囨枡搴檪浣跨敤 SQLite 绶氫笂鍌欎唤锛屾垨鍏堝仠姝㈡湇鍕欍€傜洝閲忚畵璩囨枡搴€丟it 绛嗚搴拰 blobs 渚嗚嚜鍚屼竴鏅傞枔榛炪€?

鍏у缓 backup/restore helper 涓嶆渻璺熼毃 symlink銆俙vaults/` 鎴?`blobs/` 涓嬬殑 symlink 姊濈洰鏈冨湪鍌欎唤鏅傝烦閬庯紝鍦ㄦ仮寰╂竻鐞嗘檪鍙Щ闄ら€ｇ祼鏈韩锛屼笉鏈冭Ц纰伴€ｇ祼鐩銆?

restic 绡勪緥锛?

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

鍌欎唤闆㈤枊姗熷櫒鍓嶆噳鍏堝姞瀵嗭紝涓﹀畾鏈熸脯瑭︽仮寰┿€?

## 纾佺鍔犲瘑

鐩￠噺浣跨敤 LUKS銆丅itLocker銆丗ileVault 鎴栭洸绔緵鎳夊晢瑷楃纾佺鍔犲瘑銆傚鏋?VPS 渚涙噳鍟嗙劇娉曞姞瀵嗘牴纾佺锛屽姞瀵嗛洟绶氬倷浠藉氨涓嶆槸鍙伕闋咃紝鑰屾槸蹇呰闋呫€?

## Token 琛涚敓

瑁濈疆 bearer token 鏈冨湪瑾嶈瓑浣跨敤鏅傜簩鏈燂紝閫ｇ簩 90 澶╂湭浣跨敤鎵嶆渻閬庢湡锛屽柈鍊?token 鏈€闀锋湁鏁?365 澶╋紝涔熷彲浠ョ敱浣跨敤鑰呮垨绠＄悊鍝℃挙閵枫€傚湪閬庢湡鎴栨挙閵峰墠锛岃珛鎶婃椿韬?token 鐣朵綔鎲戣瓑铏曠悊銆?

Obsidian 鏈冩妸澶栨帥鐨勬椿韬?token銆侀儴缃查噾閼般€佺櫥鍏ョ媭鎱嬪拰绌╁畾瑁濈疆韬垎淇濆瓨鍦ㄨ缃湰姗熷劜瀛樹腑銆傜瓎瑷樺韩鏈澶栨帥 `data.json` 鍙繚鐣欓潪鏁忔劅鍋忓ソ鍜屽悓姝ョ储寮曪紱鐩墠鐗堟湰鐨勫悓姝ョ储寮?key 涓嶅啀鍖呭惈閮ㄧ讲閲戦懓锛岃垔鐗堝付鏁忔劅璩囪▕鐨勭储寮曢爡鏈冨湪涓嬫瀵叆澶栨帥璩囨枡鏅傝涓熸銆傝珛鎻愰啋浣跨敤鑰呬繚璀?Obsidian 瑁濈疆鏈鍎插瓨銆佸垎浜绺寘銆佷笉鍙俊鍚屾鐩銆佹槑鏂囧倷浠戒互鍙婅垔鐗堟湰鐣欎笅鐨?`data.json` 鍓湰銆傚鏋滈€欎簺鍎插瓨鍙兘澶栨穿锛岃珛鎾ら姺鍙楀奖闊跨殑瑁濈疆 token锛涘鏋滈儴缃查噾閼版浘缍撴毚闇诧紝璜嬭吉鎻涢儴缃查噾閼般€?

寤鸿锛?

- 寰?Admin WebUI 瑁濈疆闋侀潰鎾ら姺閬哄け瑁濈疆銆?
- 濡傛灉鍙伜澶卞柈鍙拌缃紝鍎厛鎾ら姺瑭茶缃?token锛岃€屼笉鏄噸瑷暣鍊嬪赋铏熴€?
- 鎳风枒甯宠櫉鎲戣瓑娲╅湶鏅傚啀杓彌浣跨敤鑰呭瘑纰笺€?
- 渚嬭缍鏅傛鏌ヨ垔 token 鍜屽凡鎾ら姺 token銆?

## 娲诲嫊鍜屾棩瑾?

PKV Sync 鏈冭閷勫悓姝ャ€佺瓎瑷樺韩鐢熷懡閫辨湡鍜屽敮璁€鐎忚娲诲嫊锛屽寘鎷娇鐢ㄨ€呫€佺瓎瑷樺韩銆佸嫊浣溿€佽缃悕绋便€佹獢妗堟暩銆佸ぇ灏忋€両P銆乁ser-Agent銆佽┏鎯呭拰鏅傞枔鎴炽€傜瓎瑷樺韩鐢熷懡閫辨湡琛屽寘鎷締鑷?Admin WebUI銆佸鎺涙垨 API 鎿嶄綔鐨?`create_vault` 鍜?`delete_vault`銆傚彲鐢?Admin WebUI 鐨勬椿鍕曠閬告鏌ヤ娇鐢ㄨ€呮垨鍕曚綔椤炲瀷銆?

闂滄敞鎳夌敤鍜屽弽鍚戜唬鐞嗘棩瑾屼腑閲嶈鍑虹従鐨勶細

- `401`锛氭啈璀夌劇鏁堟垨宸查亷鏈?
- `403`锛氬赋铏熷仠鐢ㄦ垨鎿嶄綔琚姝?
- `404`锛氱敓鐢腑浠嬭粺楂旀嫆绲曢儴缃查噾閼版垨 User-Agent
- `409`锛氬悓姝?head 涓嶅尮閰嶆垨璩囨簮閲嶈
- `429`锛氱櫥鍏ャ€佽ɑ鍐娿€佽獚璀夊悓姝?API 鎴?MCP HTTP 闄愭祦

## 鐧煎竷琛涚敓

鐢熺敘鍗囩礆鍓嶏細

1. 闁辫畝 `CHANGELOG.md`銆?
2. 纰鸿獚 release tag 鑸囨湇鍕欑銆佸鎺涖€丱penAPI銆丏ocker 鍜屾枃浠剁増鏈竴鑷淬€?
3. 妾㈡煡 GitHub release 鍖呭惈 Linux amd64銆丩inux arm64銆乄indows x64銆佸鎺?zip 鍜?`SHA256SUMS`銆?
4. 纰鸿獚 GHCR 鏄犲儚瀛樺湪灏嶆噳 tag 鍜?`latest`銆?
5. 鍌欎唤鐩墠璩囨枡銆?
6. 濡傛灉鐩墠閮ㄧ讲鏄?0.x锛屽暉鍕?1.0 浜岄€蹭綅鎴栨槧鍍忓墠鍏堥柋璁€ [`upgrade-notes-v1.0.zh-Hant.md`](./upgrade-notes-v1.0.zh-Hant.md)銆備笉瑕佹妸 1.0 鐩存帴鎸囧悜鏃㈡湁鐨?0.x `metadata.db`銆?
7. 鐢ㄦ柊浜岄€蹭綅鍩疯 migrations銆?

PKV Sync 1.0 浣跨敤鍠竴 v1 SQLite 鍩虹窔銆傚湪閫欐鍩虹窔涔嬪緦锛屽凡鐧煎竷鐨?1.x migration 灏嶆棦鏈?1.x 閮ㄧ讲淇濇寔杩藉姞寮忋€?
