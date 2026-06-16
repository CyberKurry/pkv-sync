# AI 宸ュ叿鐨?MCP 鎺ュ叆

[English](./mcp-howto.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./mcp-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./mcp-howto.ja.md) | [頃滉淡鞏碷(./mcp-howto.ko.md)

鏂囨。鐗堟湰锛歷1.4.5銆?

PKV Sync 鍙互閫氳繃 MCP server 鏆撮湶绗旇搴撳唴瀹广€傛湇鍔＄鍦ㄨ繑鍥炴枃浠跺唴瀹瑰墠浼氳В鏋?blob pointer锛屽彲浠ラ€氳繃鏄惧紡璇诲啓宸ュ叿鍐欏叆鏂囦欢锛屽苟涓旇姹備娇鐢ㄦ櫘閫氱殑 PKV Sync bearer 璁惧 token銆?

## 宸ュ叿

- `list_vaults`锛氬垪鍑哄凡璁よ瘉鐢ㄦ埛鍙闂殑绗旇搴撱€?
- `list_files {vault_id, at?}`锛氬垪鍑?HEAD 鐨勮矾寰勶紱璁剧疆 `at` 鏃讹紝鍒楀嚭璇?commit SHA 涓嬬殑璺緞銆?
- `read_file {vault_id, path}`锛氳鍙?HEAD 涓嬬殑鏂囦欢銆?
- `read_file_at_commit {vault_id, path, commit}`锛氳鍙栨寚瀹?commit 涓嬬殑鏂囦欢銆?
- `search {vault_id, query, at?, limit?}`锛氬湪鏂囨湰鏂囦欢涓墽琛屽ぇ灏忓啓涓嶆晱鎰熺殑瀛愪覆鎼滅储銆俙at` 灏嗚寖鍥撮檺瀹氬埌鏌愪釜鍘嗗彶 commit锛沗limit` 闄愬埗杩斿洖鐨勫尮閰嶆暟閲忋€?
- `link_graph {vault_id, at?, path_prefix?, limit?}`锛氳繑鍥炵瑪璁板簱鐨?wikilink 鍜?Markdown 閾炬帴鍥俱€傚搷搴斿寘鍚瘡涓枃浠惰妭鐐圭殑 `outlinks` 鍜岃绠楀嚭鐨?`inlinks`銆佸绔嬮〉闈€佸甫鏈?`missing` 鎴?`ambiguous` 鍘熷洜鐨勬柇閾撅紝浠ュ強 `truncated` 鏍囧織銆?
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`锛氬垪鍑鸿嚜 `since_commit` 浠ユ潵鏂板銆佷慨鏀广€佸垹闄ゆ垨閲嶅懡鍚嶇殑鏂囦欢銆傚搷搴斿寘鍚?`from_commit`銆佸綋鍓?`to_commit`銆乣changes` 鍜?`truncated`锛涘鏋?`since_commit` 涓嶆槸 HEAD 鐨勭鍏堬紝宸ュ叿浼氳繑鍥?`unrelated_commit`锛屼互渚垮鎴风閲嶆柊璇诲彇绗旇搴撱€?
- `write_file {vault_id, path, content, parent_commit}`锛氫互 `parent_commit` 杩涜涔愯骞跺彂鎺у埗锛屽垱寤烘垨鏇存柊鏂囨湰鏂囦欢銆?
- `delete_file {vault_id, path, parent_commit}`锛氫互 `parent_commit` 杩涜涔愯骞跺彂鎺у埗锛屽垹闄ゆ枃浠躲€?
- `write_files {vault_id, parent_commit, writes?, deletes?}`锛氬湪涓€涓?commit 涓師瀛愬湴鍒涘缓銆佹洿鏂板拰锛忔垨鍒犻櫎澶氫釜鏂囨湰鏂囦欢銆俙writes[]` 鍖呭惈 `{path, content}` 瀵硅薄锛沗deletes[]` 鍖呭惈璺緞銆?
- `move_file {vault_id, parent_commit, from, to}`锛氬湪涓€涓?commit 涓Щ鍔ㄦ垨閲嶅懡鍚嶆枃鏈枃浠讹紝骞朵繚鐣?git rename 鍘嗗彶銆傜洰鏍囪矾寰勪笉鑳藉凡缁忓瓨鍦ㄣ€?

鎵€鏈?MCP 璇诲彇宸ュ叿閮戒細閬靛畧褰撳墠鐨?SyncPathFilter銆傝鍐呯疆闅愯棌璺緞瑙勫垯鎴栬繍琛屾椂 exclude glob 鎷掔粷鐨勮矾寰勶紝涓嶄細琚垪鍑恒€佹悳绱€佽鍙栥€佺撼鍏ラ摼鎺ュ浘锛屼篃涓嶄細浣滀负鍙樻洿鎶ュ憡銆?

## stdio transport

鏈湴 AI 宸ュ叿闇€瑕佸惎鍔ㄥ懡浠ゆ椂锛屼娇鐢?stdio銆俿tdio 妯″紡闄愬畾鍒颁竴涓瑪璁板簱銆?

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

涔熷彲浠ョ洿鎺ヤ紶鍏?token锛?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

褰撳鎴风杩炴帴鍒颁竴涓凡缁忚繍琛岀殑鏈湴鎴栧唴缃?MCP 绔偣鏃讹紝浣跨敤 HTTP銆侾KV Sync 鎻愪緵涓ょ HTTP 閮ㄧ讲妯″紡锛?

- **宓屽叆妯″紡**锛氬湪 `config.toml` 涓缃?`[mcp].embed_in_serve = true`锛岀劧鍚?`pkvsyncd serve` 浼氬湪涓绘湇鍔＄鍙ｆ寕杞?`/mcp`銆?
- **鐙珛妯″紡**锛氳繍琛屽崟鐙殑 MCP 杩涚▼锛岄€傚悎涓撶敤鐩戝惉鍦板潃銆侀殧绂?MCP锛屾垨鐙珛鎵╃缉瀹癸細

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

绔偣璺緞濮嬬粓鏄?`/mcp`锛涘祵鍏ユā寮忎娇鐢ㄤ富鏈嶅姟 origin锛岀嫭绔嬫ā寮忎娇鐢ㄥ崟鐙殑鐩戝惉鍦板潃锛?

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

姣忎釜璇锋眰閮藉繀椤诲寘鍚細

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

閮ㄧ讲瀵嗛挜鏉ヨ嚜涓庝富 PKV Sync 鏈嶅姟鐩稿悓鐨勯厤缃枃浠躲€傜己灏戞垨閿欒鐨勯儴缃插瘑閽ヤ細鍦?bearer token 璁よ瘉鍓嶆敹鍒?HTTP `404`銆?

MCP HTTP 浣跨敤鍥哄畾绐楀彛闄愭祦锛屾瘡 60 绉掓渶澶?120 娆¤姹傘€傝秴闄愭椂锛屾湇鍔＄杩斿洖 HTTP `429`锛屽苟杩斿洖 code 涓?`-32029` 鐨?JSON-RPC error銆?
澶辫触鐨?MCP bearer-token 璁よ瘉涔熶細鍦ㄨ繘绋嬪唴闄愭祦锛宻tdio 鍜?HTTP transport 鍚堣姣?60 绉掓渶澶?30 娆″け璐ュ皾璇曘€?

POST 鎵胯浇 JSON-RPC 宸ュ叿璋冪敤骞惰繑鍥?JSON 鍝嶅簲銆侴ET 鎼哄甫 `Accept: text/event-stream` 鏃惰闃?`vault_changed` notification銆備簨浠?id 浣跨敤 `<vault-id>:<commit-sha>`锛屽鎴风閲嶈繛鏃跺彲浣滀负 `Last-Event-ID` 浼犲洖锛屼互 replay 鏂嚎鏈熼棿閿欒繃鐨?commit銆俁eplay 鏈変笂闄愶紱濡傛灉鏈嶅姟绔棤娉曡鐩栭敊杩囩殑鍘嗗彶锛屼細鍙戦€?`lagged`锛屽鎴风搴旈€氳繃鍚屾 API 鍒锋柊銆?

闄ら潪鏀惧湪鍙俊缃戠粶鎺у埗涔嬪悗锛屽惁鍒欒灏?HTTP 缁戝畾鍒?loopback銆俠earer token 浼氭巿浜堣鐢ㄦ埛鎵€鏈夌瑪璁板簱鐨勮鍐欒闂潈闄愩€?

## 璇诲彇鍜屾悳绱笂闄?

`search` 鏈€澶氭壂鎻?5000 涓彲瑙?tree 鏂囦欢锛屾渶澶氳繑鍥?500 鏉″尮閰嶏紝骞跺湪鐢熶骇鐜鎼滅储鏂囨湰绱杈惧埌 256 MiB 鍚庡仠姝€俙link_graph` 鏈€澶氭壂鎻?5000 涓彲瑙佹枃鏈枃浠讹紝骞朵娇鐢ㄧ浉鍚岀殑鐢熶骇鐜鏂囨湰棰勭畻銆俙changes_since` 鏈€澶氳繑鍥?5000 鏉″彲瑙佸彉鏇撮」銆俙read_file` 鍜?`read_file_at_commit` 浼氬湪杩斿洖鍓嶈В鏋?blob pointer锛涜秴杩?64 MiB 鐨勪簩杩涘埗/blob 鍝嶅簲浼氳鎷掔粷锛岃€屼笉鏄 base64 灞曞紑杩?JSON銆?

## 鍐欏叆宸ュ叿

PKV Sync 鍦ㄨ鍙栧伐鍏蜂箣澶栨彁渚涘洓涓?MCP 鍐欏叆宸ュ叿锛?

- `write_file(vault_id, path, content, parent_commit)`锛氬垱寤烘垨鏇存柊鏂囨湰鏂囦欢銆?
- `delete_file(vault_id, path, parent_commit)`锛氬垹闄ゆ枃浠躲€?
- `write_files(vault_id, parent_commit, writes[], deletes[])`锛氬湪涓€涓?commit 涓師瀛愬湴鍒涘缓銆佹洿鏂板拰鍒犻櫎澶氫釜鏂囨湰鏂囦欢銆傚鏋滀换涓€璺緞鏃犳晥銆佹枃浠惰秴杩?`max_file_size`銆佹壒娆′负绌猴紙`empty_batch`锛夛紝鎴栨壒娆¤秴杩?100 涓彉鏇达紙`batch_too_large`锛夛紝鏈嶅姟绔笉浼氭彁浜や换浣曞唴瀹广€傞檲鏃х殑 `parent_commit` 浼氳繑鍥炲父瑙?`Conflict` 鍝嶅簲銆?
- `move_file(vault_id, parent_commit, from, to)`锛氬湪鍗曚釜 commit 涓Щ鍔ㄦ垨閲嶅懡鍚嶄竴涓枃鏈枃浠躲€傚畠浼氭嫆缁濆凡瀛樺湪鐨勭洰鏍囷紙`target_exists`锛夈€佷簩杩涘埗锛廱lob-pointer 婧愭枃浠讹紙`unsupported_binary_move`锛夛紝浠ュ強缂哄け鎴栭殣钘忕殑婧愭枃浠讹紙`not_found`锛夈€?

### 涔愯骞跺彂鎺у埗

姣忔鍐欏叆閮藉繀椤绘彁渚?`parent_commit`锛屼篃灏辨槸瀹㈡埛绔涓哄綋鍓嶇瑪璁板簱 HEAD 鎵€鍦ㄧ殑 commit hash銆傚鏋滃鎴风涓婃璇诲彇鍚庣瑪璁板簱宸茬粡鍓嶈繘锛屾湇鍔＄浼氳繑鍥?`{ "conflict": true, "current_head": "..." }`锛屽苟涓斾笉浼氬啓鍏ャ€傚鎴风闇€瑕侀噸鏂拌鍙栥€佸繀瑕佹椂鍚堝苟锛屽啀鐢ㄦ柊鐨?`parent_commit` 閲嶈瘯銆?

### 闄愭祦

鍐欏叆宸ュ叿鎸?`(token, vault)` 缁勫悎闄愭祦锛屾瘡鍒嗛挓鏈€澶?60 娆″啓鍏ャ€俙write_files` 鏁翠釜鎵规鍙秷鑰椾竴娆￠檺娴佽褰曘€傝鍙栧伐鍏峰拰 SSE 璁㈤槄涓嶅彈杩欎釜鍐欏叆棰濆害褰卞搷銆?

1.2.1 鐨勫姞鍥鸿鍐欏叆鏍￠獙淇濇寔 fail-closed锛歚writes[]` 鍜?`deletes[]` 涓綊涓€鍖栧悗閲嶅鐨勮矾寰勪細琚嫆缁濓紝闅愯棌鎴栨帓闄よ矾寰勪笉浼氭硠婕忕洰鏍囧瓨鍦ㄦ€э紝鏃犳晥鐨?`move_file` 鏉ユ簮浼氬湪娑堣€楀啓鍏ラ搴﹀墠琚嫆缁濄€侻CP 閴存潈閿欒淇濇寔娉涘寲锛孲treamable HTTP JSON 璇锋眰浣撲笂闄愪负 100 MiB銆?

### 瀹¤璁板綍

姣忔鎴愬姛鍐欏叆銆佹壒閲忓啓鍏ャ€佺Щ鍔ㄦ垨鍒犻櫎閮戒細鍦ㄦ椿鍔ㄦ棩蹇椾腑璁板綍涓?`mcp_write` 鎴?`mcp_delete`锛宒etails 涓寘鍚矾寰勬憳瑕併€乧ommit 鍜?size銆傜鐞嗗憳鍙互鍦ㄦ椿鍔ㄩ〉鏌ョ湅 AI 椹卞姩鐨勬敼鍔ㄣ€?

### 娉ㄦ剰锛氬啓鍏ヤ細杩涘叆 git 鍘嗗彶

AI 椹卞姩鐨勫啓鍏ヤ細鎴愪负绗旇搴?git 鍘嗗彶涓殑 commit銆備綘鍙互閫氳繃鏅€?git 鎿嶄綔鍥炴粴锛屼絾鏃犳硶璁╁凡缁忔彁浜ょ殑鏀瑰姩鈥滀粠鏈彂鐢熲€濓紱杩欑鍙璁℃€ф槸鏈夋剰璁捐銆?

## 瀹㈡埛绔彁绀?

- Claude Code銆丆odex CLI銆丆herry Studio銆丱penCode锛屼互鍙婇€氳繃妗ユ帴浣跨敤 MCP 鐨勫鎴风锛岄兘鍙互閫氳繃鍚姩 `pkvsyncd mcp` 浣跨敤 stdio 妯″紡銆?
- 鏀寔 Streamable HTTP 鐨勫鎴风鍙互鎸囧悜 `/mcp`锛屽苟鍦ㄦ瘡涓姹備笂鍙戦€?bearer auth 鍜岄儴缃插瘑閽ャ€?
- 鏈嶅姟绔槸鏃犵姸鎬佺殑锛屼笉瑕佹眰涔熶笉杩斿洖 `Mcp-Session-Id`銆?
