# AI 銉勩兗銉悜銇?MCP 銈偗銈汇偣

[English](./mcp-howto.md) | [绠€浣撲腑鏂嘳(./mcp-howto.zh-CN.md) | [绻侀珨涓枃](./mcp-howto.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./mcp-howto.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

銇撱伄鏂囨浉銇姊扮炕瑷炽伀銈堛倠鍒濈増銇с仚銆傚叕闁嬪墠銇儘銈ゃ儐銈ｃ儢瑭辫€呫伀銈堛倠銉儞銉ャ兗銈掓帹濂ㄣ仐銇俱仚銆?

PKV Sync 銇?MCP server 銈掗€氥仒銇?vault 鍐呭銈掑叕闁嬨仹銇嶃伨銇欍€傘偟銉笺儛銉笺伅銉曘偂銈ゃ儷鍐呭銈掕繑銇欏墠銇?blob pointers 銈掕В姹恒仐銆佹槑绀虹殑銇?read-write tools 銈掗€氥仒銇︽浉銇嶈炯銇裤倐銇с亶銆侀€氬父銇?PKV Sync bearer device token 銇屽繀瑕併仹銇欍€?

## Tools

- `list_vaults`: 瑾嶈娓堛伩銉︺兗銈躲兗銇屽埄鐢ㄣ仹銇嶃倠 vault 銈掍竴瑕ц〃绀恒仐銇俱仚銆?
- `list_files {vault_id, at?}`: HEAD銆併伨銇熴伅 `at` 銇屾寚瀹氥仌銈屻仧鍫村悎銇仢銇?commit SHA 銇?paths 銈掍竴瑕ц〃绀恒仐銇俱仚銆?
- `read_file {vault_id, path}`: HEAD 銇儠銈°偆銉倰瑾伩鍙栥倞銇俱仚銆?
- `read_file_at_commit {vault_id, path, commit}`: 鐗瑰畾 commit 銇儠銈°偆銉倰瑾伩鍙栥倞銇俱仚銆?
- `search {vault_id, query, at?, limit?}`: 銉嗐偔銈广儓銉曘偂銈ゃ儷銇銇椼仸澶ф枃瀛楀皬鏂囧瓧銈掑尯鍒ャ仐銇亜 substring search 銈掑疅琛屻仐銇俱仚銆俙at` 銇ч亷鍘汇伄 commit 銇?scope 銇椼€乣limit` 銇ц繑銇曘倢銈嬩竴鑷存暟銇笂闄愩倰鎸囧畾銇椼伨銇欍€?
- `link_graph {vault_id, at?, path_prefix?, limit?}`: vault 銇?wikilink 銇?Markdown link graph 銈掕繑銇椼伨銇欍€傚繙绛斻伀銇€併儠銈°偆銉仈銇ㄣ伄 node銆乣outlinks`銆佽▓绠椼仌銈屻仧 `inlinks`銆乷rphaned pages銆乣missing` 銇俱仧銇?`ambiguous` reason 銈掓寔銇?broken links銆乣truncated` flag 銇屽惈銇俱倢銇俱仚銆?
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`: `since_commit` 浠ラ檷銇拷鍔犮€佸鏇淬€佸墛闄ゃ€乺ename 銇曘倢銇熴儠銈°偆銉倰涓€瑕ц〃绀恒仐銇俱仚銆傚繙绛斻伀銇?`from_commit`銆佺従鍦ㄣ伄 `to_commit`銆乣changes`銆乣truncated` 銇屽惈銇俱倢銇俱仚銆俙since_commit` 銇?HEAD 銇?ancestor 銇с仾銇勫牬鍚堛€乧lient 銇?vault 銈掑啀瑾伩鍙栥倞銇с亶銈嬨倛銇嗐伀 `unrelated_commit` 銈掕繑銇椼伨銇欍€?
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit` 銇倛銈?optimistic concurrency 銇с儐銈偣銉堛儠銈°偆銉倰浣滄垚銇俱仧銇洿鏂般仐銇俱仚銆?
- `delete_file {vault_id, path, parent_commit}`: `parent_commit` 銇倛銈?optimistic concurrency 銇с儠銈°偆銉倰鍓婇櫎銇椼伨銇欍€?
- `write_files {vault_id, parent_commit, writes?, deletes?}`: 瑜囨暟銇儐銈偣銉堛儠銈°偆銉伄浣滄垚銆佹洿鏂般€佸墛闄ゃ倰 1 銇ゃ伄 commit 銇ㄣ仐銇?atomically 銇疅琛屻仐銇俱仚銆俙writes[]` 銇?`{path, content}` objects銆乣deletes[]` 銇?paths 銈掑惈銇裤伨銇欍€?
- `move_file {vault_id, parent_commit, from, to}`: 銉嗐偔銈广儓銉曘偂銈ゃ儷銈?1 銇ゃ伄 commit 銇хЩ鍕曘伨銇熴伅 rename 銇椼€乬it rename history 銈掍繚銇°伨銇欍€倀arget path 銇棦瀛樸仹銇傘仯銇︺伅銇勩亼銇俱仜銈撱€?

銇欍伖銇︺伄 MCP read tools 銇従鍦ㄣ伄 SyncPathFilter 銈掑皧閲嶃仐銇俱仚銆傜祫銇胯炯銇裤伄 hidden-path rules 銇俱仧銇?runtime exclude globs 銇嫆鍚︺仌銈屻仧 paths 銇€佷竴瑕ц〃绀恒€佹绱€佽銇垮彇銈娿€乴ink graph 銇搞伄鍚湁銆乧hange reporting 銇璞°伀銇倞銇俱仜銈撱€?

## stdio transport

銈炽優銉炽儔銈掕捣鍕曘仚銈嬨儹銉笺偒銉?AI 銉勩兗銉仹銇?stdio 銈掍娇鐢ㄣ仐銇俱仚銆俿tdio mode 銇?1 銇ゃ伄 vault 銇?scope 銇曘倢銇俱仚銆?

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

token 銈掔洿鎺ユ浮銇欍亾銇ㄣ倐銇с亶銇俱仚銆?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

銈儵銈ゃ偄銉炽儓銇屻仚銇с伀瀹熻涓伄銉兗銈儷銇俱仧銇唴閮?MCP endpoint 銇帴缍氥仚銈嬪牬鍚堛伅 HTTP 銈掍娇鐢ㄣ仐銇俱仚銆侾KV Sync 銇伅 2 銇ゃ伄 HTTP 銉囥儣銉偆銉兗銉夈亴銇傘倞銇俱仚銆?

- **Embedded**: `config.toml` 銇?`[mcp].embed_in_serve = true` 銈掕ō瀹氥仚銈嬨仺銆乣pkvsyncd serve` 銇屻儭銈ゃ兂銈点兗銉愩兗銉濄兗銉堛伀 `/mcp` 銈掋優銈︺兂銉堛仐銇俱仚銆?
- **Standalone**: 灏傜敤 bind address銆侀殧闆仌銈屻仧 MCP銆佺嫭绔?scaling 銇屽繀瑕併仾鍫村悎銇€佸垾 MCP 銉椼儹銈汇偣銈掑疅琛屻仐銇俱仚銆?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

endpoint path 銇父銇?`/mcp` 銇с仚銆俥mbedded mode 銇с伅銉°偆銉炽偟銉笺儛銉?origin銆乻tandalone mode 銇с伅灏傜敤 bind address 銈掍娇銇勩伨銇欍€?

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

銇欍伖銇︺伄銉偗銈ㄣ偣銉堛伀銇銇屽繀瑕併仹銇欍€?

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

銉囥儣銉偆銉°兂銉堛偔銉笺伅涓?PKV Sync 銈点兗銉愩兗銇ㄥ悓銇樿ō瀹氥儠銈°偆銉亱銈夎銇垮彇銈夈倢銇俱仚銆傘偔銉笺亴銇亜銆併伨銇熴伅闁撻仌銇ｃ仸銇勩倠鍫村悎銇?bearer token 瑾嶈銇墠銇?HTTP `404` 銈掕繑銇椼伨銇欍€?

MCP HTTP 銇浐瀹氥偊銈ｃ兂銉夈偊銇?60 绉掋亗銇熴倞 120 銉偗銈ㄣ偣銉堛伀鍒堕檺銇曘倢銇俱仚銆傚埗闄愩倰瓒呫亪銈嬨仺銆併偟銉笺儛銉笺伅 HTTP `429` 銇?JSON-RPC error code `-32029` 銈掕繑銇椼伨銇欍€傚け鏁椼仐銇?MCP bearer token 瑾嶈銈傘儣銉偦銈瑰唴銇у埗闄愩仌銈屻€乻tdio 銇?HTTP transports 銇悎瑷堛仹 60 绉掋亗銇熴倞鏈€澶?30 鍥炪伄澶辨晽瑭﹁銇俱仹銇с仚銆?

POST 銇?JSON-RPC tool calls 銈掗亱銇炽€丣SON responses 銈掕繑銇椼伨銇欍€俙Accept: text/event-stream` 銈掓寔銇?GET 銇?`vault_changed` notifications 銈掕臣瑾仐銇俱仚銆侲vent ids 銇?`<vault-id>:<commit-sha>` 銈掍娇鐢ㄣ仐銆佸啀鎺ョ稓鏅傘伀 `Last-Event-ID` 銇ㄣ仐銇﹂€併倞杩斻仚銇撱仺銇?missed commits 銈?replay 銇с亶銇俱仚銆俁eplay 銇伅涓婇檺銇屻亗銈娿伨銇欍€傘偟銉笺儛銉笺亴 missed history 銈掋偒銉愩兗銇с亶銇亜鍫村悎銇?`lagged` 銈掗€佷俊銇椼€併偗銉┿偆銈兂銉堛伅 sync API 銇嬨倝鏇存柊銇欍倠蹇呰銇屻亗銈娿伨銇欍€?

淇￠牸銇с亶銈嬨儘銉冦儓銉兗銈埗寰°伄鑳屽緦銇疆銇嬨仾銇勯檺銈娿€丠TTP 銇?loopback 銇?bind 銇椼仸銇忋仩銇曘亜銆俠earer token 銇€併仢銇儲銉笺偠銉笺亴鎵€鏈夈仚銈嬨仚銇广仸銇?vault 銇搞伄瑾伩鏇搞亶銈偗銈汇偣銈掍笌銇堛伨銇欍€?

## Read and search limits

`search` 銇渶澶?5000 鍊嬨伄 visible tree files 銈掕蛋鏌汇仐銆佹渶澶?500 matches 銈掕繑銇椼€乸roduction 銇с伅妞滅储娓堛伩 text 銇?256 MiB 銇仈銇欍倠銇ㄥ仠姝仐銇俱仚銆俙link_graph` 銇渶澶?5000 鍊嬨伄 visible text files 銈掕蛋鏌汇仐銆佸悓銇?production text budget 銈掍娇鐢ㄣ仐銇俱仚銆俙changes_since` 銇渶澶?5000 鍊嬨伄 visible change entries 銈掕繑銇椼伨銇欍€俙read_file` 銇?`read_file_at_commit` 銇繙绛斿墠銇?blob pointer 銈掕В姹恒仐銇俱仚銆?4 MiB 銈掕秴銇堛倠 binary/blob response 銇€乥ase64 銇ㄣ仐銇?JSON 銇睍闁嬨仌銈屻倠浠ｃ倧銈娿伀鎷掑惁銇曘倢銇俱仚銆?

## Write tools

PKV Sync 銇銇垮彇銈?tools 銇ㄤ降銇涖仸 4 銇ゃ伄 MCP write tools 銈掓彁渚涖仐銇俱仚銆?

- `write_file(vault_id, path, content, parent_commit)`: 銉嗐偔銈广儓銉曘偂銈ゃ儷銈掍綔鎴愩伨銇熴伅鏇存柊銇椼伨銇欍€?
- `delete_file(vault_id, path, parent_commit)`: 銉曘偂銈ゃ儷銈掑墛闄ゃ仐銇俱仚銆?
- `write_files(vault_id, parent_commit, writes[], deletes[])`: 瑜囨暟銇儐銈偣銉堛儠銈°偆銉倰 1 銇ゃ伄 commit 銇?atomically 銇綔鎴愩€佹洿鏂般€佸墛闄ゃ仐銇俱仚銆俻ath 銇岀劇鍔广€乫ile 銇?`max_file_size` 銈掕秴銇堛倠銆乥atch 銇岀┖ (`empty_batch`)銆併伨銇熴伅 batch 銇?100 changes 銈掕秴銇堛倠 (`batch_too_large`) 鍫村悎銆佷綍銈?commit 銇曘倢銇俱仜銈撱€傚彜銇?`parent_commit` 銇с伅閫氬父銇?`Conflict` response 銈掕繑銇椼伨銇欍€?
- `move_file(vault_id, parent_commit, from, to)`: 1 銇ゃ伄銉嗐偔銈广儓銉曘偂銈ゃ儷銈掑崢涓€ commit 銇хЩ鍕曘伨銇熴伅 rename 銇椼伨銇欍€傛棦瀛?target (`target_exists`)銆乥inary/blob-pointer source (`unsupported_binary_move`)銆佸瓨鍦ㄣ仐銇亜銇俱仧銇?hidden 銇?source (`not_found`) 銇嫆鍚︺仐銇俱仚銆?

### Optimistic concurrency control

銇欍伖銇︺伄鏇搞亶杈笺伩銇伅 `parent_commit`銆併仱銇俱倞銈儵銈ゃ偄銉炽儓銇岀従鍦ㄣ伄 vault head 銇犮仺鑰冦亪銈?commit hash 銇屽繀瑕併仹銇欍€傘偗銉┿偆銈兂銉堛亴鏈€寰屻伀瑾倱銇犲緦銇?vault 銇岄€层倱銇с亜銈嬪牬鍚堛€併偟銉笺儛銉笺伅 `{ "conflict": true, "current_head": "..." }` 銈掕繑銇椼€佹浉銇嶈炯銇裤伨銇涖倱銆傘偗銉┿偆銈兂銉堛伅鍐嶈銇垮彇銈娿仐銆佸繀瑕併仾銈?merge 銇椼€佹柊銇椼亜 `parent_commit` 銇?retry 銇欍倠蹇呰銇屻亗銈娿伨銇欍€?

### Rate limit

Write tools 銇?`(token, vault)` 銉氥偄銇斻仺銇?1 鍒嗐亗銇熴倞 60 writes 銇埗闄愩仌銈屻伨銇欍€俙write_files` 銇?batch 鍏ㄤ綋銇?1 銇ゃ伄 rate-limit record 銇犮亼銈掓秷璨汇仐銇俱仚銆俁ead tools 銇?SSE subscriptions 銇亾銇?write quota 銇奖闊裤倰鍙椼亼銇俱仜銈撱€?

1.2.1 銇挤鍖栥仹銇€佹浉銇嶈炯銇挎瑷笺倰 fail-closed 銇繚銇°伨銇欍€俙writes[]` 銇?`deletes[]` 銇ф瑕忓寲寰屻伀閲嶈銇欍倠 path 銇嫆鍚︺仌銈屻€乭idden 銇俱仧銇?excluded path 銇璞°伄瀛樺湪銈掓紡銈夈仌銇氥€佺劇鍔广仾 `move_file` 銇Щ鍕曞厓銇?write quota 銈掓秷璨汇仚銈嬪墠銇嫆鍚︺仌銈屻伨銇欍€侻CP 瑾嶈銈ㄣ儵銉笺伅姹庣敤銉°儍銈汇兗銈搞伄銇俱伨銇с€丼treamable HTTP JSON body 銇笂闄愩伅 100 MiB 銇с仚銆?

### Audit trail

鎴愬姛銇椼仧 write銆乥atch write銆乵ove銆乨elete 銇仚銇广仸銆乤ctivity log 銇?`mcp_write` 銇俱仧銇?`mcp_delete` 銇ㄣ仐銇﹁閷层仌銈屻€乨etails 銇伅 path summary銆乧ommit銆乻ize 銇屽惈銇俱倢銇俱仚銆傜鐞嗚€呫伅 activity page 銇嬨倝 AI-driven changes 銈掔⒑瑾嶃仹銇嶃伨銇欍€?

### Caveat: writes enter git history

AI-driven writes 銇?vault git history 銇?commits 銇仾銈娿伨銇欍€傞€氬父銇?git operations 銇?roll back 銇с亶銇俱仚銇屻€乧ommit 娓堛伩銇鏇淬倰銆岀櫤鐢熴仐銇亱銇ｃ仧銆嶃亾銇ㄣ伀銇仹銇嶃伨銇涖倱銆傘亾銇?audit trail 銇剰鍥崇殑銇倐銇仹銇欍€?

## Client notes

- Claude Code銆丆odex CLI銆丆herry Studio銆丱penCode銆併亰銈堛伋 bridge-based MCP clients 銇€乣pkvsyncd mcp` 銈掕捣鍕曘仐銇?stdio mode 銈掍娇鐢ㄣ仹銇嶃伨銇欍€?
- Streamable HTTP 銈掋偟銉濄兗銉堛仚銈?clients 銇?`/mcp` 銈掓寚銇椼€併仚銇广仸銇儶銈偍銈广儓銇?bearer auth 銇ㄣ儑銉椼儹銈ゃ儭銉炽儓銈兗銈掗€佷俊銇с亶銇俱仚銆?
- 銈点兗銉愩兗銇?stateless 銇с仚銆俙Mcp-Session-Id` 銈掕姹傘仜銇氥€佽繑銇椼伨銇涖倱銆?
