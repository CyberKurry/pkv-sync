# AI 宸ュ叿鐨?MCP 鎺ュ叆

[English](./mcp-howto.md) | [绠€浣撲腑鏂嘳(./mcp-howto.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./mcp-howto.ja.md) | [頃滉淡鞏碷(./mcp-howto.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

PKV Sync 鍙互閫忛亷 MCP server 鏆撮湶绛嗚搴収瀹广€傛湇鍕欑杩斿洖妾旀鍏у鍓嶆渻瑙ｆ瀽 blob pointer锛屼篃鍙互閫忛亷椤紡璁€瀵伐鍏峰鍏ユ獢妗堬紝涓︿笖蹇呴爤浣跨敤鏅€?PKV Sync bearer 瑁濈疆 token銆?

## 宸ュ叿

- `list_vaults`锛氬垪鍑虹洰鍓嶄娇鐢ㄨ€呭彲瀛樺彇鐨勭瓎瑷樺韩銆?
- `list_files {vault_id, at?}`锛氬垪鍑?HEAD 涓嬬殑璺緫锛涜ō瀹?`at` 鏅傚墖鍒楀嚭瑭?commit SHA 涓嬬殑璺緫銆?
- `read_file {vault_id, path}`锛氳畝鍙?HEAD 涓嬬殑妾旀銆?
- `read_file_at_commit {vault_id, path, commit}`锛氳畝鍙栨寚瀹?commit 涓嬬殑妾旀銆?
- `search {vault_id, query, at?, limit?}`锛氬湪鏂囧瓧妾旀涓煼琛屽ぇ灏忓涓嶆晱鎰熺殑瀛愬瓧涓叉悳灏嬨€俙at` 灏囩瘎鍦嶉檺瀹氬埌姝峰彶 commit锛沗limit` 闄愬埗鍥炲偝鐨勫懡涓暩閲忋€?
- `link_graph {vault_id, at?, path_prefix?, limit?}`锛氳繑鍥炵瓎瑷樺韩鐨?wikilink 鑸?Markdown 閫ｇ祼鍦栥€傚洖鎳夊寘鍚瘡鍊嬫獢妗堢殑绡€榛炲強鍏?`outlinks` 鑸囪▓绠楀嚭鐨?`inlinks`銆佸绔嬮爜闈€佸付鏈?`missing` 鎴?`ambiguous` 鍘熷洜鐨勬柗瑁傞€ｇ祼锛屼互鍙?`truncated` 妯欒銆?
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`锛氬垪鍑鸿嚜 `since_commit` 浠ヤ締鏂板銆佷慨鏀广€佸埅闄ゆ垨閲嶆柊鍛藉悕鐨勬獢妗堛€傚洖鎳夊寘鍚?`from_commit`銆佺洰鍓嶇殑 `to_commit`銆乣changes` 鑸?`truncated`锛涘鏋?`since_commit` 涓嶆槸 HEAD 鐨勭鍏堬紝宸ュ叿鏈冭繑鍥?`unrelated_commit`锛岃畵鐢ㄦ埗绔噸鏂拌畝鍙栫瓎瑷樺韩銆?
- `write_file {vault_id, path, content, parent_commit}`锛氫互 `parent_commit` 妯傝涓︾櫦鎺у埗寤虹珛鎴栨洿鏂版枃瀛楁獢妗堛€?
- `delete_file {vault_id, path, parent_commit}`锛氫互 `parent_commit` 妯傝涓︾櫦鎺у埗鍒櫎妾旀銆?
- `write_files {vault_id, parent_commit, writes?, deletes?}`锛氬湪涓€鍊?commit 涓師瀛愬湴寤虹珛銆佹洿鏂板拰锛忔垨鍒櫎澶氬€嬫枃瀛楁獢妗堛€俙writes[]` 鍖呭惈 `{path, content}` 鐗╀欢锛沗deletes[]` 鍖呭惈璺緫銆?
- `move_file {vault_id, parent_commit, from, to}`锛氬湪涓€鍊?commit 涓Щ鍕曟垨閲嶆柊鍛藉悕鏂囧瓧妾旀锛屼甫淇濈暀 git rename 姝峰彶銆傜洰妯欒矾寰戜笉鑳藉凡缍撳瓨鍦ㄣ€?

鎵€鏈?MCP 璁€鍙栧伐鍏烽兘閬靛畧鐩墠鐨?SyncPathFilter銆傝鍏у缓闅辫棌璺緫瑕忓墖鎴栧煼琛岄殠娈?exclude globs 鎷掔禃鐨勮矾寰戯紝涓嶆渻琚垪鍑恒€佹悳灏嬨€佽畝鍙栥€佺磵鍏ラ€ｇ祼鍦栵紝鎴栧洖鍫辩偤璁婃洿銆?

## stdio transport

鏈 AI 宸ュ叿闇€瑕佸暉鍕曞懡浠ゆ檪锛屼娇鐢?stdio銆俿tdio 妯″紡鍙毚闇蹭竴鍊嬬瓎瑷樺韩銆?

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

涔熷彲浠ョ洿鎺ュ偝鍏?token锛?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

鐣剁敤鎴剁閫ｆ帴涓€鍊嬪凡缍撳煼琛岀殑鏈鎴栧収閮?MCP 绔粸鏅傦紝浣跨敤 HTTP銆侾KV Sync 鎻愪緵鍏╃ó HTTP 閮ㄧ讲妯″紡锛?

- **鍏у祵妯″紡**锛氬湪 `config.toml` 涓ō瀹?`[mcp].embed_in_serve = true`锛宍pkvsyncd serve` 鏈冨湪涓绘湇鍕欑鍙ｆ帥杓?`/mcp`銆?
- **鐛ㄧ珛妯″紡**锛氬煼琛屽柈鐛ㄧ殑 MCP 閫茬▼锛岄仼鍚堝皥鐢ㄧ洠鑱戒綅鍧€銆侀殧闆?MCP 鎴栫崹绔嬫摯绺锛?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

绔粸璺緫濮嬬祩鏄?`/mcp`锛涘収宓屾ā寮忎娇鐢ㄤ富鏈嶅嫏 origin锛岀崹绔嬫ā寮忎娇鐢ㄥ柈鐛ㄧ殑鐩ｈ伣浣嶅潃锛?

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

姣忓€嬭珛姹傞兘蹇呴爤鍖呭惈锛?

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

閮ㄧ讲閲戦懓渚嗚嚜鑸囦富 PKV Sync 鏈嶅嫏鐩稿悓鐨勮ō瀹氭獢銆傜己灏戞垨閷鐨勯儴缃查噾閼版渻鍦?bearer token 椹楄瓑鍓嶇洿鎺ュ洖鍌?HTTP `404`銆?

MCP HTTP 浣跨敤鍥哄畾瑕栫獥闄愭祦锛屾瘡 60 绉掓渶澶?120 娆¤珛姹傘€傝秴闄愭檪锛屼己鏈嶅櫒鏈冭繑鍥?HTTP `429`锛孞SON-RPC error code 鐐?`-32029`銆傚け鏁楃殑 MCP bearer token 瑾嶈瓑涔熸渻鍦ㄩ€茬▼鍏ч檺娴侊紝stdio 鍜?HTTP transport 鍚堣▓姣?60 绉掓渶澶?30 娆″け鏁楀槜瑭︺€?

POST 鎵胯級 JSON-RPC 宸ュ叿鍛煎彨涓﹁繑鍥?JSON 鍥炴噳銆侴ET 鏀滃付 `Accept: text/event-stream` 鏅傝▊闁?`vault_changed` notification銆備簨浠?id 浣跨敤 `<vault-id>:<commit-sha>`锛岀敤鎴剁閲嶉€ｆ檪鍙綔鐐?`Last-Event-ID` 鍌冲洖锛屼互 replay 鏂风窔鏈熼枔閷亷鐨?commit銆俁eplay 鏈変笂闄愶紱濡傛灉鏈嶅嫏绔劇娉曡钃嬮尟閬庣殑姝峰彶锛屾渻鐧奸€?`lagged`锛岀敤鎴剁鎳夐€忛亷鍚屾 API 閲嶆柊鏁寸悊銆?

闄ら潪鏀惧湪鍙俊缍茶矾鎺у埗涔嬪緦锛屽惁鍓囪珛鎶?HTTP 缍佸畾鍒?loopback銆俠earer token 鏈冩巿浜堣┎浣跨敤鑰呮墍鏈夌瓎瑷樺韩鐨勮畝瀵瓨鍙栨瑠闄愩€?

## 璁€鍙栧拰鎼滃皨涓婇檺

`search` 鏈€澶氭巸鎻?5000 鍊嬪彲瑕?tree 妾旀锛屾渶澶氳繑鍥?500 姊濆尮閰嶏紝涓﹀湪鐢熺敘鐠板鎼滃皨鏂囧瓧绱▓閬斿埌 256 MiB 寰屽仠姝€俙link_graph` 鏈€澶氭巸鎻?5000 鍊嬪彲瑕嬫枃瀛楁獢妗堬紝涓︿娇鐢ㄧ浉鍚岀殑鐢熺敘鐠板鏂囧瓧闋愮畻銆俙changes_since` 鏈€澶氳繑鍥?5000 姊濆彲瑕嬭畩鏇撮爡鐩€俙read_file` 鍜?`read_file_at_commit` 鏈冨湪杩斿洖鍓嶈В鏋?blob pointer锛涜秴閬?64 MiB 鐨勪簩閫蹭綅/blob 鍥炴噳鏈冭鎷掔禃锛岃€屼笉鏄 base64 灞曢枊閫?JSON銆?

## 瀵叆宸ュ叿

PKV Sync 鍦ㄨ畝鍙栧伐鍏蜂箣澶栨彁渚涘洓鍊?MCP 瀵叆宸ュ叿锛?

- `write_file(vault_id, path, content, parent_commit)`锛氬缓绔嬫垨鏇存柊鏂囧瓧妾旀銆?
- `delete_file(vault_id, path, parent_commit)`锛氬埅闄ゆ獢妗堛€?
- `write_files(vault_id, parent_commit, writes[], deletes[])`锛氬湪涓€鍊?commit 涓師瀛愬湴寤虹珛銆佹洿鏂板拰鍒櫎澶氬€嬫枃瀛楁獢妗堛€傚鏋滀换涓€璺緫鐒℃晥銆佹獢妗堣秴閬?`max_file_size`銆佹壒娆＄偤绌猴紙`empty_batch`锛夛紝鎴栨壒娆¤秴閬?100 鍊嬭畩鏇达紙`batch_too_large`锛夛紝鏈嶅嫏绔笉鏈冩彁浜や换浣曞収瀹广€傞櫝鑸婄殑 `parent_commit` 鏈冭繑鍥炲父瑕?`Conflict` 鍥炴噳銆?
- `move_file(vault_id, parent_commit, from, to)`锛氬湪鍠€?commit 涓Щ鍕曟垨閲嶆柊鍛藉悕涓€鍊嬫枃瀛楁獢妗堛€傚畠鏈冩嫆绲曞凡瀛樺湪鐨勭洰妯欙紙`target_exists`锛夈€佷簩閫蹭綅锛廱lob-pointer 渚嗘簮妾旀锛坄unsupported_binary_move`锛夛紝浠ュ強缂哄け鎴栭毐钘忕殑渚嗘簮妾旀锛坄not_found`锛夈€?

### 妯傝涓︾櫦鎺у埗

姣忔瀵叆閮藉繀闋堟彁渚?`parent_commit`锛屼篃灏辨槸鐢ㄦ埗绔獚鐐虹洰鍓嶇瓎瑷樺韩 HEAD 鎵€鍦ㄧ殑 commit hash銆傚鏋滅敤鎴剁涓婃璁€鍙栧緦绛嗚搴凡缍撳墠閫诧紝鏈嶅嫏绔渻杩斿洖 `{ "conflict": true, "current_head": "..." }`锛屼甫涓斾笉鏈冨鍏ャ€傜敤鎴剁闇€瑕侀噸鏂拌畝鍙栥€佸繀瑕佹檪鍚堜降锛屽啀鐢ㄦ柊鐨?`parent_commit` 閲嶈│銆?

### 闄愭祦

瀵叆宸ュ叿鎸?`(token, vault)` 绲勫悎闄愭祦锛屾瘡鍒嗛悩鏈€澶?60 娆″鍏ャ€俙write_files` 鏁村€嬫壒娆″彧娑堣€椾竴娆￠檺娴佽閷勩€傝畝鍙栧伐鍏峰拰 SSE 瑷傞柋涓嶅彈閫欏€嬪鍏ラ厤椤嶅奖闊裤€?

1.2.1 鐨勫姞鍥鸿畵瀵叆椹楄瓑淇濇寔 fail-closed锛歚writes[]` 鍜?`deletes[]` 涓瑕忓寲寰岄噸瑜囩殑璺緫鏈冭鎷掔禃锛岄毐钘忔垨鎺掗櫎璺緫涓嶆渻娲╂紡鐩瀛樺湪鎬э紝鐒℃晥鐨?`move_file` 渚嗘簮鏈冨湪娑堣€楀鍏ラ厤椤嶅墠琚嫆绲曘€侻CP 椹楄瓑閷淇濇寔娉涘寲锛孲treamable HTTP JSON request body 涓婇檺鐐?100 MiB銆?

### 绋芥牳瑷橀寗

姣忔鎴愬姛瀵叆銆佹壒閲忓鍏ャ€佺Щ鍕曟垨鍒櫎閮芥渻鍦ㄦ椿鍕曟棩瑾屼腑瑷橀寗鐐?`mcp_write` 鎴?`mcp_delete`锛宒etails 涓寘鍚矾寰戞憳瑕併€乧ommit 鍜?size銆傜鐞嗗摗鍙互鍦ㄦ椿鍕曢爜鏌ョ湅 AI 椹呭嫊鐨勬敼鍕曘€?

### 娉ㄦ剰锛氬鍏ユ渻閫插叆 git 姝峰彶

AI 椹呭嫊鐨勫鍏ユ渻鎴愮偤绛嗚搴?git 姝峰彶涓殑 commit銆備綘鍙互閫忛亷鏅€?git 鎿嶄綔鍥炴痪锛屼絾鐒℃硶璁撳凡缍撴彁浜ょ殑鏀瑰嫊銆屽緸鏈櫦鐢熴€嶏紱閫欑ó鍙ń鏍告€ф槸鏈夋剰瑷▓銆?

## 鐢ㄦ埗绔彁绀?

- Claude Code銆丆odex CLI銆丆herry Studio銆丱penCode锛屼互鍙婇€忛亷姗嬫帴浣跨敤 MCP 鐨勭敤鎴剁锛岄兘鍙互閫忛亷鍟熷嫊 `pkvsyncd mcp` 浣跨敤 stdio 妯″紡銆?
- 鏀彺 Streamable HTTP 鐨勭敤鎴剁鍙互鎸囧悜 `/mcp`锛屼甫鍦ㄦ瘡鍊嬭珛姹備笂鐧奸€?bearer auth 鑸囬儴缃查噾閼般€?
- 鏈嶅嫏绔槸鐒＄媭鎱嬬殑锛屼笉瑕佹眰涔熶笉杩斿洖 `Mcp-Session-Id`銆?
