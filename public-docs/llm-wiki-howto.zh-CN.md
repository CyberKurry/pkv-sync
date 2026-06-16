# 浣跨敤 PKV Sync 鐨?LLM Wiki 宸ヤ綔娴?

[English](./llm-wiki-howto.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./llm-wiki-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./llm-wiki-howto.ja.md) | [頃滉淡鞏碷(./llm-wiki-howto.ko.md)

鏂囨。鐗堟湰锛歷1.4.5銆?

PKV Sync 涓虹敱 LLM 缁存姢鐨?wiki 鎻愪緵瀛樺偍銆佸巻鍙插拰 MCP 鍩哄簳銆備綘鑷繁鐨?MCP-capable agent 璐熻矗杩愯 LLM锛岄€氳繃鏅€氱殑 PKV Sync 璁惧 token 璇诲啓锛屽苟鎶婃瘡涓凡鎺ュ彈鐨勬敼鍔ㄦ彁浜ゅ埌绗旇搴撶殑 git 鍘嗗彶涓€?

## 涓夊眰缁撴瀯

浣跨敤涓€涓皬鑰屾槑纭殑缁撴瀯锛岃浜虹被鍜?agent 閮借兘鐞嗚В绗旇搴撱€?

- **Sources**锛氬師濮嬬瑪璁般€佺矘璐寸殑鐮旂┒鏉愭枡銆佸鍏ユ枃浠躲€佷細璁褰曪紝浠ュ強鍏朵粬璇佹嵁銆傚敖閲忚创杩戝師濮嬫潗鏂欙紝骞跺寘鍚冻澶熺殑鏉ユ簮淇℃伅锛屾柟渚夸互鍚庡璁°€?
- **Wiki**锛氱畝娲侀〉闈紝鐢ㄦ潵瑙ｉ噴闀挎湡鏈夋晥鐨勪簨瀹炪€佸喅绛栥€佹蹇点€佷汉鐗┿€侀」鐩垨娴佺▼銆傝繖浜涢〉闈㈠郊姝ら摼鎺ワ紝骞跺紩鐢?source 椤甸潰銆?
- **Schema**锛氬皯閲忕害瀹氾紝璁?wiki 鍙互琚?lint锛屼緥濡傚繀闇€鐨?frontmatter銆佺储寮曢〉鍜岀淮鎶ゆ棩蹇椼€?

PKV Sync 鏄熀搴曪紝涓嶆槸 LLM host銆傛湇鍔＄鏆撮湶瀹夊叏璇诲彇宸ュ叿銆佷箰瑙傚啓鍏ュ伐鍏枫€侀摼鎺ユ鏌ュ拰鍙樻洿妫€鏌ワ紱浣犻€夋嫨鐨?agent 鍐冲畾瑕佹€荤粨銆侀噸鍐欏摢浜涘唴瀹癸紝鎴栦綍鏃惰浣犵‘璁ゃ€?

## 杩炴帴 agent

鍒涘缓鎴栧鐢ㄤ竴涓?PKV Sync 璁惧 token锛岀劧鍚庨€氳繃 stdio 灏?MCP-capable agent 鎸囧悜鍗曚釜绗旇搴擄細

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

瀵逛簬鏀寔 Streamable HTTP 鐨?agent锛屼綘鍙互鐢ㄥ祵鍏ユā寮忔垨鐙珛妯″紡鏆撮湶 `/mcp`锛屽苟鍦ㄦ瘡涓姹備腑鍚屾椂鍙戦€侀儴缃插瘑閽ュ拰 bearer token銆倀ransport 缁嗚妭璇峰弬瑙?MCP access guide銆?

缁?agent 涓€涓寖鍥村緢绐勭殑鎸囦护锛氳鍙?source 椤甸潰銆佹彁鍑?wiki 鏇存柊銆佸啓鍏ユ椂浣跨敤涓婃璇诲彇寰楀埌鐨?`parent_commit`锛屽苟鍦ㄤ簨瀹炰笉纭畾鎴栧嚭鐜板啿绐佹椂鍋滀笅鏉ョ瓑寰呬汉宸?review銆?

## 鎺ㄨ崘 schema

浠庤繖涓竷灞€寮€濮嬶紝鍙湁褰撳畠瀵逛綘鐨勫伐浣滄祦鏉ヨ澶皬鏃跺啀璋冩暣锛?

```text
index.md
log.md
sources/
wiki/
```

浣跨敤 `index.md` 浣滀负 wiki 鍦板浘锛?

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

浣跨敤 `log.md` 浣滀负缁存姢鏃ュ織锛?

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

鍦?wiki 椤甸潰涓婁娇鐢?frontmatter 鏉ヤ繚鐣欐潵婧愶細

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

Source 椤甸潰鍙互淇濇寔鍘熷鐘舵€侊紝浣嗗簲璇存槑淇℃伅鏉ヨ嚜鍝噷锛?

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent 寰幆

1. Ingest锛氬湪 `sources/` 涓嬫柊澧炴垨鏇存柊 source 鏉愭枡锛屽敖閲忎繚鐣欏師濮嬫帾杈炪€傚綋涓€涓?source 浼氬睍寮€鎴?10 鍒?25 涓?source 鍜?wiki 椤甸潰鏃讹紝浣跨敤 `write_files`锛岃鏁翠釜 ingest 浠ヤ竴涓師瀛?commit 钀藉湴銆?
2. Query锛氳姹?agent 璇诲彇鐩稿叧 source 鍜?wiki 椤甸潰锛岀劧鍚庢彁鍑?`wiki/` 涓嬬殑鏇存柊銆?
3. Write锛氬彧鏈夊湪 agent 鎷垮埌褰撳墠 `parent_commit` 鍚庯紝鎵嶅厑璁稿畠浣跨敤 `write_file`銆乣write_files`銆乣move_file` 鎴?`delete_file`銆傞〉闈㈠悎骞躲€佹媶鍒嗗拰褰掓。绉诲姩鏃朵娇鐢?`move_file`锛岃 git 鑳芥姤鍛婇噸鍛藉悕锛岃€屼笉鏄涪澶卞巻鍙层€?
4. Lint锛氳繍琛?`link_graph` 鏌ユ壘瀛ょ珛閾炬帴銆佺己澶遍摼鎺ユ垨鏈夋涔夌殑閾炬帴锛涗粠涓婃 review 杩囩殑 commit 寮€濮嬭繍琛?`changes_since`锛屾€荤粨鍙戠敓浜嗕粈涔堝彉鍖栥€?
5. Review锛氭鏌ユ彁鍑虹殑 commit锛岃В鍐冲啿绐侊紝骞舵妸涓嶇‘瀹氱殑涓诲紶鐣欏湪 sources 涓紝鐩村埌浜虹被灏嗗叾鎻愬崌涓?wiki 椤甸潰銆?

鍦?v1.2.1 涓紝杩欎釜寰幆鏇撮€傚悎澶у瀷 wiki 绗旇搴擄細鎵归噺 ingest 缁х画閫氳繃 `write_files` 淇濇寔鍘熷瓙鎬э紝缁撴瀯鎬х殑椤甸潰绉诲姩閫氳繃 `move_file` 淇濈暀鍘嗗彶锛岄摼鎺ュ拰鍙樻洿宸ュ叿淇濇寔鏈夌晫骞堕殣钘忚杩囨护璺緞锛岄噸澶嶅悓姝ュ懆鏈熶細灏藉彲鑳藉鐢ㄨ繃婊ゅ櫒銆乼oken 妫€鏌ュ拰鎵弿缁撴灉缂撳瓨銆?

## Lint 渚嬭娴佺▼

姣忔缁存姢瀹屾垚鍚庯紝璇?agent锛?

- 鐢?vault id 璋冪敤 `link_graph`锛屽苟鎶ュ憡鏂摼銆佹湁姝т箟鐨?basename 閾炬帴锛屼互鍙婃柊澧炵殑瀛ょ珛椤甸潰锛?
- 鐢ㄤ笂娆′汉宸?review 杩囩殑 commit 璋冪敤 `changes_since`锛屽苟鎬荤粨鏂板銆佷慨鏀广€佸垹闄ゅ拰閲嶅懡鍚嶇殑椤甸潰锛?
- 褰撴柊澧炰簡闀挎湡鏈夋晥鐨?wiki 椤甸潰鏃讹紝鏇存柊 `index.md`锛?
- 鍚?`log.md` 杩藉姞涓€鏉＄畝鐭褰曪紝璇存槑 source 鏉愭枡銆佹敼鍔ㄨ繃鐨?wiki 椤甸潰锛屼互鍙婃湭瑙ｅ喅鐨勯棶棰樸€?

闅愯棌璺緞浼氬湪鏁翠釜宸ヤ綔娴佷腑淇濇寔闅愯棌銆傚鏋滄煇涓矾寰勮 SyncPathFilter 鎴?exclude glob 鎷掔粷锛孧CP 璇诲彇宸ュ叿涓嶄細鍦ㄦ枃浠跺垪琛ㄣ€佹悳绱㈢粨鏋溿€侀摼鎺ュ浘鎴栧彉鏇存憳瑕佷腑鎶ュ憡瀹冦€?
