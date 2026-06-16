# PKV Sync

**鑷灦浣犵殑 Obsidian 绛嗚搴€?* PKV Sync 璺戝湪浣犺嚜宸辩殑浼烘湇鍣ㄤ笂锛屾妸鎵嬫銆佸钩鏉裤€佹姗熺殑 Obsidian 绛嗚搴繚鎸佸悓姝ャ€備竴浠戒簩閫蹭綅銆佷竴鍊?SQLite 璩囨枡搴€佹瘡鍊嬬瓎瑷樺韩涓€鍊?bare git 鍊夊韩鈥斺€斾笉鐢ㄥ彚闆嗐€佷笉鐢?S3銆佷笉鐢ㄤ换浣曡绠￠洸銆傝濂斤紝鎶?Obsidian 鎸囬亷鍘伙紝绛嗚灏卞悓姝ヤ簡銆?

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

鏂囦欢鐗堟湰锛歷1.4.4銆?

[English](./README.md) | [绠€浣撲腑鏂嘳(./README.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./README.ja.md) | [頃滉淡鞏碷(./README.ko.md)

## 鐗规€?

- **澶氫娇鐢ㄨ€呫€佸绛嗚搴?*鍚屾锛屼緷瑁濈疆绨界櫦 token锛屾瘡鍊嬬瓎瑷樺韩甯?push 閹栬垏鍐瓑閲嶈│銆?
- **鍗虫檪鎺ㄩ€?*銆傚皬淇敼閫忛亷 SSE 鍦ㄤ簽绉掔礆钀藉湴锛涜吉瑭㈠仛鐐哄厹搴曚繚闅€?
- **Git 鍗崇湡鐩?*銆傛瘡鍊嬬瓎瑷樺韩閮芥槸涓€鍊?bare git 鍊夊韩锛屽柈妾旀鍙层€乽nified diff銆佸柈妾旈倓鍘熼枊绠卞嵆鐢ㄢ€斺€斿鎺涚鍜岀鐞嗗緦鍙伴兘鑳界敤銆?
- **琛濈獊瀹夊叏**銆傚鎺涗笉鏈冮粯榛樿钃嬫湰鍦颁慨鏀癸紝琛濈獊鏈冧互 `.conflict-*` 妾旀鍛堢従锛屼竴閸点€屼繚鐣欐湰鍦般€嶆垨銆屾帯绱嶉仩绔€嶃€?
- **浜旇獮瑷€绠＄悊寰屽彴**锛圗nglish銆佺畝涓€佺箒涓€佹棩鏈獮銆來暅甑柎锛夛細浣跨敤鑰呫€佽缃?token銆佺瓎瑷樺韩銆侀個璜嬬⒓銆佹椿鍕曟棩瑾屻€乥lob 鍨冨溇鍥炴敹锛屼甫灏嶇牬澹炴€х殑绛嗚搴拰浣跨敤鑰呮搷浣滃綀鍑虹⒑瑾嶃€?
- **AI 鍙畝**銆侻CP 鍙€忛亷 stdio銆佺崹绔?Streamable HTTP锛屾垨 `pkvsyncd serve` 鍏у祵鐨?`/mcp` 璺敱鏆撮湶璁€瀵伐鍏枫€?
- **闋愯ō鏈夐倞鐣?*銆傜鐞嗗摗寤虹珛锛忛噸瑷瘑纰间娇鐢?setup 鍚岀礆寮峰瘑纰肩瓥鐣ワ紱token 鏄庢枃鍙睍绀轰竴娆★紱涓婂偝鍜?MCP 鍥炴噳閮芥湁澶у皬涓婇檺锛涘嵆鏅?SSE 涓叉祦鏈冭鏌ュ凡鎾ら姺 token銆?
- **鍒绘剰鍋氬緱鐒¤亰**銆傚柈涓€浜岄€蹭綅銆佸柈涓€ SQLite 涓辜璩囨枡搴€佹瘡搴竴鍊?bare git 鍊夈€佹瘡鍊嬮檮浠朵竴鍊嬪収瀹瑰畾鍧€ blob銆?

## 鐢?Docker Compose 蹇€熶笂鎵?

閫欐槸鎺ㄨ枽璺緫銆俙deploy/caddy/` 瑁＄殑 Caddy 閫忛亷 Let's Encrypt 鑷嫊绨界櫦 HTTPS锛汸KV Sync 鍦?compose 鍏х恫鐩ｈ伣 `127.0.0.1:6710`锛屽叕缍插畬鍏ㄧ湅涓嶅埌鏄庢枃 HTTP銆?

浣犻渶瑕侊細涓€鍊嬬恫鍩燂紙渚嬪 `sync.example.com`锛夛紝A锛廇AAA 瑷橀寗鎸囧悜浼烘湇鍣紱鍏恫鑳介€ｅ埌 `80` 鍜?`443` 閫ｆ帴鍩狅紙80 鐢ㄦ柤 ACME HTTP-01 椹楄瓑锛夈€?

1. 鐢㈢敓閮ㄧ讲閲戦懓锛?

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 鍦?`docker-compose.yml` 鏃佹斁涓€浠?`config.toml`锛?

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 鏇挎彌鐐?genkey 杓稿嚭
   public_host    = "sync.example.com"   # 蹇呭～锛岀鐞嗙 POST 鎵嶈兘閫?

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge 缍叉

   [mcp]
   embed_in_serve = false                # true 鏈冨湪鏈湇鍕欎笂鎺涜級 /mcp
   ```

3. 绶ㄨ集 `deploy/caddy/Caddyfile`锛屾妸 `sync.example.com` 鎻涙垚浣犵殑鐪熷缍插煙銆?

4. 鎶婃暣濂楁湇鍕欐媺璧蜂締锛?

   ```bash
   docker compose up -d
   ```

   鐎忚鍣ㄦ墦闁?`https://sync.example.com/setup`锛屽缓绔嬬涓€鍊嬬鐞嗗摗甯宠櫉銆?

5. 鍦?Obsidian 瑁℃妸 `pkv-sync-plugin.zip` 瑙ｅ鍒?`<vault>/.obsidian/plugins/pkv-sync/`锛屽暉鐢ㄥ鎺涳紝寰炵鐞嗗緦鍙拌瑁藉垎浜?URL 璨奸€插幓锛岀櫥鍏ユ垨瑷诲唺锛岄伕涓€鍊嬬瓎瑷樺韩銆?

涔嬪緦鍗囩礆灏辨槸 `docker compose pull && docker compose up -d`銆傚鏋滆鍘熺敓瀹夎銆佽鍙嶅悜浠ｇ悊锛圕addy锛廚ginx锛廡raefik锛夈€佷簡瑙?`public_host` 鐨勮獮缇┿€佸仛鍌欎唤閭勫師鎴栫纰熷姞瀵嗭紝璜嬬湅[閮ㄧ讲鍔犲浐鎸囧崡](./public-docs/deployment-hardening.zh-Hant.md)銆?

## MCP 閮ㄧ讲妯″紡

PKV Sync 鎻愪緵鍏╃ó MCP Streamable HTTP 閮ㄧ讲鏂瑰紡銆傚収宓屾ā寮忛渶瑕佹槑纰洪枊鍟燂細瑷畾 `[mcp].embed_in_serve = true` 寰岋紝`pkvsyncd serve` 鏈冨湪涓绘湇鍕欑鍙ｆ帥杓?`/mcp`锛屽京鐢ㄥ悓涓€濂?TLS 绲傛銆佸弽鍚戜唬鐞嗐€侀儴缃查噾閼板拰 bearer 娆婃潠鏍￠銆傜崹绔嬫ā寮忎繚鐣欏師鏈夊柈鐛ㄩ€茬▼锛歚pkvsyncd mcp --transport http --bind 127.0.0.1:6711`锛岄仼鍚堥殧闆?MCP銆佸皥鐢ㄧ洠鑱戒綅鍧€鎴栫崹绔嬫摯绺銆?

## Obsidian 澶栨帥

鏈湴妾旀灏辨槸鐪熺浉鈥斺€斿鎺涚洿鎺ヨ畝瀵綘纾佺涓婄殑 Obsidian 绛嗚搴紝涓嶅瓨鍦ㄤ唬鐞嗘獢妗堢郴绲遍偅绋澅瑗裤€傞潪鏁忔劅鐨勫鎺涜ō瀹氬拰鍚屾绱㈠紩淇濆瓨鍦?`<vault>/.obsidian/plugins/pkv-sync/data.json`锛涚櫥鍏ョ媭鎱嬨€佺暥鍓?bearer 瑁濈疆娆婃潠銆侀儴缃查噾閼板拰绌╁畾瑁濈疆韬垎淇濆瓨鍦?Obsidian 鐨勮缃湰姗熷劜瀛樹腑銆傝珛鎶?Obsidian 瑁濈疆鏈鍎插瓨銆佹槑鏂囧倷浠戒互鍙婅垔鐗堟湰鐣欎笅鐨勫鎺?`data.json` 鍓湰鐣舵垚鏁忔劅璩囨枡銆傝缃瑠鏉栧湪浣跨敤鏅傛渻鑷嫊绾屾湡锛?0 澶╃劇娲诲嫊寰屽け鏁堬紝涓斿柈鍊嬫瑠鏉栨渶闀锋湁鏁?365 澶╋紱鍦ㄥ悓涓€瑁濈疆閲嶆柊鐧诲叆鏈冩彌鎺夎垔娆婃潠銆?

鏃ュ父浣跨敤鈥斺€斿懡浠ら潰鏉裤€佹獢妗堟鍙层€佷甫鎺?diff銆佽绐佽В姹恒€乣.obsidian` 閬告搰鎬у悓姝ャ€佽缃鐞嗐€佸鎺涜嚜鏇存柊鈥斺€旈兘瀵湪[浣跨敤鑰呮墜鍐奭(./public-docs/user-manual.zh-Hant.md)瑁°€?

## 闂滄柤鍔犲瘑

PKV Sync 1.0 **鏆笉**鎻愪緵鍘熺敓绔埌绔姞瀵嗏€斺€斾己鏈嶅櫒鑳借畝鍒扮瓎瑷樺収瀹广€傚師鐢熺殑鎸夊韩 E2EE 鍦?1.x 璺窔鍦栦笂锛屾渻浠ュ彲閬告ā寮忎笂绶氾紝鍥犵偤鍔犲瘑鏈冩彌鎺変己鏈嶅櫒閭ｄ簺璁?Git-native PKV 鐪熸鏈夌敤鐨勫姛鑳斤紙姝峰彶 diff銆佷笁鏂硅嚜鍕曞悎浣点€丼SE 鍏у祵鎺ㄩ€併€丮CP 璁€瀵級銆?

鍦ㄥ師鐢?E2EE 钀藉湴鍓嶏紝濡傛灉浣犻渶瑕佺鍒扮鍔犲瘑锛屽彲浠ュ湪绛嗚搴笂鐤婁竴灞?[`git-crypt`](https://github.com/AGWA/git-crypt)锛氳妯欒鐨勮矾寰戞渻浠ュ瘑鏂?blob 褰㈠紡鍒伴仈浼烘湇鍣紝浼烘湇鍣ㄧ劇娉曡В瀵嗐€傛獢鍚嶄粛浠ユ槑鏂囧舰寮忓瓨鍦ㄦ柤浼烘湇鍣紙灏嶅ぇ澶氭暩濞佽剠妯″瀷渚嗚鍙帴鍙楋級銆傛寔鏈夐噾閼扮殑瀹㈡埗绔緷鐒跺彲浠ョ敤妯欐簴 `git clone` 鍜?`pkvsyncd materialize`銆?

姝ｅ紡閮ㄧ讲閭勬噳瑭茶窇鍦?HTTPS 寰岄潰銆佹妸 `trusted_proxies` 鏀剁穵銆佺郸璩囨枡纰熷姞瀵嗐€佺郸鍌欎唤鍔犲瘑鈥斺€斿叿楂旂湅[閮ㄧ讲鍔犲浐鎸囧崡](./public-docs/deployment-hardening.zh-Hant.md)銆?

## 浣犲湪鎵锯€︹€?

| 涓婚 | 鏂囦欢 |
| --- | --- |
| 澶栨帥鏃ュ父浣跨敤 | [浣跨敤鑰呮墜鍐奭(./public-docs/user-manual.zh-Hant.md) |
| 浼烘湇鍣ㄧ鐞嗚垏鍩疯鏅傝ō瀹?| [绠＄悊鍝℃墜鍐奭(./public-docs/admin-manual.zh-Hant.md) |
| 鎵€鏈?CLI 鍛戒护鑸囧弮鏁?| [CLI 鍙冭€僝(./public-docs/cli-reference.zh-Hant.md) |
| 寰?0.x 鍗囩礆鍒?1.0 | [1.0 鍗囩礆瑾槑](./public-docs/upgrade-notes-v1.0.zh-Hant.md) |
| 鍙嶅悜浠ｇ悊銆乀LS銆佸倷浠姐€佸姞鍥?| [閮ㄧ讲鍔犲浐](./public-docs/deployment-hardening.zh-Hant.md) |
| HTTP API 濂戠磩 | [OpenAPI 瑕忕瘎](./public-docs/openapi.yaml) |
| MCP 瀹夎鑸囧伐鍏峰垪琛?| [MCP 鎿嶄綔鎸囧崡](./public-docs/mcp-howto.zh-Hant.md) |
| LLM 缍鐨?Wiki 宸ヤ綔娴?| [LLM Wiki 鎿嶄綔鎸囧崡](./public-docs/llm-wiki-howto.zh-Hant.md) |
| 寰?Obsidian Sync 閬风Щ | [閬风Щ鎸囧崡](./public-docs/migrate-from-obsidian-sync.zh-Hant.md) |
| 瀹夊叏婕忔礊閫氬牨 | [SECURITY.md](./SECURITY.md) |
| 鐧煎竷绱€閷?| [CHANGELOG.md](./CHANGELOG.md) |

## 鐙€鎱?

PKV Sync 1.4.4 寤剁簩瀵╄▓淇京锛屽伌閲嶆纰烘€э細寰屽彴鐩ｇ潱浠诲嫏鍦ㄥ劒闆呴棞闁夋檪涓锛岄亷鏈?DashMap 姊濈洰瀹氭湡鍥炴敹锛岃嚜鍕曞悎浣垫纰哄崁鍒嗙己澶辩墿浠惰垏 Git 鏆厠閷锛屽啰绛夊揩鍙栧湪涓辜璩囨枡浜ゆ槗澶辨晽寰屽绲傚鍏ワ紝涓﹁鏂囧瓧寤虹珛閫忛亷琛濈獊妾旀鎻愬崌淇濈暀銆傚悓鏅傛竻鐞嗕簡鐒＄敤绋嬪紡纰硷紙寤㈡杓斿姪鍑藉紡銆乮18n 閸点€丏ocker 灞わ級銆?

PKV Sync 1.0 鏄涓€鍊嬬┅瀹氱増銆傚叕闁?REST API銆丆LI銆佸劜瀛樺竷灞€銆佸鎺涘寘銆丏ocker 鏄犲儚浣滅偤涓€绲勫悓姝ョ櫦鐗堬紝閬靛惊 semver锛?.X.Y 鍦ㄥ叕闁嬭〃闈繚鎸佸悜寰岀浉瀹癸紝OpenAPI 瑕忕瘎鏄€欏€嬬浉瀹瑰绱勭殑娆婂▉渚嗘簮銆?.x 寤虹珛鐨?SQLite 璩囨枡搴?*涓嶆敮鎻?*灏卞湴鍗囩礆鍒?1.0.0鈥斺€旇珛渚漑1.0 鍗囩礆瑾槑](./public-docs/upgrade-notes-v1.0.zh-Hant.md)鎿嶄綔銆?

姣忓€?GitHub release 鏈冪櫦甯?Linux amd64锛廰rm64 浜岄€蹭綅銆乄indows x64 浜岄€蹭綅銆佸鏋舵 GHCR Docker 鏄犲儚銆丱bsidian 澶栨帥 zip 鍖咃紝浠ュ強 `SHA256SUMS`銆?

## 闁嬬櫦鑷

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 鍦?Linux 鍜?Windows 涓婅窇瀹屾暣 Rust 鐭╅櫍锛屽姞涓婂鎺涚殑 test锛弔ypecheck锛廱uild锛弍ackage銆丏ocker 妲嬪缓锛屼互鍙婄櫦甯冧簩閫蹭綅鐨勫啋鐓欐脯瑭︺€?

## 鎺堟瑠姊濇

AGPL-3.0-only銆傝┏瑕?[LICENSE](./LICENSE)銆?
