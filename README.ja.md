# PKV Sync

**Obsidian 銉溿兗銉儓銈掋偦銉儠銉涖偣銉堛仹銆?* PKV Sync 銇嚜鍓嶃伄銈点兗銉愩兗涓娿仹鍕曘亶銆?
銈广優銉涖€併偪銉栥儸銉冦儓銆併儑銈广偗銉堛儍銉椼伄闁撱仹 Obsidian 銉溿兗銉儓銈掑悓鏈熴仐缍氥亼銇俱仚銆?
銉愩偆銉娿儶銇层仺銇ゃ€丼QLite 銉囥兗銈裤儥銉笺偣銇层仺銇ゃ€併儨銉笺儷銉堛仈銇ㄣ伀 bare 銇?Git
銉儩銈搞儓銉伈銇ㄣ仱 鈥?銈儵銈广偪銉笺倐 S3 銈傘優銉嶃兗銈搞儔銈儵銈︺儔銈備笉瑕併仹銇欍€?
銈ゃ兂銈广儓銉笺儷銇椼仸銆丱bsidian 銇嬨倝鎸囥仐绀恒仜銇般€併儙銉笺儓銇屽悓鏈熴仌銈屻伨銇欍€?

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.4銆?

[English](./README.md) | [绠€浣撲腑鏂嘳(./README.zh-CN.md) | [绻侀珨涓枃](./README.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./README.ko.md)

## 姗熻兘

- **銉炪儷銉併儲銉笺偠銉笺€併優銉儊銉溿兗銉儓**銇悓鏈熴€傝獚瑷兼笀銇裤儑銉愩偆銈硅秺銇椼伀銆?
  銉溿兗銉儓鍗樹綅銇?push lock 銇ㄥ啰绛夈儶銉堛儵銈や粯銇嶃仹鍕曘亶銇俱仚銆?
- **銉偄銉偪銈ゃ儬 push**銆傚皬銇曘仾绶ㄩ泦銇?Server-Sent Events 绲岀敱銇?1 绉掓湭婧€銇?
  灞娿亶銇俱仚銆傘儩銉笺儶銉炽偘銇繚闄恒仺銇椼仸娈嬨倞銇俱仚銆?
- **Git 銇屼俊闋笺仹銇嶃倠鍞竴銇儏鍫辨簮**銆傘仚銇广仸銇儨銉笺儷銉堛伅 bare 銇?Git
  銉儩銈搞儓銉仾銇仹銆併儠銈°偆銉崢浣嶃伄灞ユ銆乽nified diff銆佸崢涓€銉曘偂銈ゃ儷銇?
  寰╁厓銇屾婧栥仹鍕曘亶銇俱仚 鈥?銉椼儵銈般偆銉炽仹銈傜鐞嗐儜銉嶃儷銇с倐銆?
- **銈炽兂銉曘儶銈儓銇挤銇?*銆傘儣銉┿偘銈ゃ兂銇儹銉笺偒銉法闆嗐倰榛欍仯銇︿笂鏇搞亶銇椼伨銇涖倱銆?
  銈炽兂銉曘儶銈儓銇?`.conflict-*` 銉曘偂銈ゃ儷銇ㄣ仐銇﹁銇堛倠鍖栥仌銈屻€併儻銉炽偗銉儍銈?
  銉偩銉儛銇цВ姹恒仹銇嶃伨銇欍€?
- **绠＄悊銉戙儘銉?*銇?5 瑷€瑾炲蹇滐紙English銆佺畝涓€佺箒涓€佹棩鏈獮銆來暅甑柎锛夈€?
  銉︺兗銈躲兗銆併儑銉愩偆銈广儓銉笺偗銉炽€併儨銉笺儷銉堛€佹嫑寰呫€併偄銈儐銈ｃ儞銉嗐偅銆乥lob GC 銈?
  銇撱亾銇嬨倝鎿嶄綔銇椼€佺牬澹婄殑銇儨銉笺儷銉堟搷浣溿仺銉︺兗銈躲兗鎿嶄綔銇伅纰鸿獚銉€銈ゃ偄銉偘銈掕〃绀恒仐銇俱仚銆?
- **AI 銇嬨倝瑾倎銈?vault**銆侻CP 銇?stdio銆佺嫭绔嬨仐銇?Streamable HTTP銆併伨銇熴伅 `pkvsyncd serve` 銇煁銈佽炯銇俱倢銇?`/mcp` 銉兗銉堛仹 read/write tools 銈掑叕闁嬨仐銇俱仚銆?
- **鏃㈠畾銇т笂闄愪粯銇?*銆傜鐞嗚€呫亴浣滄垚锛忋儶銈汇儍銉堛仚銈嬨儜銈广儻銉笺儔銇?setup 銇ㄥ悓銇樺挤搴︺儩銉偡銉笺倰浣裤亜銆乼oken 銇钩鏂囥伅涓€搴︺仩銇戣〃绀恒仌銈屻€乽pload 銇?MCP response 銇偟銈ゃ偤涓婇檺銇у畧銈夈倢銆乴ive SSE stream 銇彇銈婃秷銇曘倢銇?token 銈掑啀妞滆銇椼伨銇欍€?
- **閫€灞堛仾銇ゃ亸銈娿伅鎰忓洺鐨?*銆傘儛銈ゃ儕銉伈銇ㄣ仱銆丼QLite 銉°偪銉囥兗銈?DB 銇层仺銇ゃ€?
  銉溿兗銉儓銇斻仺銇?bare Git 銉儩銈搞儓銉伈銇ㄣ仱銆佹坊浠樸仈銇ㄣ伀 content-addressed 銇?
  blob 銇层仺銇ゃ€?

## Docker Compose 銇с仚銇愬銈併倠

銇撱倢銇屾帹濂ㄣ儷銉笺儓銇с仚銆俙deploy/caddy/` 銇?Caddy 銇?Let's Encrypt 銇?HTTPS 銈?
銇曘伆銇嶃€丳KV Sync 銇?compose 銉嶃儍銉堛儻銉笺偗鍐呫伄 `127.0.0.1:6710` 銇у緟銇″彈銇戙€?
鍏枊銈ゃ兂銈裤兗銉嶃儍銉堛亱銈夈伄骞虫枃 HTTP 銇伅涓€鍒囪Е銈屻伨銇涖倱銆?

銉夈儭銈ゃ兂鍚嶏紙渚嬶細`sync.example.com`锛夈伄 A/AAAA 銉偝銉笺儔銈掋偟銉笺儛銉笺伀鍚戙亼銇?
銇娿亶銆併儩銉笺儓 `80` 銇?`443` 銈掋偆銉炽偪銉笺儘銉冦儓銇嬨倝鍒伴仈鍙兘銇仐銇︺亸銇犮仌銇?
锛堛儩銉笺儓 80 銇?ACME HTTP-01 妞滆銇繀瑕併仹銇欙級銆?

1. 銉囥儣銉偆銉°兂銉堛偔銉笺倰鐢熸垚銇椼伨銇欍€?

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` 銇殻銇?`config.toml` 銈掔疆銇嶃伨銇欍€?

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 銇嚭鍔涖伀缃亶鎻涖亪銈?
   public_host    = "sync.example.com"   # 蹇呴爤銆乤dmin POST 銇岄€氥倠銈堛亞銇仾銈娿伨銇?

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker 銉栥儶銉冦偢銉嶃儍銉堛儻銉笺偗

   [mcp]
   embed_in_serve = false                # true 銇с亾銇偟銉笺儛銉笺伀 /mcp 銈掋優銈︺兂銉?
   ```

3. `deploy/caddy/Caddyfile` 銈掔法闆嗐仐銆乣sync.example.com` 銈掑疅闅涖伄銉夈儭銈ゃ兂銇?
   缃亶鎻涖亪銇俱仚銆?

4. 銈广偪銉冦偗銈掔珛銇′笂銇掋伨銇欍€?

   ```bash
   docker compose up -d
   ```

   銉栥儵銈︺偠銇?`https://sync.example.com/setup` 銈掗枊銇嶃€佹渶鍒濄伄绠＄悊鑰呫偄銈偊銉炽儓銈?
   浣滄垚銇椼伨銇欍€?

5. Obsidian 銇?`pkv-sync-plugin.zip` 銈掋偆銉炽偣銉堛兗銉?
   锛坄<vault>/.obsidian/plugins/pkv-sync/`锛夈仐銆佹湁鍔瑰寲銇椼仧銇傘仺銆佺鐞嗐儜銉嶃儷銇?
   鍏辨湁 URL 銈掕布銈婁粯銇戙€併儹銈般偆銉炽伨銇熴伅鐧婚尣銇椼仸銉溿兗銉儓銈掗伕銇炽伨銇欍€?

鏇存柊銇?`docker compose pull && docker compose up -d` 銇с仚銆傘儘銈ゃ儐銈ｃ儢
銈ゃ兂銈广儓銉笺儷銆併儶銉愩兗銈广儣銉偔銈枫伄銉併儱銉笺儖銉炽偘锛圕addy / Nginx / Traefik锛夈€?
`public_host` 銇剰鍛炽€併儛銉冦偗銈儍銉楋紡銉偣銉堛偄銆併儑銈ｃ偣銈殫鍙峰寲銇仱銇勩仸銇?
[銉囥儣銉偆寮峰寲銈偆銉塢(./public-docs/deployment-hardening.ja.md) 銈掑弬鐓с仐銇︺亸銇犮仌銇勩€?

## MCP 銉囥儣銉偆銉兗銉?

PKV Sync 銇?MCP Streamable HTTP transport 銈?2 閫氥倞銇у叕闁嬨仹銇嶃伨銇欍€傚煁銈佽炯銇?
銉兗銉夈伅鏄庣ず鐨勩伀鏈夊姽鍖栥仐銇俱仚銆俙[mcp].embed_in_serve = true` 銈掕ō瀹氥仚銈嬨仺銆?
`pkvsyncd serve` 銇屻儭銈ゃ兂銈点兗銉愩兗銉濄兗銉堛伀 `/mcp` 銈掋優銈︺兂銉堛仐銆佸悓銇?TLS
绲傜銆併儶銉愩兗銈广儣銉偔銈枫€併儑銉椼儹銈ゃ儭銉炽儓銈兗銆乥earer token 妞滆銈掑叡鏈夈仐銇俱仚銆?
銈广偪銉炽儔銈儹銉炽儮銉笺儔銇緭鏉ャ仼銇娿倞鍒ャ儣銉偦銈广仹銇? `pkvsyncd mcp --transport
http --bind 127.0.0.1:6711`銆侻CP 銈掗殧闆仐銇熴亜鍫村悎銆佸皞鐢?bind address 銈掍娇銇?
鍫村悎銆併伨銇熴伅鐙珛銇椼仸銈广偙銉笺儷銇椼仧銇勫牬鍚堛伀渚垮埄銇с仚銆?

## Obsidian 銉椼儵銈般偆銉?

銉兗銈儷銉曘偂銈ゃ儷銇屼俊闋笺仹銇嶃倠鎯呭牨婧愩仹銇?鈥?銉椼儵銈般偆銉炽伅銉囥偅銈广偗涓娿伄閫氬父銇?
Obsidian 銉溿兗銉儓銈掋仢銇伨銇捐銇挎浉銇嶃仐銇俱仚銆傘儣銉偔銈枫儠銈°偆銉偡銈广儐銉犮伅
銇傘倞銇俱仜銈撱€傘儣銉┿偘銈ゃ兂銇潪姗熷瘑瑷畾銇ㄥ悓鏈熴偆銉炽儑銉冦偗銈广伅
`<vault>/.obsidian/plugins/pkv-sync/data.json` 銇繚瀛樸仌銈屻伨銇欍€傘儹銈般偆銉崇姸鎱嬨€?
鐝惧湪銇?bearer 銉囥儛銈ゃ偣銉堛兗銈兂銆併儑銉椼儹銈ゃ儭銉炽儓銈兗銆佸畨瀹氥仐銇熴儑銉愩偆銈?ID 銇?
Obsidian 銇儑銉愩偆銈广儹銉笺偒銉偣銉堛儸銉笺偢銇繚瀛樸仌銈屻伨銇欍€侽bsidian 銇?
銉囥儛銈ゃ偣銉兗銈儷銈广儓銉兗銈搞€佸钩鏂囥儛銉冦偗銈儍銉椼€佹棫銉愩兗銈搞儳銉炽亱銈夋畫銇ｃ仧銉椼儵銈般偆銉?
`data.json` 銇偝銉斻兗銇瀵嗐仺銇椼仸鎵便仯銇︺亸銇犮仌銇勩€傘儑銉愩偆銈广儓銉笺偗銉炽伅浣跨敤鏅傘伀鏇存柊銇曘倢銆?
90 鏃ャ偄銈ゃ儔銉仹澶卞姽銇椼€佸悇銉堛兗銈兂銇伅 365 鏃ャ伄绲跺鏈夊姽鏈熼檺銇屻亗銈娿伨銇欍€傚悓銇樸儑銉愩偆銈广仹鍐嶃儹銈般偆銉炽仚銈嬨仺鏈夊姽銇儓銉笺偗銉炽亴鍏ャ倢鏇裤倧銈娿伨銇欍€?

鏃ャ€呫伄姗熻兘 鈥?銈炽優銉炽儔銉戙儸銉冦儓銆併儠銈°偆銉饱姝淬€併偟銈ゃ儔銉愩偆銈点偆銉?diff銆?
銈炽兂銉曘儶銈儓瑙ｆ焙銆乣.obsidian` 銇伕鎶炵殑鍚屾湡銆併儑銉愩偆銈圭鐞嗐€併偦銉儠
銈儍銉椼儑銉笺儓 鈥?銇痆銉︺兗銈躲兗銉炪儖銉ャ偄銉玗(./public-docs/user-manual.ja.md)銇?
銇层仺銇ㄣ亰銈婅В瑾仐銇︺亜銇俱仚銆?

## 鐝炬檪鐐广伄鏆楀彿鍖栥伀銇ゃ亜銇?

PKV Sync 1.0 銇伨銇犮儘銈ゃ儐銈ｃ儢銇?End-to-End 鏆楀彿鍖栥倰 **鍚屾⒈銇椼仸銇勩伨銇涖倱** 鈥?
銈点兗銉愩兗銇儨銉笺儷銉堛伄鍐呭銈掕銈併伨銇欍€傘儨銉笺儷銉堝崢浣嶃伄銉嶃偆銉嗐偅銉?E2EE 銇?1.x
銉兗銉夈優銉冦儣銇偑銉椼儓銈ゃ兂姗熻兘銇ㄣ仐銇﹁▓鐢汇仐銇︺亜銇俱仚銇屻€佹殫鍙峰寲銈掑叆銈屻倠銇?
Git-native 銇?PKV 銈掓湁鐢ㄣ伀銇椼仸銇勩倠姗熻兘锛堝饱姝?diff銆佷笁鑰呰嚜鍕曘優銉笺偢銆丼SE 銇?
銈ゃ兂銉┿偆銉炽儦銈ゃ儹銉笺儔銆丮CP 銇?read/write锛夈仺寮曘亶鎻涖亪銇仾銈娿伨銇欍€?

銉嶃偆銉嗐偅銉栧蹇溿倰寰呫仧銇氥伀 E2EE 銇屽繀瑕併仾銈夈€併儨銉笺儷銉堛伀
[`git-crypt`](https://github.com/AGWA/git-crypt) 銈掗噸銇仸銇忋仩銇曘亜銆傛寚瀹氥儜銈广伅
銈点兗銉愩兗銇嬨倝瑕嬨倢銇板京鍙蜂笉鑳姐仾 ciphertext blob 銇ㄣ仐銇﹀眾銇嶃伨銇欍€傘儠銈°偆銉悕銇?
銈点兗銉愩兗涓娿仹銇钩鏂囥伄銇俱伨銇с仚锛堝銇忋伄鑴呭▉銉儑銉仹銇ū瀹圭瘎鍥层仹銇欙級銆傞嵉銈掓寔銇?
銈儵銈ゃ偄銉炽儓銇倝 `git clone` 銇?`pkvsyncd materialize` 銇紩銇嶇稓銇嶆鑳姐仐銇俱仚銆?

鏈暘閬嬬敤銇с伅鍔犮亪銇?HTTPS 銇儗寰屻仹鍕曘亱銇椼€乣trusted_proxies` 銈掔禐銈娿€併儑銉笺偪
銉囥偅銈广偗銈掓殫鍙峰寲銇椼€併儛銉冦偗銈儍銉椼倐鏆楀彿鍖栥仐銇︺亸銇犮仌銇?鈥?瑭崇窗銇?
[銉囥儣銉偆寮峰寲銈偆銉塢(./public-docs/deployment-hardening.ja.md) 銇亗銈娿伨銇欍€?

## 銇婃帰銇椼伄銈傘伄銇€?

| 銉堛償銉冦偗 | 銉夈偔銉ャ儭銉炽儓 |
| --- | --- |
| 鏃ャ€呫伄銉椼儵銈般偆銉冲埄鐢?| [銉︺兗銈躲兗銉炪儖銉ャ偄銉玗(./public-docs/user-manual.ja.md) |
| 銈点兗銉愩兗绠＄悊銇ㄣ儵銉炽偪銈ゃ儬瑷畾 | [绠＄悊鑰呫優銉嬨儱銈儷](./public-docs/admin-manual.ja.md) |
| 銇欍伖銇︺伄 CLI 銈炽優銉炽儔銇ㄣ儠銉┿偘 | [CLI 銉儠銈°儸銉炽偣](./public-docs/cli-reference.ja.md) |
| 0.x 銇嬨倝 1.0 銇搞伄銈儍銉椼偘銉兗銉?| [1.0 銈儍銉椼偘銉兗銉夈儙銉笺儓](./public-docs/upgrade-notes-v1.0.ja.md) |
| 銉儛銉笺偣銉椼儹銈偡銆乀LS銆併儛銉冦偗銈儍銉椼€佸挤鍖?| [銉囥儣銉偆寮峰寲](./public-docs/deployment-hardening.ja.md) |
| HTTP API 浠曟 | [OpenAPI spec](./public-docs/openapi.yaml) |
| MCP 銈汇儍銉堛偄銉冦儣銇ㄣ儎銉笺儷涓€瑕?| [MCP 銉忋偊銉勩兗](./public-docs/mcp-howto.ja.md) |
| LLM 銇屼繚瀹堛仚銈?Wiki 銉兗銈儠銉兗 | [LLM Wiki 銉忋偊銉勩兗](./public-docs/llm-wiki-howto.ja.md) |
| Obsidian Sync 銇嬨倝銇Щ琛?| [绉昏銈偆銉塢(./public-docs/migrate-from-obsidian-sync.ja.md) |
| 銈汇偔銉ャ儶銉嗐偅闁嬬ず | [SECURITY.md](./SECURITY.md) |
| 銉儶銉笺偣灞ユ | [CHANGELOG.md](./CHANGELOG.md) |

## 銈广儐銉笺偪銈?

PKV Sync 1.4.4 銇洠鏌讳慨姝ｃ伄缍氥亶銇с€佹纰烘€с伀閲嶇偣銈掔疆銇勩仸銇勩伨銇欍€傜洠瑕栦粯銇嶃儛銉冦偗銈般儵銈︺兂銉夈偪銈广偗銇偘銉兗銈广儠銉偡銉ｃ儍銉堛儉銈︺兂鏅傘伀涓銇曘倢銆佸彜銇?DashMap 銈ㄣ兂銉堛儶銇畾鏈熺殑銇洖鍙庛仌銈屻€佽嚜鍕曘優銉笺偢銇瑺钀姐偑銉栥偢銈с偗銉堛仺 Git 涓€鏅傘偍銉┿兗銈掓銇椼亸鍖哄垾銇椼€佸啰绛夈偔銉ｃ儍銈枫儱銇儭銈裤儑銉笺偪銉堛儵銉炽偠銈偡銉с兂澶辨晽寰屻伀蹇呫仛鏇搞亶杈笺伨銈屻€佷甫琛屻儐銈偣銉堜綔鎴愩伅銈炽兂銉曘儶銈儓銉曘偂銈ゃ儷鏄囨牸銇т繚鎸併仌銈屻伨銇欍€備笉瑕併偝銉笺儔锛堟湭浣跨敤銉樸儷銉戙兗銆乮18n 銈兗銆丏ocker 銉偆銉ゃ兗锛夈倐鏁寸悊銇曘倢銇俱仐銇熴€?

PKV Sync 1.0 銇渶鍒濄伄瀹夊畾鐗堛儶銉兗銈广仹銇欍€傚叕闁?REST API銆丆LI 銈点兗銉曘偋銈广€?
銈广儓銉兗銈搞儸銈ゃ偄銈︺儓銆併儣銉┿偘銈ゃ兂銉戙儍銈便兗銈搞€丏ocker 銈ゃ儭銉笺偢銇悓銇?semver 銇?
銉愩兗銈搞儳銉崇鐞嗐仌銈屻伨銇欍€?.X.Y 銇叕闁嬨偟銉笺儠銈с偣銇у緦鏂逛簰鎻涙€с倰缍寔銇椼€?
OpenAPI 浠曟銇屼簰鎻涙€с伄姝ｆ湰銇ㄣ仾銈娿伨銇欍€?.x 銇т綔銈夈倢銇?SQLite 銉囥兗銈裤儥銉笺偣銇?
1.0.0 銇搞偆銉炽儣銉兗銈广仹銈儍銉椼偘銉兗銉夈仹銇嶃伨銇涖倱 鈥?
[1.0 銈儍銉椼偘銉兗銉夈儙銉笺儓](./public-docs/upgrade-notes-v1.0.ja.md) 銇緭銇ｃ仸
銇忋仩銇曘亜銆?

鍚?GitHub 銉儶銉笺偣銇с伅 Linux amd64/arm64 銉愩偆銉娿儶銆乄indows x64 銉愩偆銉娿儶銆?
銉炪儷銉併偄銉笺偔銇?GHCR Docker 銈ゃ儭銉笺偢銆丱bsidian 銉椼儵銈般偆銉炽伄 zip銆乣SHA256SUMS`
銈掑叕闁嬨仐銇俱仚銆?

## 闁嬬櫤銉併偋銉冦偗

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 銇с伅 Linux 銇?Windows 銇?Rust 銉曘儷銉炪儓銉偗銈广伀鍔犮亪銆併儣銉┿偘銈ゃ兂銇?
銉嗐偣銉堬紡typecheck锛廱uild锛忋儜銉冦偙銉笺偢銉炽偘銆丏ocker 銉撱儷銉夈€併儶銉兗銈?
銉愩偆銉娿儶銇偣銉兗銈儐銈广儓銇岃蛋銈娿伨銇欍€?

## 銉┿偆銈汇兂銈?

AGPL-3.0-only銆傝┏銇椼亸銇?[LICENSE](./LICENSE) 銈掑弬鐓с仐銇︺亸銇犮仌銇勩€?
