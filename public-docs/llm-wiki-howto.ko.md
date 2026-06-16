# PKV Sync毳?靷毄頃?LLM Wiki workflow

[English](./llm-wiki-howto.md) | [绠€浣撲腑鏂嘳(./llm-wiki-howto.zh-CN.md) | [绻侀珨涓枃](./llm-wiki-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./llm-wiki-howto.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.3.

鞚?氍胳劀電?旮瓣硠 氩堨棴鞚?氚旐儠鞙茧 雼る摤鞚€ 頃滉淡鞏?氍胳劀鞛呺媹雼? 鞏挫儔頃?響滍槃鞚措倶 鞚橂臧€ 氇樃頃?攵€攵勳澊 鞛堨溂氅?鞓侅柎 鞗愲鞚?頃粯 頇曥澑頃橃劯鞖?

PKV Sync電?LLM鞚?鞙犾 甏€毽晿電?wiki毳?鞙勴暅 storage, history, MCP substrate毳?鞝滉车頃╇媹雼? 靷毄鞛愱皜 靹犿儩頃?MCP-capable agent臧€ LLM鞚?鞁ろ枆頃橁碃, 鞚茧皹 PKV Sync device token鞚?韱淀暣 鞚疥碃 鞊半┌, 鞀轨澑霅?氇摖 氤€瓴?靷暛鞚?vault鞚?git history鞐?commit頃╇媹雼?

## 靹?臧€歆€ 瓿勳傅

靷瀸瓿?agent臧€ 氇憪 vault毳?於旊頃?靾?鞛堧弰搿?鞛戧碃 氇呾嫓鞝侅澑 甑“毳?靷毄頃橃劯鞖?

- **Sources**: 鞗愲掣 notes, 攵欖棳雱ｌ潃 research, imported files, meeting transcripts, 攴?氚栰潣 evidence鞛呺媹雼? 鞗愳瀽耄岇棎 臧€旯濌矊 氤搓磤頃橁碃 雮橃鞐?audit頃?靾?鞛堧弰搿?於╇秳頃?provenance毳?韽暔頃橃劯鞖?
- **Wiki**: durable facts, decisions, concepts, people, projects, processes毳?臧勱舶頃橁矊 靹る獏頃橂姅 pages鞛呺媹雼? 鞚?pages電?靹滊 link頃橁碃 source pages毳?cite頃╇媹雼?
- **Schema**: required frontmatter, index page, maintenance log觳橂熂 wiki毳?lintable頃橁矊 毵岆摐電?氇?臧€歆€ conventions鞛呺媹雼?

PKV Sync電?substrate鞚挫 LLM host臧€ 鞎勲嫏雼堧嫟. 靹滊矂電?safe read tools, optimistic write tools, link inspection, change inspection鞚?雲胳稖頃╇媹雼? 氍挫棁鞚?summarize, rewrite頃橁卑雮?靷毄鞛愳棎瓴?confirmation鞚?鞖旍箔頃犾電?靷毄鞛愱皜 靹犿儩頃?agent臧€ 瓴办爼頃╇媹雼?

## Agent 鞐瓣舶

PKV Sync device token鞚?毵岆摛瓯半倶 鞛偓鞖╉暅 霋? MCP-capable agent臧€ stdio搿?頃橂倶鞚?vault毳?臧€毽偆瓴?頃橃劯鞖?

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

Streamable HTTP毳?歆€鞗愴晿電?agents鞚?瓴届毎 embedded 霕愲姅 standalone mode搿?`/mcp`毳?雲胳稖頃橁碃 氇摖 鞖旍箔鞐?deployment key鞕€ bearer token鞚?頃粯 氤措偧 靾?鞛堨姷雼堧嫟. transport details電?MCP access guide毳?彀胳“頃橃劯鞖?

agent鞐愲姅 膦侅潃 instruction鞚?欤检劯鞖? source pages毳?鞚疥碃, wiki updates毳?鞝滌晥頃橁碃, 鞊?霑岆姅 毵堨毵?read鞐愳劀 鞏混潃 `parent_commit`鞚?靷毄頃橂┌, facts臧€ 攵堩檿鞁ろ晿瓯半倶 conflicts臧€ 雮橅儉雮橂┐ human review毳?鞙勴暣 氅堨稊霃勲 歆€鞁滍暕雼堧嫟.

## 甓岇灔 schema

雼れ潓 layout鞙茧 鞁滌瀾頃橁碃, workflow鞐?牍勴暣 雱堧 鞛戩晞臁岇潉 霑岆 臁办爼頃橃劯鞖?

```text
index.md
log.md
sources/
wiki/
```

`index.md`電?wiki鞚?map鞙茧 靷毄頃╇媹雼?

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

`log.md`電?maintenance journal搿?靷毄頃╇媹雼?

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

wiki pages鞐愲姅 provenance毳?氤挫〈頃橁赴 鞙勴暣 frontmatter毳?靷毄頃╇媹雼?

```markdown
---
kind: wiki
sources:
  - sources/meeting-2026-06-08.md
  - sources/spec-phase-1.md
updated: 2026-06-08
---

# Project Alpha
```

Source pages電?raw 靸來儨搿?霊?靾?鞛堨毵? 鞝曤炒鞚?於滌矘毳?氇呾嫓頃挫暭 頃╇媹雼?

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent 耄攧

1. Ingest: `sources/` 鞎勲灅 source material鞚?於旉皜頃橁卑雮?鞐呺嵃鞚错姼頃橂悩, 臧€電ロ晿氅?鞗愲 響滍槃鞚?氤挫〈頃╇媹雼? 頃橂倶鞚?source臧€ 10-25臧滌潣 source 氚?wiki pages搿?頇曥灔霅?霑岆姅 `write_files`毳?靷毄頃?鞝勳泊 ingest臧€ 頃橂倶鞚?atomic commit鞙茧 鞝€鞛ル悩瓴?頃╇媹雼?
2. Query: agent鞐愱矊 甏€霠?source 氚?wiki pages毳?鞚疥矊 頃?雼れ潓 `wiki/` 鞎勲灅 updates毳?鞝滌晥頃橁矊 頃╇媹雼?
3. Write: agent臧€ current `parent_commit`鞚?頇曤炒頃?霋れ棎毵?`write_file`, `write_files`, `move_file`, 霕愲姅 `delete_file`鞚?靷毄頃橁矊 頃╇媹雼? page merge, split, archival move鞐愲姅 `move_file`鞚?靷毄頃?git鞚?history毳?鞛冹 鞎婈碃 rename鞙茧 氤搓碃頃?靾?鞛堦矊 頃╇媹雼?
4. Lint: `link_graph`毳?鞁ろ枆頃?orphaned, missing, ambiguous links毳?彀娟碃, 毵堨毵?reviewed commit攵€韯?`changes_since`毳?鞁ろ枆頃?氤€瓴?靷暛鞚?summarize頃╇媹雼?
5. Review: proposed commits毳?inspect頃橁碃 conflicts毳?resolve頃橂┌, 攵堩檿鞁ろ暅 claims電?靷瀸鞚?wiki pages搿?promote頃?霑岅箤歆€ sources鞐?雮波 霊‰媹雼?

v1.2.1鞐愳劀電?鞚?耄攧臧€ 雿?韥?wiki vault鞐?毵炾矊 臁办爼霅橃棃鞀惦媹雼? 鞚缄磩 ingest電?`write_files`搿?鞗愳瀽鞝侅溂搿?鞙犾霅橁碃, 甑“鞝侅澑 韼橃澊歆€ 鞚措彊鞚€ `move_file`搿?旮半鞚?氤挫〈頃橂┌, link/change tools電?靸來暅鞚?鞙犾頃橂┐靹?頃勴劙毵侂悳 paths毳?靾赴瓿? 氚橂车 sync cycles電?臧€電ロ暅 瓴届毎 cached filters, token checks, scans毳?鞛偓鞖╉暕雼堧嫟.

## Lint 耄嫶

臧?maintenance pass 鞚错泟 agent鞐愱矊 雼れ潓鞚?鞖旍箔頃橃劯鞖?

- vault id鞕€ 頃粯 `link_graph`毳?順胳稖頃橁碃 broken links, ambiguous basename links, new orphaned pages毳?氤搓碃頃╇媹雼?
- 毵堨毵?human-reviewed commit瓿?頃粯 `changes_since`毳?順胳稖頃橁碃 added, modified, deleted, renamed pages毳?summarize頃╇媹雼?
- durable wiki pages臧€ 於旉皜霅橃棃鞙茧┐ `index.md`毳?鞐呺嵃鞚错姼頃╇媹雼?
- source material, 氤€瓴诫悳 wiki pages, unresolved questions毳?靹る獏頃橂姅 歆ъ潃 entry毳?`log.md`鞐?於旉皜頃╇媹雼?

Hidden paths電?workflow 鞝勳泊鞐愳劀 hidden 靸來儨搿?鞙犾霅╇媹雼? 鞏措枻 path臧€ SyncPathFilter 霕愲姅 exclude glob鞐?鞚橅暣 瓯半秬霅橂┐ MCP read tools電?file lists, search results, link graphs, change summaries鞐愳劀 頃措嫻 path毳?氤搓碃頃橃 鞎婌姷雼堧嫟.
