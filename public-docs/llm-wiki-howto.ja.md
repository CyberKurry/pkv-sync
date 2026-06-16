# PKV Sync 銇т綔銈?LLM Wiki 銉兗銈儠銉兗

[English](./llm-wiki-howto.md) | [绠€浣撲腑鏂嘳(./llm-wiki-howto.zh-CN.md) | [绻侀珨涓枃](./llm-wiki-howto.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./llm-wiki-howto.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

銇撱伄鏂囨浉銇姊扮炕瑷炽伀銈堛倠鍒濈増銇с仚銆傚叕闁嬪墠銇儘銈ゃ儐銈ｃ儢瑭辫€呫伀銈堛倠銉儞銉ャ兗銈掓帹濂ㄣ仐銇俱仚銆?

PKV Sync 銇€丩LM 銇屼繚瀹堛仚銈?wiki 銇仧銈併伄 storage銆乭istory銆丮CP substrate 銈掓彁渚涖仐銇俱仚銆傘儲銉笺偠銉艰嚜韬伄 MCP 瀵惧繙 agent 銇?LLM 銈掑疅琛屻仐銆侀€氬父銇?PKV Sync device token 銇ц銇挎浉銇嶃仐銆佹壙瑾嶃仌銈屻仧銇欍伖銇︺伄澶夋洿銈?vault 銇?git history 銇?commit 銇椼伨銇欍€?

## 3 銇ゃ伄灞?

浜洪枔銇?agent 銇浮鏂广亴 vault 銈掔悊瑙ｃ仹銇嶃倠銈堛亞銇€佸皬銇曘亸鏄庣ず鐨勩仾妲嬮€犮倰浣裤亜銇俱仚銆?

- **Sources**: raw notes銆佽布銈婁粯銇戙仧 research銆乮mported files銆乵eeting transcripts銆併仢銇粬銇?evidence銆傚師璩囨枡銇繎銇勫舰銇ф畫銇椼€佸緦銇嬨倝 audit 銇с亶銈嬨仩銇戙伄 provenance 銈掑惈銈併伨銇欍€?
- **Wiki**: durable facts銆乨ecisions銆乧oncepts銆乸eople銆乸rojects銆乸rocesses 銈掔啊娼斻伀瑾槑銇欍倠 pages銆傘亾銈屻倝銇?pages 銇簰銇勩伀 link 銇椼€乻ource pages 銈?cite 銇椼伨銇欍€?
- **Schema**: wiki 銈?lintable 銇仚銈嬪皯鏁般伄 conventions銆俽equired frontmatter銆乮ndex page銆乵aintenance log 銇仼銇с仚銆?

PKV Sync 銇?substrate 銇с亗銈娿€丩LM host 銇с伅銇傘倞銇俱仜銈撱€俿erver 銇?safe read tools銆乷ptimistic write tools銆乴ink inspection銆乧hange inspection 銈掑叕闁嬨仐銇俱仚銆備綍銈?summarize 銇椼€乺ewrite 銇椼€佺⒑瑾嶃倰姹傘倎銈嬨亱銇€併儲銉笺偠銉笺亴閬搞伓 agent 銇屽垽鏂仐銇俱仚銆?

## Agent 銈掓帴缍氥仚銈?

PKV Sync device token 銈掍綔鎴愩伨銇熴伅鍐嶅埄鐢ㄣ仐銆乻tdio 銇?MCP 瀵惧繙 agent 銈掑崢涓€銇?vault 銇悜銇戙伨銇欍€?

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

Streamable HTTP 銈掋偟銉濄兗銉堛仚銈?agent 銇с伅銆乪mbedded 銇俱仧銇?standalone mode 銇?`/mcp` 銈掑叕闁嬨仐銆併仚銇广仸銇?request 銇?deployment key 銇?bearer token 銈掗€佷俊銇с亶銇俱仚銆倀ransport 銇┏绱般伅 MCP access guide 銈掑弬鐓с仐銇︺亸銇犮仌銇勩€?

agent 銇伅鐙亜 instruction 銈掓浮銇椼伨銇欍€俿ource pages 銈掕銇裤€亀iki updates 銈掓彁妗堛仐銆佹浉銇嶈炯銇挎檪銇伅鏈€寰屻伄 read 銇у緱銇?`parent_commit` 銈掍娇銇勩€乫acts 銇屼笉纰恒亱銇俱仧銇?conflicts 銇屽嚭銇熴倝 human review 銇仧銈併伀鍋滄銇欍倠銈堛亞鎸囩ず銇椼伨銇欍€?

## 鎺ㄥエ schema

銇俱仛銇撱伄 layout 銇嬨倝濮嬨倎銆亀orkflow 銇銇椼仸灏忋仌銇欍亷銈嬨仺鎰熴仒銇熴仺銇嶃仩銇戣鏁淬仐銇俱仚銆?

```text
index.md
log.md
sources/
wiki/
```

`index.md` 銇?wiki 銇?map 銇ㄣ仐銇︿娇銇勩伨銇欍€?

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

`log.md` 銇?maintenance journal 銇ㄣ仐銇︿娇銇勩伨銇欍€?

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

wiki pages 銇伅 provenance 銈掓畫銇欍仧銈併伀 frontmatter 銈掍娇銇勩伨銇欍€?

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

Source pages 銇?raw 銇伨銇俱伀銇с亶銇俱仚銇屻€佹儏鍫便伄 origin 銈掓槑瑷樸仐銇俱仚銆?

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent loop

1. Ingest: `sources/` 銇笅銇?source material 銈掕拷鍔犮伨銇熴伅鏇存柊銇椼€佸彲鑳姐仾闄愩倞鍏冦伄 wording 銈掍繚銇°伨銇欍€? 銇ゃ伄 source 銇?10-25 鍊嬨伄 source pages 銇?wiki pages 銇睍闁嬨仌銈屻倠鍫村悎銇€乣write_files` 銈掍娇銇勩€乮ngest 鍏ㄤ綋銈?1 銇ゃ伄 atomic commit 銇ㄣ仐銇︿繚瀛樸仐銇俱仚銆?
2. Query: agent 銇枹閫ｃ仚銈?source pages 銇?wiki pages 銈掕銇俱仜銆乣wiki/` 銇洿鏂版銈掑嚭銇曘仜銇俱仚銆?
3. Write: agent 銇?current `parent_commit` 銈掓寔銇ｃ仸銇勩倠鍫村悎銇犮亼銆乣write_file`銆乣write_files`銆乣move_file`銆併伨銇熴伅 `delete_file` 銈掍娇銈忋仜銇俱仚銆俻age merge銆乻plit銆乤rchive move 銇伅 `move_file` 銈掍娇銇勩€乬it 銇?history 銈掑け銈忋仛 rename 銇ㄣ仐銇﹀牨鍛娿仹銇嶃倠銈堛亞銇仐銇俱仚銆?
4. Lint: `link_graph` 銈掑疅琛屻仐銇?orphaned銆乵issing銆乤mbiguous links 銈掓帰銇椼€佹渶寰屻伀浜洪枔銇?review 銇椼仧 commit 銇嬨倝 `changes_since` 銈掑疅琛屻仐銇﹀鏇村唴瀹广倰 summarize 銇椼伨銇欍€?
5. Review: proposed commits 銈?inspect 銇椼€乧onflicts 銈?resolve 銇椼€佷笉纰恒亱銇?claims 銇汉闁撱亴 wiki pages 銇?promote 銇欍倠銇俱仹 sources 銇畫銇椼伨銇欍€?

v1.2.1 銇с伅銆併亾銇?loop 銇屻倛銈婂ぇ銇嶃仾 wiki vault 鍚戙亼銇鏁淬仌銈屻仸銇勩伨銇欍€備竴鎷?ingest 銇?`write_files` 銇у師瀛愮殑銇繚銇熴倢銆佹閫犵殑銇儦銉笺偢绉诲嫊銇?`move_file` 銇у饱姝淬倰淇濇寔銇椼€乴ink/change tools 銇笂闄愪粯銇嶃伄銇俱伨銉曘偅銉偪銉兼笀銇?path 銈掗殸銇椼€佺拱銈婅繑銇椼伄 sync cycles 銇彲鑳姐仾闄愩倞 cached filters銆乼oken checks銆乻cans 銈掑啀鍒╃敤銇椼伨銇欍€?

## Lint routine

鍚?maintenance pass 銇緦銆乤gent 銇銈掍緷闋笺仐銇俱仚銆?

- vault id 銈掓寚瀹氥仐銇?`link_graph` 銈掑懠銇冲嚭銇椼€乥roken links銆乤mbiguous basename links銆佹柊銇椼亜 orphaned pages 銈掑牨鍛娿仚銈嬨€?
- 鏈€寰屻伀 human-reviewed 銇曘倢銇?commit 銈掓寚瀹氥仐銇?`changes_since` 銈掑懠銇冲嚭銇椼€乤dded銆乵odified銆乨eleted銆乺enamed pages 銈?summarize 銇欍倠銆?
- durable wiki pages 銇岃拷鍔犮仌銈屻仧鍫村悎銇?`index.md` 銈掓洿鏂般仚銈嬨€?
- source material銆佸鏇淬仌銈屻仧 wiki pages銆乽nresolved questions 銈掕鏄庛仚銈嬬煭銇?entry 銈?`log.md` 銇拷鍔犮仚銈嬨€?

Hidden paths 銇?workflow 鍏ㄤ綋銇?hidden 銇伨銇俱仹銇欍€俻ath 銇?SyncPathFilter 銇俱仧銇?exclude glob 銇嫆鍚︺仌銈屻仧鍫村悎銆丮CP read tools 銇?file lists銆乻earch results銆乴ink graphs銆乧hange summaries 銇с仢銇?path 銈掑牨鍛娿仐銇俱仜銈撱€?
