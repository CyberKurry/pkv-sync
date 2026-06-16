# PKV Sync

**鑷墭绠′綘鐨?Obsidian 绗旇搴撱€?* PKV Sync 璺戝湪浣犺嚜宸辩殑鏈嶅姟鍣ㄤ笂锛屾妸鎵嬫満銆佸钩鏉裤€佹闈㈢鐨?Obsidian 绗旇搴撲繚鎸佸悓姝ャ€備竴涓簩杩涘埗銆佷竴涓?SQLite 鏁版嵁搴撱€佹瘡涓瑪璁板簱涓€涓?bare git 浠撳簱鈥斺€斾笉闇€瑕侀泦缇わ紝涓嶉渶瑕?S3锛屼笉闇€瑕佷换浣曟墭绠′簯銆傝濂斤紝璁?Obsidian 杩炰笂鍘伙紝绗旇灏卞悓姝ヤ簡銆?

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

鏂囨。鐗堟湰锛歷1.4.4銆?

[English](./README.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./README.zh-Hant.md) | [鏃ユ湰瑾瀅(./README.ja.md) | [頃滉淡鞏碷(./README.ko.md)

## 鐗规€?

- **澶氱敤鎴枫€佸绗旇搴?*鍚屾锛屾寜璁惧绛惧彂浠ょ墝锛屾瘡涓瑪璁板簱甯?push 閿佷笌骞傜瓑閲嶈瘯銆?
- **瀹炴椂鎺ㄩ€?*銆傚皬鏀瑰姩閫氳繃 SSE 鍦ㄤ簹绉掔骇钀藉湴锛涜疆璇綔涓哄厹搴曚繚闄┿€?
- **Git 鍗崇湡鐩?*銆傛瘡涓瑪璁板簱閮芥槸涓€涓?bare git 浠撳簱锛屽崟鏂囦欢鍘嗗彶銆佺粺涓€ diff銆佸崟鏂囦欢鎭㈠寮€绠卞嵆鐢ㄢ€斺€旀彃浠剁鍜岀鐞嗗悗鍙伴兘鑳界敤銆?
- **鍐茬獊瀹夊叏**銆傛彃浠朵笉浼氶潤榛樿鐩栨湰鍦版敼鍔紝鍐茬獊浼氫互 `.conflict-*` 鏂囦欢鍛堢幇锛屼竴閿€屼繚鐣欐湰鍦般€嶆垨銆岄噰绾宠繙绔€嶃€?
- **浜旇瑷€绠＄悊鍚庡彴**锛圗nglish銆佺畝涓€佺箒涓€佹棩鏈獮銆來暅甑柎锛夛細鐢ㄦ埛銆佽澶囦护鐗屻€佺瑪璁板簱銆侀個璇风爜銆佹椿鍔ㄦ棩蹇椼€乥lob 鍨冨溇鍥炴敹锛屽苟瀵圭牬鍧忔€х殑绗旇搴撳拰鐢ㄦ埛鎿嶄綔寮瑰嚭纭銆?
- **AI 鍙**銆侻CP 閫氳繃 stdio銆佺嫭绔?Streamable HTTP锛屾垨 `pkvsyncd serve` 鍐呭祵鐨?`/mcp` 璺敱鏆撮湶璇诲啓宸ュ叿銆?
- **榛樿鏈夎竟鐣?*銆傜鐞嗗憳鍒涘缓/閲嶇疆瀵嗙爜浣跨敤 setup 鍚岀骇寮哄瘑鐮佺瓥鐣ワ紱token 鏄庢枃鍙睍绀轰竴娆★紱涓婁紶鍜?MCP 鍝嶅簲閮芥湁澶у皬涓婇檺锛涘疄鏃?SSE 娴佷細澶嶆煡宸叉挙閿€ token銆?
- **鏁呮剰鍋氬緱鏃犺亰**銆傚崟浜岃繘鍒躲€佸崟 SQLite 鍏冩暟鎹簱銆佹瘡搴撲竴涓?bare git 浠撱€佹瘡涓檮浠朵竴涓唴瀹瑰鍧€ blob銆?

## 鐢?Docker Compose 蹇€熶笂鎵?

杩欐槸鎺ㄨ崘璺緞銆俙deploy/caddy/` 閲岀殑 Caddy 閫氳繃 Let's Encrypt 鑷姩绛惧彂 HTTPS锛汸KV Sync 鍦?compose 鍐呯綉鐩戝惉 `127.0.0.1:6710`锛屽叕缃戝畬鍏ㄨ涓嶅埌鏄庢枃 HTTP銆?

浣犻渶瑕侊細涓€涓煙鍚嶏紙姣斿 `sync.example.com`锛夛紝鍏?A/AAAA 璁板綍鎸囧悜鏈嶅姟鍣紱鍏綉鑳借闂埌 `80` 鍜?`443` 绔彛锛?0 鐢ㄤ簬 ACME HTTP-01 楠岃瘉锛夈€?

1. 鐢熸垚閮ㄧ讲瀵嗛挜锛?

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 鍦?`docker-compose.yml` 鏃佹斁涓€浠?`config.toml`锛?

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 鏇挎崲涓?genkey 杈撳嚭
   public_host    = "sync.example.com"   # 蹇呭～锛岀鐞嗙 POST 鎵嶈兘閫?

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge 缃戞

   [mcp]
   embed_in_serve = false                # true 浼氬湪鏈湇鍔′笂鎸傝浇 /mcp
   ```

3. 缂栬緫 `deploy/caddy/Caddyfile`锛屾妸 `sync.example.com` 鎹㈡垚浣犵殑鐪熷疄鍩熷悕銆?

4. 鍚姩鏁村鏈嶅姟锛?

   ```bash
   docker compose up -d
   ```

   娴忚鍣ㄦ墦寮€ `https://sync.example.com/setup`锛屽缓绗竴涓鐞嗗憳璐﹀彿銆?

5. 鍦?Obsidian 閲屾妸 `pkv-sync-plugin.zip` 瑙ｅ帇鍒?`<vault>/.obsidian/plugins/pkv-sync/`锛屽惎鐢ㄦ彃浠讹紝浠庣鐞嗗悗鍙板鍒跺垎浜?URL 绮樿繘鍘伙紝鐧诲綍鎴栨敞鍐岋紝閫変竴涓瑪璁板簱銆?

鍚庣画鏇存柊灏辨槸 `docker compose pull && docker compose up -d`銆傚鏋滆鍘熺敓瀹夎銆佽皟鍙嶅悜浠ｇ悊锛圕addy锛廚ginx锛廡raefik锛夈€佷簡瑙?`public_host` 鐨勮涔夈€佸仛澶囦唤杩樺師鎴栫鐩樺姞瀵嗭紝璇风湅[閮ㄧ讲鍔犲浐鎸囧崡](./public-docs/deployment-hardening.zh-CN.md)銆?

## MCP 閮ㄧ讲妯″紡

PKV Sync 鎻愪緵涓ょ MCP Streamable HTTP 閮ㄧ讲鏂瑰紡銆傚唴宓屾ā寮忛渶瑕佹樉寮忓紑鍚細璁剧疆 `[mcp].embed_in_serve = true` 鍚庯紝`pkvsyncd serve` 浼氬湪涓绘湇鍔＄鍙ｆ寕杞?`/mcp`锛屽鐢ㄥ悓涓€濂?TLS 缁堟銆佸弽鍚戜唬鐞嗐€侀儴缃插瘑閽ュ拰 bearer 浠ょ墝鏍￠獙銆傜嫭绔嬫ā寮忎繚鐣欏師鏈夊崟鐙繘绋嬶細`pkvsyncd mcp --transport http --bind 127.0.0.1:6711`锛岄€傚悎闅旂 MCP銆佷笓鐢ㄧ洃鍚湴鍧€鎴栫嫭绔嬫墿缂╁銆?

## Obsidian 鎻掍欢

鏈湴鏂囦欢灏辨槸鐪熺浉鈥斺€旀彃浠剁洿鎺ヨ鍐欎綘纾佺洏涓婄殑 Obsidian 绗旇搴擄紝涓嶅瓨鍦ㄤ唬鐞嗘枃浠剁郴缁熻繖绉嶄笢瑗裤€傞潪鏁忔劅鐨勬彃浠惰缃拰鍚屾绱㈠紩淇濆瓨鍦?`<vault>/.obsidian/plugins/pkv-sync/data.json`锛涚櫥褰曠姸鎬併€佸綋鍓?bearer 璁惧浠ょ墝銆侀儴缃插瘑閽ュ拰绋冲畾璁惧韬唤淇濆瓨鍦?Obsidian 鐨勮澶囨湰鍦板瓨鍌ㄤ腑銆傝鎶?Obsidian 璁惧鏈湴瀛樺偍銆佹槑鏂囧浠戒互鍙婃棫鐗堟湰鐣欎笅鐨勬彃浠?`data.json` 鍓湰褰撴垚鏁忔劅鏁版嵁銆傝澶囦护鐗屽湪浣跨敤鏃朵細鑷姩缁湡锛?0 澶╂棤娲诲姩鍚庡け鏁堬紝涓斿崟涓护鐗屾渶闀挎湁鏁?365 澶╋紱鍦ㄥ悓涓€璁惧閲嶆柊鐧诲綍浼氳疆鎹㈡帀鏃т护鐗屻€?

鏃ュ父浣跨敤鈥斺€斿懡浠ら潰鏉裤€佹枃浠跺巻鍙层€佸苟鎺?diff銆佸啿绐佽В鍐炽€乣.obsidian` 閫夋嫨鎬у悓姝ャ€佽澶囩鐞嗐€佹彃浠惰嚜鏇存柊鈥斺€旈兘鍐欏湪[鐢ㄦ埛鎵嬪唽](./public-docs/user-manual.zh-CN.md)閲屻€?

## 鍏充簬鍔犲瘑

PKV Sync 1.0 **鏆備笉**鎻愪緵鍘熺敓绔埌绔姞瀵嗏€斺€旀湇鍔＄鑳借鍒扮瑪璁板唴瀹广€傚師鐢熺殑鎸夊簱 E2EE 鍦?1.x 璺嚎鍥句笂锛屽皢浠ュ彲閫夋ā寮忎笂绾匡紝鍥犱负鍔犲瘑浼氭崲鎺夋湇鍔＄閭ｄ簺璁?Git-native PKV 鐪熸鏈夌敤鐨勫姛鑳斤紙鍘嗗彶 diff銆佷笁鏂硅嚜鍔ㄥ悎骞躲€丼SE 鍐呰仈鎺ㄩ€併€丮CP 璇诲啓锛夈€?

鍦ㄥ師鐢?E2EE 钀藉湴鍓嶏紝濡傛灉浣犻渶瑕佺鍒扮鍔犲瘑锛屽彲浠ュ湪绗旇搴撲笂鍙犱竴灞?[`git-crypt`](https://github.com/AGWA/git-crypt)锛氳鏍囪鐨勮矾寰勪細浠ュ瘑鏂?blob 褰㈠紡鍒拌揪鏈嶅姟绔紝鏈嶅姟绔棤娉曡В瀵嗐€傛枃浠跺悕浠嶄互鏄庢枃褰㈠紡瀛樺湪浜庢湇鍔＄锛堝澶у鏁板▉鑳佹ā鍨嬫潵璇村彲鎺ュ彈锛夈€傛寔鏈夊瘑閽ョ殑瀹㈡埛绔緷鐒跺彲浠ョ敤鏍囧噯 `git clone` 鍜?`pkvsyncd materialize`銆?

鐢熶骇閮ㄧ讲杩樺簲璇ヨ窇鍦?HTTPS 鍚庨潰銆佹妸 `trusted_proxies` 鏀剁揣銆佺粰鏁版嵁鐩樺姞瀵嗐€佺粰澶囦唤鍔犲瘑鈥斺€斿叿浣撶湅[閮ㄧ讲鍔犲浐鎸囧崡](./public-docs/deployment-hardening.zh-CN.md)銆?

## 浣犲湪鎵锯€︹€?

| 涓婚 | 鏂囨。 |
| --- | --- |
| 鎻掍欢鏃ュ父浣跨敤 | [鐢ㄦ埛鎵嬪唽](./public-docs/user-manual.zh-CN.md) |
| 鏈嶅姟绔鐞嗕笌杩愯鏃惰缃?| [绠＄悊鍛樻墜鍐宂(./public-docs/admin-manual.zh-CN.md) |
| 鎵€鏈?CLI 鍛戒护鍜屽弬鏁?| [CLI 鍙傝€僝(./public-docs/cli-reference.zh-CN.md) |
| 浠?0.x 鍗囩骇鍒?1.0 | [1.0 鍗囩骇璇存槑](./public-docs/upgrade-notes-v1.0.zh-CN.md) |
| 鍙嶅悜浠ｇ悊銆乀LS銆佸浠姐€佸姞鍥?| [閮ㄧ讲鍔犲浐](./public-docs/deployment-hardening.zh-CN.md) |
| HTTP API 濂戠害 | [OpenAPI 瑙勮寖](./public-docs/openapi.yaml) |
| MCP 瀹夎涓庡伐鍏峰垪琛?| [MCP 鎿嶄綔鎸囧崡](./public-docs/mcp-howto.zh-CN.md) |
| LLM 缁存姢鐨?Wiki 宸ヤ綔娴?| [LLM Wiki 鎿嶄綔鎸囧崡](./public-docs/llm-wiki-howto.zh-CN.md) |
| 浠?Obsidian Sync 杩佺Щ | [杩佺Щ鎸囧崡](./public-docs/migrate-from-obsidian-sync.zh-CN.md) |
| 瀹夊叏婕忔礊鍙嶉 | [SECURITY.md](./SECURITY.md) |
| 鍙戝竷璁板綍 | [CHANGELOG.md](./CHANGELOG.md) |

## 鐘舵€?

PKV Sync 1.4.4 缁х画瀹¤淇锛屼晶閲嶆纭€э細鍚庡彴鐩戠潱浠诲姟鍦ㄤ紭闆呭叧闂椂涓锛岃繃鏈?DashMap 鏉＄洰瀹氭湡鍥炴敹锛岃嚜鍔ㄥ悎骞舵纭尯鍒嗙己澶卞璞′笌 Git 鐬€侀敊璇紝骞傜瓑缂撳瓨鍦ㄥ厓鏁版嵁浜嬪姟澶辫触鍚庡缁堝啓鍏ワ紝骞跺彂鏂囨湰鍒涘缓閫氳繃鍐茬獊鏂囦欢鎻愬崌淇濈暀銆傚悓鏃舵竻鐞嗕簡鏃犵敤浠ｇ爜锛堝簾寮冭緟鍔╁嚱鏁般€乮18n 閿€丏ocker 灞傦級銆?

PKV Sync 1.0 鏄涓€涓ǔ瀹氱増銆傚叕寮€ REST API銆丆LI銆佸瓨鍌ㄥ竷灞€銆佹彃浠跺寘銆丏ocker 闀滃儚浣滀负涓€缁勫悓姝ュ彂鐗堬紝閬靛惊 semver锛?.X.Y 鍦ㄥ叕寮€琛ㄩ潰淇濇寔鍚戝悗鍏煎锛孫penAPI 瑙勮寖鏄繖涓吋瀹瑰绾︾殑鏉冨▉鏉ユ簮銆?.x 鍒涘缓鐨?SQLite 搴?*涓嶆敮鎸?*灏卞湴鍗囩骇鍒?1.0.0鈥斺€旇鎸?[1.0 鍗囩骇璇存槑](./public-docs/upgrade-notes-v1.0.zh-CN.md)鎿嶄綔銆?

姣忎釜 GitHub release 浼氬彂甯?Linux amd64/arm64 浜岃繘鍒躲€乄indows x64 浜岃繘鍒躲€佸鏋舵瀯 GHCR Docker 闀滃儚銆丱bsidian 鎻掍欢 zip 鍖咃紝浠ュ強 `SHA256SUMS`銆?

## 寮€鍙戣嚜妫€

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 鍦?Linux 鍜?Windows 涓婅窇瀹屾暣 Rust 鐭╅樀锛屽姞涓婃彃浠剁殑 test锛弔ypecheck锛廱uild锛弍ackage銆丏ocker 鏋勫缓锛屼互鍙婂彂甯冧簩杩涘埗鐨勫啋鐑熸祴璇曘€?

## 璁稿彲

AGPL-3.0-only銆傝瑙?[LICENSE](./LICENSE)銆?
