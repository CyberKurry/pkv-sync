# 鍗囩礆瑾槑锛?.x 鍒?1.0

[English](./upgrade-notes-v1.0.md) | [绠€浣撲腑鏂嘳(./upgrade-notes-v1.0.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./upgrade-notes-v1.0.ja.md) | [頃滉淡鞏碷(./upgrade-notes-v1.0.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

PKV Sync 1.0 鏄涓€鍊嬬┅瀹氱増銆傚畠涔熺偤寰岀簩 1.x 缍閲嶇疆浜?SQLite migration 鍩虹窔銆?

## 閲嶈璩囨枡搴鏄?

PKV Sync 1.0 鍙櫦甯冧竴鍊?`0001_initial.sql` 鍩虹窔 migration銆傜敱 0.x 鐗堟湰寤虹珛鐨?SQLite 璩囨枡搴?*涓嶆敮鎻村師鍦板崌绱?*鍒?1.0.0銆?

濡傛灉浣犳鍦ㄥ煼琛?0.x 鏈嶅嫏绔紝璜嬮伕鎿囦笅闈㈣矾寰戜箣涓€锛?

1. 鑸婇儴缃插彧鍦ㄩ伔绉绘簴鍌欐湡闁撳仠鐣欏湪鏈€绲?0.8.x patch 鐗堟湰锛岀敤鏂煎倷浠姐€乵aterialize 鎴栧尟鍑鸿硣鏂欍€?
2. 鍏堝倷浠芥垨 materialize 姣忓€嬬瓎瑷樺韩锛屼娇鐢ㄥ叏鏂扮殑 1.0 璩囨枡鐩寗鍟熷嫊鏈嶅嫏锛岄噸鏂板缓绔嬩娇鐢ㄨ€呭拰绛嗚搴紝鐒跺緦鎶婄瓎瑷樺韩鍏у鍖叆鎴?push 鍒版柊鏈嶅嫏绔€?
3. 鍦ㄤ换浣曢伔绉绘紨绶村墠锛屽厛鐢?`pkvsyncd backup` 淇濆瓨瀹屾暣鐨?0.x 璩囨枡鏍圭洰閷勩€?

涓嶈鎶?1.0 浜岄€蹭綅鎴?Docker 鏄犲儚鐩存帴鎸囧悜鏃㈡湁鐨?0.x `metadata.db`銆?

## 1.0 绌╁畾鎵胯

寰?1.0 闁嬪锛屼互涓嬭〃闈㈤伒寰獮缇╁寲鐗堟湰锛?

- `public-docs/openapi.yaml` 涓閷勭殑鍏枊 REST 璺敱銆?
- MCP how-to 涓閷勭殑 MCP stdio 鍜?Streamable HTTP 宸ュ叿琛岀偤銆?
- 闈㈠悜 1.x 鍏ㄦ柊璩囨枡搴殑 SQLite migrations锛涘湪閫欐 v1 鍩虹窔涔嬪緦锛屾湭渚?1.x migration 淇濇寔杩藉姞寮忋€?
- 姣忕瓎瑷樺韩 git repository 甯冨眬鍜屽収瀹瑰畾鍧€ blob 鍎插瓨銆?
- CLI 瀛愬懡浠ゅ拰鏃㈡湁鍙冩暩銆?
- Obsidian 澶栨帥瑷畾鍜屽悓姝ヨ鐐猴紝鍏佽ū 1.x 姝ｅ父鏂板鍚戝緦鐩稿鍔熻兘銆?

OpenAPI 涓矑鏈夎閷勭殑璺敱锛屼緥濡?Admin Web UI 琛ㄥ柈铏曠悊鍣紝灞柤鍏ч儴瀵︿綔绱扮瘈銆?

## 鎺ㄨ枽鐨?0.x 鍒?1.0 娴佺▼

1. 濡傛浠跺厑瑷憋紝鍏堟妸鑸婇儴缃插崌绱氬埌鏈€绲?0.8.x patch 鐗堟湰锛岀劧寰屽儏鐢ㄥ畠瀹屾垚鍌欎唤銆乵aterialize 鎴栧尟鍑烘簴鍌欍€?
2. 鍩疯 `pkvsyncd backup --output <backup-dir>` 涓﹀Ε鍠勪繚瀛樺倷浠姐€?
3. 灏嶆瘡鍊嬬瓎瑷樺韩锛屼娇鐢ㄦ渶鏂?Obsidian 鐢ㄦ埗绔€乣git clone`锛屾垨 `pkvsyncd materialize <vault-id> --output <dir>` 寰楀埌鐩墠妾旀妯广€?
4. 鍋滄鑸婃湇鍕欑銆?
5. 浣跨敤鍏ㄦ柊鐨勭┖ `data_dir` 鍜?`metadata.db` 鍟熷嫊 PKV Sync 1.0銆?
6. 瀹屾垚 `/setup`锛岄噸鏂板缓绔嬩娇鐢ㄨ€呭拰绛嗚搴紝鐒跺緦 push 鎴栧尟鍏?materialized 绛嗚搴収瀹广€?
7. 閫氱煡浣跨敤鑰呮妸 Obsidian 澶栨帥鏇存柊鍒?1.0.0銆?

## 澶栨帥鐩稿鎬?

1.0 鏈嶅嫏绔殑鍙楁敮鎻村鎺涙槸闅ㄦ湇鍕欑鎹嗙秮鐨?1.0 Obsidian 澶栨帥銆傝垔鐨?v0.8.x 澶栨帥浣跨敤鍚屼竴濂楁牳蹇冨悓姝?API锛屼絾鏂扮殑淇京鍜岃嚜鏇存柊鍔犲浐鍙湪 1.0+ 涓董璀枫€?

## 鐩稿皪 0.x 鐨勭牬澹炴€ц畩鏇?

- 鐢辨柤 migrations 宸插绺偤鍠€?v1 鍩虹窔锛?.x SQLite 璩囨枡搴笉鑳藉師鍦板崌绱氥€?
- 棣栨鍩疯 setup 浠嶇劧閫忛亷鐎忚鍣ㄥ畬鎴愶紱鍏ㄦ柊鏈嶅嫏绔笉鏈冨啀鎶婇毃姗熺鐞嗗摗瀵嗙⒓鍒楀嵃鍒版棩瑾屻€?

绛嗚鍏у銆乬it 姝峰彶鍜?blobs 浠嶅彲閫忛亷 backup/materialize/recreate/import 宸ヤ綔娴佸付鍒版柊閮ㄧ讲銆?

## 宸茬煡娉ㄦ剰浜嬮爡

- 鍘熺敓 per-vault E2EE 涓嶅爆鏂?1.0 绡勫湇銆備粖澶╅渶瑕佸鎴剁鍋存獢妗堝収瀹瑰姞瀵嗙殑浣跨敤鑰呭彲浠ヤ娇鐢?[`git-crypt`](./git-crypt-howto.zh-Hant.md)锛屼甫鎺ュ彈璺緫浠嶇偤鏄庢枃鐨勫彇鎹ㄣ€?
- `/metrics` 闋愯ō闂滈枆锛涘暉鐢ㄥ緦浠嶉渶鐢熺敘瑾嶈瓑闁€绂併€?
- 鐢熺敘閮ㄧ讲璜嬭ō瀹?`public_host`銆傜暥鏈嶅嫏绔劇娉曠⒑瀹氳ō瀹氬ソ鐨?HTTPS 鍏恫 origin 鏅傦紝admin POST 鏈冩晠鎰?fail closed銆?
