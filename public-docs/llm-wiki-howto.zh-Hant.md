# 浣跨敤 PKV Sync 鐨?LLM Wiki 宸ヤ綔娴佺▼

[English](./llm-wiki-howto.md) | [绠€浣撲腑鏂嘳(./llm-wiki-howto.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./llm-wiki-howto.ja.md) | [頃滉淡鞏碷(./llm-wiki-howto.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

PKV Sync 鐐虹敱 LLM 缍鐨?wiki 鎻愪緵鍎插瓨銆佹鍙茶垏 MCP substrate銆備綘鑷繁鐨?MCP-capable agent 璨犺铂鍩疯 LLM锛岄€忛亷鏅€?PKV Sync 瑁濈疆 token 璁€瀵紝涓︽妸姣忓€嬫帴鍙楃殑璁婃洿鎻愪氦鍒扮瓎瑷樺韩鐨?git 姝峰彶銆?

## 涓夊€嬪堡娆?

浣跨敤灏忚€屾槑纰虹殑绲愭锛岃畵浜洪鑸?agent 閮借兘鐞嗚В绛嗚搴€?

- **Sources**锛氬師濮嬬瓎瑷樸€佽布涓婄殑鐮旂┒璩囨枡銆佸尟鍏ユ獢妗堛€佹渻璀伴€愬瓧绋匡紝浠ュ強鍏朵粬璀夋摎銆傜洝閲忚布杩戝師濮嬬礌鏉愪繚瀛橈紝涓﹀寘鍚冻澶犱締婧愯硣瑷婏紝浠ヤ究鏃ュ緦绋芥牳銆?
- **Wiki**锛氱簿绨￠爜闈紝鐢ㄤ締瑾槑鎸佷箙鐨勪簨瀵︺€佹焙绛栥€佹蹇点€佷汉鐗┿€佸皥妗堟垨娴佺▼銆傞€欎簺闋侀潰褰兼閫ｇ祼锛屼甫寮曠敤渚嗘簮闋侀潰銆?
- **Schema**锛氬皯閲忔叄渚嬶紝璁?wiki 鍙互琚?lint锛屼緥濡傚繀濉?frontmatter銆佺储寮曢爜鑸囩董璀锋棩瑾屻€?

PKV Sync 鏄?substrate锛岃€屼笉鏄?LLM host銆傛湇鍕欑鏆撮湶瀹夊叏鐨勮畝鍙栧伐鍏枫€佹▊瑙€瀵叆宸ュ叿銆侀€ｇ祼妾㈡煡鑸囪畩鏇存鏌ワ紱浣犻伕鎿囩殑 agent 鍓囨焙瀹氳鎽樿銆侀噸瀵摢浜涘収瀹癸紝鎴栦綍鏅傝珛浣犵⒑瑾嶃€?

## 閫ｆ帴 agent

寤虹珛鎴栭噸鐢?PKV Sync 瑁濈疆 token锛岀劧寰岀敤 stdio 灏?MCP-capable agent 鎸囧悜鍠竴绛嗚搴細

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

灏嶆柤鏀彺 Streamable HTTP 鐨?agent锛屼綘鍙互鐢ㄥ収宓屾垨鐛ㄧ珛妯″紡鏆撮湶 `/mcp`锛屼甫鍦ㄦ瘡鍊嬭珛姹備笂鍚屾檪鐧奸€侀儴缃查噾閼拌垏 bearer token銆俆ransport 瑭虫儏璜嬪弮闁?MCP access guide銆?

绲?agent 涓€娈电嫻绐勭殑鎸囦护锛氳畝鍙?source 闋侀潰銆佹彁鍑?wiki 鏇存柊銆佸鍏ユ檪浣跨敤涓婃璁€鍙栧緱鍒扮殑 `parent_commit`锛屼甫鍦ㄤ簨瀵︿笉纰哄畾鎴栧嚭鐝捐绐佹檪鍋滄锛岀瓑寰呬汉宸ュ鏌ャ€?

## 寤鸿 schema

寰為€欏€嬬増闈㈤厤缃枊濮嬶紝鍙湁鐣跺畠灏嶄綘鐨勫伐浣滄祦绋嬩締瑾お灏忔檪鎵嶈鏁达細

```text
index.md
log.md
sources/
wiki/
```

浣跨敤 `index.md` 浣滅偤 wiki 鍦板湒锛?

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

浣跨敤 `log.md` 浣滅偤缍鏃ヨ獙锛?

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

鍦?wiki 闋侀潰浣跨敤 frontmatter 淇濈暀渚嗘簮鑴堢怠锛?

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

Source 闋侀潰鍙互淇濇寔鍘熷鐙€鎱嬶紝浣嗘噳瑭叉鏄庤硣瑷婁締婧愶細

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent 寰挵

1. Ingest锛氬湪 `sources/` 涓嬫柊澧炴垨鏇存柊 source 鏉愭枡锛岀洝閲忎繚鐣欏師濮嬫帾杈€傜暥涓€鍊?source 鏈冨睍闁嬫垚 10 鍒?25 鍊?source 鑸?wiki 闋侀潰鏅傦紝浣跨敤 `write_files`锛岃畵鏁村€?ingest 浠ヤ竴鍊嬪師瀛?commit 钀藉湴銆?
2. Query锛氳珛 agent 璁€鍙栫浉闂?source 鑸?wiki 闋侀潰锛岀劧寰屾彁鍑?`wiki/` 涓嬬殑鏇存柊銆?
3. Write锛氬彧鏈夊湪 agent 鎿佹湁鐩墠鐨?`parent_commit` 涔嬪緦锛屾墠鍏佽ū瀹冧娇鐢?`write_file`銆乣write_files`銆乣move_file` 鎴?`delete_file`銆傞爜闈㈠悎浣点€佹媶鍒嗗拰姝告獢绉诲嫊鏅備娇鐢?`move_file`锛岃畵 git 鑳藉洖鍫遍噸鏂板懡鍚嶏紝鑰屼笉鏄伜澶辨鍙层€?
4. Lint锛氬煼琛?`link_graph` 鎵惧嚭瀛ょ珛銆佺己澶辨垨 ambiguous 閫ｇ祼锛涘緸涓婃瀵╂煡閬庣殑 commit 鍩疯 `changes_since`锛屾憳瑕佽畩鏇村収瀹广€?
5. Review锛氭鏌ユ彁鍑虹殑 commits銆佽В姹鸿绐侊紝涓﹀皣涓嶇⒑瀹氱殑涓诲嫉淇濈暀鍦?sources 涓紝鐩村埌浜洪灏囧畠鍊戞彁鍗囧埌 wiki 闋侀潰銆?

鍦?v1.2.1 涓紝閫欏€嬪惊鐠版洿閬╁悎澶у瀷 wiki 绛嗚搴細鎵规 ingest 绻肩簩閫忛亷 `write_files` 淇濇寔鍘熷瓙鎬э紝绲愭鎬х殑闋侀潰绉诲嫊閫忛亷 `move_file` 淇濈暀姝峰彶锛岄€ｇ祼鍜岃畩鏇村伐鍏蜂繚鎸佹湁鐣屼甫闅辫棌琚亷婵捐矾寰戯紝閲嶈鍚屾閫辨湡鏈冪洝鍙兘閲嶇敤绡╅伕鍣ㄣ€乼oken 妾㈡煡鍜屾巸鎻忕祼鏋滃揩鍙栥€?

## Lint 渚嬭娴佺▼

姣忔缍瀹屾垚寰岋紝璜?agent锛?

- 浣跨敤 vault id 鍛煎彨 `link_graph`锛屼甫鍥炲牨鏂疯閫ｇ祼銆乤mbiguous basename links锛屼互鍙婃柊鐨勫绔嬮爜闈紱
- 浣跨敤涓婃浜哄伐瀵╂煡閬庣殑 commit 鍛煎彨 `changes_since`锛屼甫鎽樿鏂板銆佷慨鏀广€佸埅闄よ垏閲嶆柊鍛藉悕鐨勯爜闈紱
- 鏂板鎸佷箙 wiki 闋侀潰鏅傦紝鏇存柊 `index.md`锛?
- 鍦?`log.md` 闄勫姞涓€鍓囩煭绱€閷勶紝鎻忚堪渚嗘簮鏉愭枡銆佽畩鏇寸殑 wiki 闋侀潰锛屼互鍙婃湭瑙ｅ晱椤屻€?

Hidden paths 鍦ㄦ暣鍊嬪伐浣滄祦绋嬩腑閮芥渻淇濇寔闅辫棌銆傚鏋滄煇鍊嬭矾寰戣 SyncPathFilter 鎴?exclude glob 鎷掔禃锛孧CP 璁€鍙栧伐鍏蜂笉鏈冨湪妾旀鍒楄〃銆佹悳灏嬬祼鏋溿€侀€ｇ祼鍦栨垨璁婃洿鎽樿涓洖鍫卞畠銆?
