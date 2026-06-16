# AI 霃勱惮鞖?MCP 鞝戧芳

[English](./mcp-howto.md) | [绠€浣撲腑鏂嘳(./mcp-howto.zh-CN.md) | [绻侀珨涓枃](./mcp-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./mcp-howto.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.3.

鞚?氍胳劀電?旮瓣硠 氩堨棴鞙茧 毵岆摖 齑堦赴 氩勳爠鞛呺媹雼? 瓿店皽 鞝勳棎 鞗愳柎氙?瓴€韱犽ゼ 甓岇灔頃╇媹雼?

PKV Sync電?MCP server毳?韱淀暣 vault 雮挫毄鞚?雲胳稖頃?靾?鞛堨姷雼堧嫟. 靹滊矂電?韺岇澕 雮挫毄鞚?氚橅櫂頃橁赴 鞝勳棎 blob pointers毳?頃挫劃頃橁碃, 氇呾嫓鞝侅澑 read-write tools毳?韱淀暣 鞊瓣赴霃?頃?靾?鞛堨溂氅? 鞚茧皹 PKV Sync bearer device token鞚?頃勳殧頃╇媹雼?

## Tools

- `list_vaults`: 鞚胳霅?靷毄鞛愱皜 靷毄頃?靾?鞛堧姅 vault毳?雮橃棿頃╇媹雼?
- `list_files {vault_id, at?}`: HEAD 霕愲姅 `at`鞚?歆€鞝曤悳 瓴届毎 頃措嫻 commit SHA鞚?paths毳?雮橃棿頃╇媹雼?
- `read_file {vault_id, path}`: HEAD鞚?韺岇澕鞚?鞚届姷雼堧嫟.
- `read_file_at_commit {vault_id, path, commit}`: 韸轨爼 commit鞚?韺岇澕鞚?鞚届姷雼堧嫟.
- `search {vault_id, query, at?, limit?}`: 韰嶌姢韸?韺岇澕鞐愳劀 雽€靻岆鞛愲ゼ 甑秳頃橃 鞎婋姅 substring search毳?靾橅枆頃╇媹雼? `at`鞚€ 瓿缄卑 commit鞙茧 氩旍渼毳?頃滌爼頃橁碃, `limit`鞚€ 氚橅櫂霅橂姅 鞚检箻 靾橃潣 靸來暅鞚?歆€鞝曧暕雼堧嫟.
- `link_graph {vault_id, at?, path_prefix?, limit?}`: vault鞚?wikilink 氚?Markdown link graph毳?氚橅櫂頃╇媹雼? 鞚戨嫷鞐愲姅 韺岇澕氤?node鞕€ `outlinks`, 瓿勳偘霅?`inlinks`, orphaned pages, `missing` 霕愲姅 `ambiguous` reason鞚?鞛堧姅 broken links, 攴鸽Μ瓿?`truncated` flag臧€ 韽暔霅╇媹雼?
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`: `since_commit` 鞚错泟 於旉皜, 靾橃爼, 靷牅, rename霅?韺岇澕鞚?雮橃棿頃╇媹雼? 鞚戨嫷鞐愲姅 `from_commit`, 順勳灛 `to_commit`, `changes`, `truncated`臧€ 韽暔霅╇媹雼? `since_commit`鞚?HEAD鞚?ancestor臧€ 鞎勲媹氅?韥措澕鞚挫柛韸戈皜 vault毳?雼れ嫓 鞚届潉 靾?鞛堧弰搿?`unrelated_commit`鞚?氚橅櫂頃╇媹雼?
- `write_file {vault_id, path, content, parent_commit}`: `parent_commit`鞚?靷毄頃?optimistic concurrency搿?韰嶌姢韸?韺岇澕鞚?毵岆摛瓯半倶 鞐呺嵃鞚错姼頃╇媹雼?
- `delete_file {vault_id, path, parent_commit}`: `parent_commit`鞚?靷毄頃?optimistic concurrency搿?韺岇澕鞚?靷牅頃╇媹雼?
- `write_files {vault_id, parent_commit, writes?, deletes?}`: 鞐煬 韰嶌姢韸?韺岇澕鞚?靸濎劚, 鞐呺嵃鞚错姼, 靷牅毳?頃橂倶鞚?commit鞙茧 atomically 靾橅枆頃╇媹雼? `writes[]`鞐愲姅 `{path, content}` objects臧€ 霌れ柎臧€瓿? `deletes[]`鞐愲姅 paths臧€ 霌れ柎臧戨媹雼?
- `move_file {vault_id, parent_commit, from, to}`: 韰嶌姢韸?韺岇澕鞚?頃橂倶鞚?commit鞐愳劀 鞚措彊頃橁卑雮?rename頃橂┌ git rename history毳?氤挫〈頃╇媹雼? target path電?鞚措 臁挫灛頃橂┐ 鞎?霅╇媹雼?

氇摖 MCP read tools電?順勳灛 SyncPathFilter毳?欷€靾橅暕雼堧嫟. 旮半掣 hidden-path rules 霕愲姅 runtime exclude globs鞐?鞚橅暣 瓯半秬霅?paths電?雮橃棿, 瓴€靸? 鞚疥赴, link graph 韽暔, 氤€瓴?靷暛 氤搓碃 雽€靸侅棎靹?鞝滌櫢霅╇媹雼?

## stdio transport

氇呺牴鞚?鞁ろ枆頃橂姅 搿滌滑 AI 霃勱惮鞐愲姅 stdio毳?靷毄頃╇媹雼? stdio mode電?頃橂倶鞚?vault搿?scope霅╇媹雼?

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

token鞚?歆侅爲 鞝勲嫭頃?靾橂弰 鞛堨姷雼堧嫟.

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

韥措澕鞚挫柛韸戈皜 鞚措 鞁ろ枆 欷戩澑 搿滌滑 霕愲姅 雮措秬 MCP endpoint鞐?鞐瓣舶頃?霑岆姅 HTTP毳?靷毄頃╇媹雼? PKV Sync電?霊?臧€歆€ HTTP 氚绊彫 氇摐毳?鞝滉车頃╇媹雼?

- **Embedded**: `config.toml`鞐愳劀 `[mcp].embed_in_serve = true`毳?靹れ爼頃橂┐ `pkvsyncd serve`臧€ 氅旍澑 靹滊矂 韽姼鞐?`/mcp`毳?毵堨毚韸疙暕雼堧嫟.
- **Standalone**: 鞝勳毄 bind address, 瓴╇Μ霅?MCP, 霃呺 scaling鞚?頃勳殧頃?霑?氤勲弰 MCP 頂勲靹胳姢毳?鞁ろ枆頃╇媹雼?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

endpoint path電?頃儊 `/mcp`鞛呺媹雼? embedded mode鞐愳劀電?氅旍澑 靹滊矂 origin鞚? standalone mode鞐愳劀電?鞝勳毄 bind address毳?靷毄頃╇媹雼?

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

氇摖 鞖旍箔鞐愲姅 雼れ潓鞚?韽暔霅橃柎鞎?頃╇媹雼?

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

氚绊彫 韨る姅 欤?PKV Sync 靹滊矂鞕€ 臧欖潃 靹れ爼 韺岇澕鞐愳劀 鞚届姷雼堧嫟. 韨り皜 鞐嗞卑雮?鞛橂霅橂┐ bearer token 鞚胳 鞝勳棎 HTTP `404`毳?氚橅櫂頃╇媹雼?

MCP HTTP電?瓿犾爼 彀?氚╈嫕鞙茧 60齑堧嫻 120臧?鞖旍箔鞙茧 鞝滍暅霅╇媹雼? 鞝滍暅鞚?齑堦臣頃橂┐ 靹滊矂電?HTTP `429`鞕€ JSON-RPC error code `-32029`毳?氚橅櫂頃╇媹雼? 鞁ろ尐頃?MCP bearer token 鞚胳霃?頂勲靹胳姢 雮挫棎靹?鞝滍暅霅橂┌, stdio鞕€ HTTP transports 頃╈偘 60齑堧嫻 斓滊寑 30須?鞁ろ尐 鞁滊弰旯岇 項堨毄霅╇媹雼?

POST電?JSON-RPC tool calls毳?雼搓碃 JSON responses毳?氚橅櫂頃╇媹雼? `Accept: text/event-stream`鞚?鞛堧姅 GET鞚€ `vault_changed` notifications毳?甑弲頃╇媹雼? Event ids電?`<vault-id>:<commit-sha>`毳?靷毄頃橂┌, 鞛棸瓴?鞁?`Last-Event-ID`搿?霅橂弻霠?氤措偞 missed commits毳?replay頃?靾?鞛堨姷雼堧嫟. Replay鞐愲姅 靸來暅鞚?鞛堨姷雼堧嫟. 靹滊矂臧€ missed history毳?旎る矂頃?靾?鞐嗢溂氅?`lagged`毳?雮措炒雮措┌, 韥措澕鞚挫柛韸鸽姅 sync API鞐愳劀 靸堧 瓿犾硱鞎?頃╇媹雼?

鞁犽頃?靾?鞛堧姅 雱ろ姼鞗岉伂 鞝滌柎 霋れ棎 霊愳 鞎婋姅 頃?HTTP毳?loopback鞐?bind頃橃劯鞖? bearer token鞚€ 頃措嫻 靷毄鞛愱皜 靻岇湢頃?氇摖 vault鞐?雽€頃?鞚疥赴 氚?鞊瓣赴 鞝戧芳 甓岉暅鞚?攵€鞐暕雼堧嫟.

## Read and search limits

`search`電?斓滊寑 5000臧?visible tree files毳?鞀れ簲頃橁碃 斓滊寑 500 matches毳?氚橅櫂頃橂┌, 頂勲雿曥厴鞐愳劀電?瓴€靸夗暅 text臧€ 256 MiB鞐?霃勲嫭頃橂┐ 欷戨嫧頃╇媹雼? `link_graph`電?斓滊寑 5000臧?visible text files毳?鞀れ簲頃橁碃 霃欖澕頃?頂勲雿曥厴 text budget鞚?靷毄頃╇媹雼? `changes_since`電?斓滊寑 5000臧?visible change entries毳?氚橅櫂頃╇媹雼? `read_file`瓿?`read_file_at_commit`鞚€ 鞚戨嫷 鞝勳棎 blob pointer毳?頃挫劃頃╇媹雼? 64 MiB毳?雱橂姅 binary/blob response電?base64搿?JSON鞐?頇曥灔霅橂姅 雽€鞁?瓯半秬霅╇媹雼?

## Write tools

PKV Sync電?鞚疥赴 tools鞕€ 頃粯 雱?臧滌潣 MCP write tools毳?鞝滉车頃╇媹雼?

- `write_file(vault_id, path, content, parent_commit)`: 韰嶌姢韸?韺岇澕鞚?毵岆摛瓯半倶 鞐呺嵃鞚错姼頃╇媹雼?
- `delete_file(vault_id, path, parent_commit)`: 韺岇澕鞚?靷牅頃╇媹雼?
- `write_files(vault_id, parent_commit, writes[], deletes[])`: 鞐煬 韰嶌姢韸?韺岇澕鞚?頃橂倶鞚?commit鞐愳劀 atomically 毵岆摛瓿? 鞐呺嵃鞚错姼頃橁碃, 靷牅頃╇媹雼? path臧€ 鞙犿毃頃橃 鞎婈卑雮? 韺岇澕鞚?`max_file_size`毳?雱橁卑雮? batch臧€ 牍勳柎 鞛堦卑雮?`empty_batch`), batch臧€ 100 changes毳?雱橃溂氅?`batch_too_large`) 鞎勲瓴冸弰 commit頃橃 鞎婌姷雼堧嫟. 鞓る灅霅?`parent_commit`鞚€ 鞚茧皹 `Conflict` response毳?氚橅櫂頃╇媹雼?
- `move_file(vault_id, parent_commit, from, to)`: 頃橂倶鞚?韰嶌姢韸?韺岇澕鞚?雼澕 commit鞐愳劀 鞚措彊頃橁卑雮?rename頃╇媹雼? 鞚措 臁挫灛頃橂姅 target(`target_exists`), binary/blob-pointer source(`unsupported_binary_move`), 鞐嗞卑雮?hidden鞚?source(`not_found`)電?瓯半秬頃╇媹雼?

### Optimistic concurrency control

氇摖 鞊瓣赴鞐愲姅 `parent_commit`鞚?頃勳殧頃╇媹雼? 鞚措姅 韥措澕鞚挫柛韸戈皜 順勳灛 vault head霛缄碃 靸濌皝頃橂姅 commit hash鞛呺媹雼? 韥措澕鞚挫柛韸戈皜 毵堨毵夓溂搿?鞚届潃 霋?vault臧€ 歆勴枆霅橃棃雼る┐ 靹滊矂電?`{ "conflict": true, "current_head": "..." }`毳?氚橅櫂頃橁碃 鞊办 鞎婌姷雼堧嫟. 韥措澕鞚挫柛韸鸽姅 雼れ嫓 鞚疥碃, 頃勳殧頃橂┐ merge頃?霋?靸?`parent_commit`鞙茧 retry頃挫暭 頃╇媹雼?

### Rate limit

Write tools電?`(token, vault)` 鞂嶋硠搿?攵勲嫻 60 writes搿?鞝滍暅霅╇媹雼? `write_files`電?batch 鞝勳泊鞐?雽€頃?rate-limit record 頃橂倶毵?靷毄頃╇媹雼? Read tools鞕€ SSE subscriptions電?鞚?write quota鞚?鞓來枼鞚?氚涭 鞎婌姷雼堧嫟.

1.2.1 臧曧檾電?鞊瓣赴 瓴€歃濎潉 fail-closed搿?鞙犾頃╇媹雼? `writes[]`鞕€ `deletes[]`鞐愳劀 鞝曣窚頇?頉?欷戨车霅橂姅 path電?瓯半秬霅橁碃, hidden 霕愲姅 excluded paths電?雽€靸?臁挫灛毳?雲胳稖頃橃 鞎婌溂氅? 鞛橂霅?`move_file` 鞗愲掣鞚€ write quota毳?靻岆箘頃橁赴 鞝勳棎 瓯半秬霅╇媹雼? MCP 鞚胳 鞓る電?鞚茧皹 氅旍嫓歆€搿?鞙犾霅橁碃 Streamable HTTP JSON body 靸來暅鞚€ 100 MiB鞛呺媹雼?

### Audit trail

靹标车頃?氇摖 write, batch write, move, delete電?activity log鞐?`mcp_write` 霕愲姅 `mcp_delete`搿?旮半霅橂┌, details鞐愲姅 path summary, commit, size臧€ 韽暔霅╇媹雼? 甏€毽瀽電?activity page鞐愳劀 AI-driven changes毳?瓴€韱犿暊 靾?鞛堨姷雼堧嫟.

### Caveat: writes enter git history

AI-driven writes電?vault git history鞚?commits臧€ 霅╇媹雼? 鞚茧皹 git operations搿?roll back頃?靾?鞛堨毵? 鞚措 commit霅?氤€瓴届潉 "never have happened"搿?毵岆摛 氚╇矔鞚€ 鞐嗢姷雼堧嫟. 鞚?audit trail鞚€ 鞚橂弰霅?瓴冹瀰雼堧嫟.

## Client notes

- Claude Code, Codex CLI, Cherry Studio, OpenCode, bridge-based MCP clients電?`pkvsyncd mcp`毳?鞁ろ枆頃?stdio mode毳?靷毄頃?靾?鞛堨姷雼堧嫟.
- Streamable HTTP毳?歆€鞗愴晿電?clients電?`/mcp`毳?臧€毽偆瓿?氇摖 鞖旍箔鞐?bearer auth鞕€ 氚绊彫 韨るゼ 氤措偧 靾?鞛堨姷雼堧嫟.
- 靹滊矂電?stateless鞛呺媹雼? `Mcp-Session-Id`毳?鞖旉惮頃橁卑雮?氚橅櫂頃橃 鞎婌姷雼堧嫟.
