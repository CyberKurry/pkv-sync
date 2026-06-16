# CLI 銉儠銈°儸銉炽偣

[English](./cli-reference.md) | [绠€浣撲腑鏂嘳(./cli-reference.zh-CN.md) | [绻侀珨涓枃](./cli-reference.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./cli-reference.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

`pkvsyncd` 銇?PKV Sync 銇偟銉笺儛銉笺儑銉笺儮銉炽儛銈ゃ儕銉仹銇欍€侶TTP/WebSocket 銇悓鏈?API銆佺鐞?UI銆丮CP 銈点兗銉愩兗銆併亰銈堛伋灏戞暟銇亱鐢ㄣ偟銉栥偝銉炪兂銉夈倰銉涖偣銉堛仐銇俱仚銆?

## 銈般儹銉笺儛銉偑銉椼偡銉с兂

浠ヤ笅銇儠銉┿偘銇仚銇广仸銇偟銉栥偝銉炪兂銉夈伀閬╃敤銇曘倢銇俱仚銆?

- `-c, --config <PATH>`: TOML 瑷畾銉曘偂銈ゃ儷銇儜銈广€傘儑銉曘偐銉儓: `/etc/pkv-sync/config.toml`銆?
- `-h, --help`: 銉樸儷銉椼倰琛ㄧず銇椼伨銇欍€?
- `-V, --version`: CLI 銇儛銉笺偢銉с兂銈掕〃绀恒仐銇俱仚銆?

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 銈点儢銈炽優銉炽儔

`pkvsyncd` 銇?9 銇ゃ伄銈点儢銈炽優銉炽儔銈掓彁渚涖仐銇俱仚銆傛渶銈備竴鑸殑銇亱鐢ㄣ儠銉兗銇?`serve`銆乣genkey`銆乣migrate up`銆乣user add`銆乣backup`銆乣restore` 銇с仚銆?

## pkvsyncd serve

HTTP 銈点兗銉愩兗銈掕捣鍕曘仐銇俱仚銆?

### 褰㈠紡

```text
pkvsyncd serve
```

### 瑾槑

鍏枊鍚屾湡鐢ㄣ伄 HTTP 銉偣銉娿兗銆佺鐞?UI銆丼SE 銈广儓銉兗銉犮€丟it smart HTTP 銉兗銉堛€併亰銈堛伋瑷畾銇曘倢銇︺亜銈嬪牬鍚堛伅 MCP HTTP 銈ㄣ兂銉夈儩銈ゃ兂銉堛倰瀹熻銇椼伨銇欍€傘儶銈广儕銉笺伅 `config.toml` 銇?`[server].bind_addr` 銇儛銈ゃ兂銉夈仐銇俱仚銆俿ystemd 閰嶄笅銇俱仧銇偝銉炽儐銉婂唴銇儠銈┿偄銈般儵銈︺兂銉夈儣銉偦銈广仺銇椼仸瀹熻銇椼仸銇忋仩銇曘亜銆?

### 渚?

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

銉囥兗銈裤儥銉笺偣銉炪偆銈般儸銉笺偡銉с兂銈炽優銉炽儔銇с仚銆傚埄鐢ㄣ仹銇嶃倠鎿嶄綔銇?`up` 銇伩銇с仚銆?

### 褰㈠紡

```text
pkvsyncd migrate up
```

### 瑾槑

`server/migrations/` 閰嶄笅銇湭閬╃敤 SQLite 銉炪偆銈般儸銉笺偡銉с兂銈?`[storage].db_path` 銇儑銉笺偪銉欍兗銈广伀瀵俱仐銇﹂仼鐢ㄣ仐銇俱仚銆傚啀瀹熻銇椼仸銈傚畨鍏ㄣ仹銇傘倞銆侀仼鐢ㄦ笀銇裤伄銉炪偆銈般儸銉笺偡銉с兂銇偣銈儍銉椼仌銈屻伨銇欍€侶TTP 銈点兗銉愩兗銇捣鍕曟檪銇倐鏈仼鐢ㄣ優銈ゃ偘銉兗銈枫儳銉炽倰瀹熻銇欍倠銇熴倎銆佹墜鍕曘伄 `migrate up` 銇屽繀瑕併仺銇倠銇伅閫氬父銆併偝銉笺儷銉夈儶銈广儓銈伄銉曘儹銉笺倓銆併偑銉曘儵銈ゃ兂銉愩儍銈偄銉冦儣銈掔Щ琛屻仚銈嬪牬鍚堛伀闄愩倝銈屻伨銇欍€?

### 渚?

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

`[server].deployment_key` 銇仼銇椼仧銉┿兂銉€銉犮仾銉囥儣銉偆銉°兂銉堛偔銉笺倰鐢熸垚銇椼伨銇欍€?

### 褰㈠紡

```text
pkvsyncd genkey
```

### 瑾槑

鏆楀彿瀛︾殑銇儵銉炽儉銉犮仾 `k_*` 銉堛兗銈兂銈掓婧栧嚭鍔涖伀琛ㄧず銇椼伨銇欍€傚€ゃ倰 `config.toml` 銇布銈婁粯銇戙€佺嫭鑷伄瀹夊叏銇祵璺仹銉椼儵銈般偆銉?绠＄悊銈儵銈ゃ偄銉炽儓銇叡鏈夈仐銇︺亸銇犮仌銇勩€?

### 渚?

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

銉︺兗銈躲兗绠＄悊銈炽優銉炽儔銇с仚銆傞亱鐢ㄤ笂銇京鏃?銉戙偣銉兗銉夊繕銈屻€併偄銈偊銉炽儓銉儍銈?銈勩€佸壇娆＄殑銇偑銉氥儸銉笺偪銉笺偄銈偊銉炽儓銇偣銈儶銉椼儓銇倛銈嬪垵鏈熸绡夈伀褰圭珛銇°伨銇欍€?

### 褰㈠紡

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 銈点儢銈炽優銉炽儔

- `add <USERNAME> [--admin]`: 銉︺兗銈躲兗銈掍綔鎴愩仐銆併儜銈广儻銉笺儔銈掑瑭辩殑銇叆鍔涖仐銇俱仚銆?
- `passwd <USERNAME>`: 銉︺兗銈躲兗銇儜銈广儻銉笺儔銈掋儶銈汇儍銉堛仐銆佹柊銇椼亜鍊ゃ倰瀵捐┍鐨勩伀鍏ュ姏銇椼伨銇欍€?
- `list`: 銇欍伖銇︺伄銉︺兗銈躲兗銈掔鐞嗚€?鏈夊姽鐘舵厠銇娿倛銇充綔鎴愭檪鍒汇仺銇ㄣ倐銇竴瑕ц〃绀恒仐銇俱仚銆?
- `set-active <USERNAME> --active <true|false>`: 銉︺兗銈躲兗銈掔劇鍔瑰寲銇俱仧銇啀鏈夊姽鍖栥仐銇俱仚銆傜劇鍔瑰寲銇曘倢銇熴儲銉笺偠銉笺伅銉堛兗銈兂銈掍繚鎸併仐銇俱仚銇屻€併儹銈般偆銉炽倓鍚屾湡銇仹銇嶃伨銇涖倱銆?

### 渚?

```bash
# 绶婃€ャ偄銈偦銈圭敤銇鐞嗚€呫偄銈偊銉炽儓銈掍綔鎴?
pkvsyncd user add alice --admin

# 銉戙偣銉兗銉夊繕銈屻倰銉偦銉冦儓
pkvsyncd user passwd alice

# 閫€鑱枫仚銈嬨儲銉笺偠銉笺倰銉囥兗銈裤倰鍓婇櫎銇涖仛銇劇鍔瑰寲
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

PKV Sync 銉溿兗銉儓銇?bare Git 銉儩銈搞儓銉倰銆併儑銈ｃ偣銈笂銇€氬父銇儠銈°偆銉儎銉兗銇睍闁嬨仐銇俱仚銆?

### 褰㈠紡

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 銈儣銈枫儳銉?

- `-o, --output <DIR>`: 鍑哄姏銉囥偅銉偗銉堛儶(瀛樺湪銇椼仾銇勩亱绌恒仹銇傘倠蹇呰銇屻亗銈娿伨銇?銆?
- `--at <SHA>`: 鐗瑰畾銇偝銉熴儍銉堛仹銉炪儐銉偄銉┿偆銈恒仐銇俱仚(銉囥儠銈┿儷銉? HEAD)銆?

### 瑾槑

`data_dir/vaults/<vault-id>` 閰嶄笅銇亗銈嬨儨銉笺儷銉堛伄 bare Git 銉儩銈搞儓銉倰瑾伩鍙栥倞銆佸悇銉曘偂銈ゃ儷銈掑嚭鍔涖儑銈ｃ儸銈儓銉伀鏇搞亶杈笺伩銇俱仚銆?

- 銉嗐偔銈广儓銉曘偂銈ゃ儷銇仢銇伨銇炬浉銇嶈炯銇俱倢銇俱仚銆?
- `pkvsync_pointer` JSON 銇ㄣ仐銇︿繚瀛樸仌銈屻仸銇勩倠銉愩偆銉娿儶銉曘偂銈ゃ儷銇€併偟銉笺儛銉笺伄 blob 銈广儓銉兗銈?`data_dir/blobs/`)銇嬨倝瀹熼殯銇?blob 銈掋偝銉斻兗銇椼仸瑙ｆ焙銇椼伨銇欍€?

銈炽優銉炽儔銇悓鏈熺殑銇嫊浣溿仐銆併偟銉笺儛銉笺亴璧峰嫊銇椼仸銇勩倠蹇呰銇亗銈娿伨銇涖倱銆傝ō瀹氥仌銈屻仧 `data_dir` 閰嶄笅銇儑銈ｃ偣銈笂銇?Git 銉儩銈搞儓銉仺 blob 銈广儓銉兗銈搞亱銈夌洿鎺ヨ銇垮彇銈娿伨銇欍€?

### 渚?

```bash
# 鏈€鏂般儛銉笺偢銉с兂銈掋優銉嗐儶銈儵銈ゃ偤
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 鐗瑰畾銇偝銉熴儍銉堛倰銉炪儐銉偄銉┿偆銈?
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 绲備簡銈炽兗銉?

- `0`: 鎴愬姛銆?
- `1`: 鍑哄姏銉囥偅銉偗銉堛儶銇岀┖銇с仾銇勩€併儨銉笺儷銉堛亴瑕嬨仱銇嬨倝銇亜銆乥lob 銇屾瑺钀姐仐銇︺亜銈嬨€併偝銉熴儍銉?SHA 銇岀劇鍔广€併仾銇┿伄銈ㄣ儵銉笺€?

> 銉溿兗銉儓 ID 銇?32 鏂囧瓧銇皬鏂囧瓧 16 閫叉暟(銉忋偆銉曘兂銇仐)銇с仚銆備笂瑷樸伄渚嬨伅瀹熼殯銇舰寮忋伄 ID 銈掍娇鐢ㄣ仐銇︺亰銈娿€佺鐞?UI 銇?`pkvsyncd user list` 銇ф湁鍔广仾 ID 銈掔⒑瑾嶃仹銇嶃伨銇欍€?

## pkvsyncd backup

銈点兗銉愩兗銉囥兗銈裤倰銉濄兗銈裤儢銉仾銉愩儍銈偄銉冦儣銉囥偅銉偗銉堛儶銇偣銉娿儍銉椼偡銉с儍銉堛仺銇椼仸淇濆瓨銇椼伨銇欍€?

### 褰㈠紡

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 銈儣銈枫儳銉?

- `-o, --output <DIR>`: 銉愩儍銈偄銉冦儣銇嚭鍔涖儑銈ｃ儸銈儓銉?瀛樺湪銇椼仾銇勩亱绌恒仹銇傘倠蹇呰銇屻亗銈娿伨銇?銆?
- `--data-dir <DIR>`: 銈儠銉┿偆銉虫搷浣滅敤銇儑銉笺偪銉囥偅銉偗銉堛儶銈掍笂鏇搞亶銇椼伨銇欍€傘儑銉曘偐銉儓銇с伅瑾伩杈笺伨銈屻仧瑷畾銇?`[storage].data_dir` 銇屼娇銈忋倢銇俱仚銆?
- `--gzip`: 銉愩儍銈偄銉冦儣銉囥偅銉偗銉堛儶銇殻銇?`.tar.gz` 銈兗銈偆銉栥倐浣滄垚銇椼伨銇欍€?
- `--include-config`: 瑾伩杈笺倱銇?`config.toml` 銈?backup 銇惈銈併伨銇欍€傘儑銉曘偐銉儓銇с伅銆乨eployment key 銇仼銇儹銉笺偒銉瀵嗐倰鍚伩寰椼倠銇熴倎 config 銇渷鐣ャ仌銈屻伨銇欍€?

### 瑾槑

SQLite 銉囥兗銈裤儥銉笺偣(VACUUM INTO 绲岀敱)銆佸悇銉溿兗銉儓銇?bare Git 銉儩銈搞儓銉€併亰銈堛伋 blob 銈广儓銈倰銆乣MANIFEST.json` 銈掑惈銈€鑷繁瀹岀祼鍨嬨伄銉囥偅銉偗銉堛儶銇偣銉娿儍銉椼偡銉с儍銉堛仐銇俱仚銆傘儛銉冦偗銈儍銉椾腑銈?HTTP 銈点兗銉愩兗銇鍍嶃倰缍氥亼銈夈倢銇俱仚銇屻€乸ush銆乥lob 銈儍銉椼儹銉笺儔銆乺ollback銆乿ault 鍓婇櫎銆丟C 銇仼銇偣銉堛儸銉笺偢鏇搞亶杈笺伩銇?data-dir 銈广儕銉冦儣銈枫儳銉冦儓銉儍銈伄寰屻倣銇у緟姗熴仐銆併儛銉冦偗銈儍銉楀畬浜嗗緦銇€层伩銇俱仚銆?

銉囥儠銈┿儷銉堛仹銇€併儛銉冦偗銈儍銉椼伅 `config.toml` 銈掔渷鐣ャ仐銇俱仚銆傝ō瀹氥倰淇濆瓨銇椼€併仢銇瀵嗘儏鍫便倰淇濊銇欍倠銇ゃ倐銈娿亴銇傘倠鍫村悎銇犮亼 `--include-config` 銈掕拷鍔犮仐銇︺亸銇犮仌銇勩€?

### 渚?

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

銉愩儍銈偄銉冦儣銉囥偅銉偗銉堛儶銈掋儑銉笺偪銉囥偅銉偗銉堛儶銇儶銈广儓銈仐銇俱仚銆?

### 褰㈠紡

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 銈儣銈枫儳銉?

- `-i, --input <DIR>`: `MANIFEST.json` 銈掑惈銈€銉愩儍銈偄銉冦儣銉囥偅銉偗銉堛儶銆?
- `--data-dir <DIR>`: 瀵捐薄銉囥兗銈裤儑銈ｃ儸銈儓銉伄涓婃浉銇嶃€傘儑銉曘偐銉儓銇?`[storage].data_dir`銆?
- `--force`: 銉偣銉堛偄鍓嶃伀绌恒仹銇亜瀵捐薄銉囥兗銈裤儑銈ｃ儸銈儓銉倰銈儶銈仐銇俱仚銆?

### 瑾槑

銉愩儍銈偄銉冦儣銇?`MANIFEST.json` 銈掓瑷笺仐銆丼QLite DB銆併儨銉笺儷銉堛儶銉濄偢銉堛儶銆乥lob 銈广儓銈倰瀵捐薄銉囥兗銈裤儑銈ｃ儸銈儓銉伕銈炽償銉笺仐銇俱仚銆傘儶銈广儓銈㈠墠銇?HTTP 銈点兗銉愩兗銈掑仠姝仐銇︺亸銇犮仌銇勩€傚彜銇勩偟銉笺儛銉笺儛銉笺偢銉с兂銇у彇寰椼仐銇熴儛銉冦偗銈儍銉椼倰銉偣銉堛偄銇欍倠鍫村悎銇€併儶銈广儓銈㈠緦銇?`pkvsyncd migrate up` 銈掑疅琛屻仐銇︺亸銇犮仌銇勩€?

### 渚?

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

銉溿兗銉儓銇?Git 銉儩銈搞儓銉仺鍐呭銈儔銉偣鎸囧畾銇曘倢銇?blob 銈掓瑷笺仐銇俱仚銆?

### 褰㈠紡

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 銈儣銈枫儳銉?

- `--data-dir <DIR>`: 銉囥兗銈裤儑銈ｃ儸銈儓銉伄涓婃浉銇嶃€?
- `--no-fail`: 妞滆銇с偍銉┿兗銇岃銇ゃ亱銇ｃ仧鍫村悎銇с倐绲備簡銈炽兗銉?0 銈掕繑銇椼伨銇欍€傘儦銉笺偢銉炽偘銇仐銇儹銈般仩銇戝彇銈娿仧銇勭洠瑕栥偣銈儶銉椼儓銇究鍒┿仹銇欍€?

### 瑾槑

`data_dir/vaults/` 閰嶄笅銇悇銉溿兗銉儓銇銇椼仸娆°倰瀹熻銇椼伨銇欍€?

- bare 銉儩銈搞儓銉伀瀵俱仐銇?`git fsck --strict` 銈掑疅琛屻仐銇俱仚銆?
- HEAD 銉勩儶銉笺倰銇熴仼銈娿€併仚銇广仸銇?`pkvsync_pointer` 銇屻€併儑銈ｃ偣銈笂銇?SHA-256 銇屻儠銈°偆銉悕銇ㄤ竴鑷淬仚銈?blob 銇В姹恒仌銈屻倠銇撱仺銈掓瑷笺仐銇俱仚銆?

銉溿兗銉儓銇斻仺銇偍銉┿兗浠舵暟銈掑牨鍛娿仐銇俱仚銆傘亜銇氥倢銇嬨伄銉溿兗銉儓銇偍銉┿兗銇屻亗銈嬪牬鍚堛伅闈炪偧銉仹绲備簡銇椼伨銇欍€傘仧銇犮仐 `--no-fail` 銇岃ō瀹氥仌銈屻仸銇勩倠鍫村悎銈掗櫎銇嶃伨銇欍€?

### 渚?

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

AI 銉勩兗銉悜銇戙伄 MCP(Model Context Protocol)銈点兗銉愩兗銈掕捣鍕曘仐銇俱仚銆?

### 褰㈠紡

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 銈儣銈枫儳銉?

- `--transport <stdio|http>`: 銉堛儵銉炽偣銉濄兗銉堛儮銉笺儔銆傘儑銉曘偐銉儓: `stdio`銆?
- `--vault <VAULT-ID>`: stdio 銇у繀闋堛€傘偗銉┿偆銈兂銉堛伀鍏枊銇欍倠鍗樹竴銇儨銉笺儷銉堛仹銇欍€?
- `--token <PKS-TOKEN>`: stdio 鐢ㄣ伄銉欍偄銉┿兗銉囥儛銈ゃ偣銉堛兗銈兂銆傜渷鐣ャ仐銇熷牬鍚堛伅鐠板澶夋暟 `PKV_TOKEN` 銇屼娇銈忋倢銇俱仚銆?
- `--bind <ADDR>`: HTTP 銇儛銈ゃ兂銉夈偄銉夈儸銈广€傘儑銉曘偐銉儓: `127.0.0.1:6711`銆?

### 瑾槑

`stdio` 銉兗銉夈伅妯欐簴鍏ュ姏銇嬨倝 JSON-RPC 銈掕銇垮彇銈娿€佹婧栧嚭鍔涖伀 JSON-RPC 銈掓浉銇嶈炯銇裤伨銇欍€俙http` 銉兗銉夈伅銈广儐銉笺儓銉偣銇?Streamable HTTP MCP 銈ㄣ兂銉夈儩銈ゃ兂銉堛倰 `/mcp` 銇ф彁渚涖仐銇俱仚銆傘仼銇°倝銇儮銉笺儔銈傚悓銇樸儎銉笺儷銈汇儍銉堛倰鍏枊銇椼伨銇? `list_vaults`銆乣list_files`銆乣read_file`銆乣read_file_at_commit`銆乣search`銆乣link_graph`銆乣changes_since`銆乣write_file`銆乣delete_file`銆乣write_files`銆乣move_file`銆俙write_files` 銇鏁般儦銉笺偢銇?wiki 绶ㄩ泦銈掑師瀛愮殑銇伨銇ㄣ倎銈嬪牬鍚堛伀銆乣move_file` 銇饱姝淬倰淇濄仯銇熷悕鍓嶅鏇淬倓銈兗銈偆銉栫Щ鍕曘伀浣裤亜銇俱仚銆傛浉銇嶈炯銇跨郴銉勩兗銉伅 `(token, vault)` 銇斻仺銇?1 鍒嗐亗銇熴倞 60 鍥炪伨銇с伀銉兗銉堝埗闄愩仌銈屻€乣write_files` batch 銇?1 銇ゃ伄鏇搞亶杈笺伩瑷橀尣銇犮亼銈掓秷璨汇仐銇俱仚銆傛绱儶銈偍銈广儓銇渶澶?5000 鍊嬨伄琛ㄧず鍙兘銇?tree files 銈掕蛋鏌汇仐銆佹渶澶?500 浠躲伄涓€鑷淬倰杩斻仐銆佹湰鐣挵澧冦仹銇绱㈡笀銇裤儐銈偣銉堛亴 256 MiB 銇仈銇欍倠銇ㄥ仠姝仐銇俱仚銆俙link_graph` 銇悓銇樻湰鐣儐銈偣銉堜簣绠椼仹鏈€澶?5000 鍊嬨伄琛ㄧず鍙兘銇儐銈偣銉堛儠銈°偆銉倰璧版熁銇椼€乣changes_since` 銇渶澶?5000 浠躲伄琛ㄧず鍙兘銇鏇淬偍銉炽儓銉倰杩斻仐銇俱仚銆?4 MiB 銈掕秴銇堛倠 binary/blob 瑾伩鍙栥倞銉偣銉濄兂銈广伅銆乥ase64 銇?JSON 銇睍闁嬨仌銈屻倠浠ｃ倧銈娿伀鎷掑惁銇曘倢銇俱仚銆?

`http` 銉兗銉夈仹銇€侀€氬父銇悓鏈?API 銇ㄥ悓銇樸亸銆併仚銇广仸銇儶銈偍銈广儓銇偟銉笺儛銉笺伄銉囥儣銉偆銉°兂銉堛偔銉笺儤銉冦儉銉笺倰浠樹笌銇欍倠蹇呰銇屻亗銈娿伨銇欍€?


銇撱伄銈点儢銈炽優銉炽儔銇紩銇嶇稓銇嶇嫭绔?MCP 銉椼儹銈汇偣銇с仚銆傚悓銇?Streamable HTTP transport 銈掋儭銈ゃ兂銈点兗銉愩兗銉濄兗銉堛亱銈夋彁渚涖仚銈嬨伀銇€乣[mcp].embed_in_serve = true` 銈掕ō瀹氥仐銆乣pkvsyncd serve` 銈掍娇銇勩伨銇欍€?
### 渚?

```bash
# 鐠板澶夋暟銇儓銉笺偗銉炽倰浣裤仯銇?stdio
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 銉兗銈儷銇?Streamable HTTP 銈ㄣ兂銉夈儩銈ゃ兂銉?
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

PKV Sync 銇儶銉兗銈广儛銈ゃ儕銉倰鐝惧湪銇疅琛屻儠銈°偆銉伄闅ｃ伀銉€銈︺兂銉兗銉夈仐銇俱仚銆?

### 褰㈠紡

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 銈儣銈枫儳銉?

- `--dry-run`: 浣曘倐銉€銈︺兂銉兗銉夈仜銇氥伀銆侀伕鎶炪仌銈屻仧銉儶銉笺偣銆併偄銈汇儍銉堛€佸璞°儜銈广倰琛ㄧず銇椼伨銇欍€?
- `--yes`: 瀵捐┍鐨勩仾纰鸿獚銉椼儹銉炽儣銉堛倰銈广偔銉冦儣銇椼伨銇欍€?
- `--version <VERSION>`: 鏈€鏂般儶銉兗銈广仹銇仾銇?`1.4.5` 銇倛銇嗐仾鐗瑰畾銇儶銉兗銈广倰銉€銈︺兂銉兗銉夈仐銇俱仚銆?

### 瑾槑

銇撱伄銈炽優銉炽儔銇従鍦ㄣ伄銉椼儵銉冦儓銉曘偐銉笺儬鍚戙亼銇儶銉兗銈广偄銈汇儍銉堛倰閬告姙銇椼€併儉銈︺兂銉兗銉夈倰 `SHA256SUMS` 銇収銈夈仐銇︽瑷笺仐銆佺従鍦ㄣ伄銉愩偆銉娿儶銇殻銇?`pkvsyncd.new`(Windows 銇с伅 `pkvsyncd.new.exe`)銈掓浉銇嶅嚭銇椼€乻ystemd 銇俱仧銇墜鍕曘仹銇樊銇楁浛銇堟墜闋嗐倰琛ㄧず銇椼伨銇欍€傜鍍嶄腑銇偟銉笺儛銉笺倰銉涖儍銉堛儶銉椼儸銉笺偣銇欍倠銇撱仺銇亗銈娿伨銇涖倱銆?

Docker 銇娿倛銇?Kubernetes 銇儑銉椼儹銈ゃ伅銆併偆銉°兗銈搞偪銈般倰銉椼儷銇俱仧銇鏇淬仐銆併偟銉笺儞銈广倓銉兗銉偄銈︺儓銈掑啀璧峰嫊銇欍倠銇撱仺銇с偄銉冦儣銈般儸銉笺儔銇欍伖銇嶃仹銇欍€傘偝銉炽儐銉婄挵澧冦倰妞滃嚭銇椼仧鍫村悎銆併偝銉炪兂銉夈伅銈ゃ儭銉笺偢銉欍兗銈广伄銈偆銉€銉炽偣銈掕〃绀恒仐銆併儛銈ゃ儕銉倰鏇搞亶鍑恒仌銇氥伀绲備簡銇椼伨銇欍€?

### 渚?

```bash
# 銈儍銉椼偘銉兗銉夎▓鐢汇倰銉椼儸銉撱儱銉?
pkvsyncd upgrade --dry-run

# 鏈€鏂般伄妞滆娓堛伩銉愩偆銉娿儶銈掋儉銈︺兂銉兗銉?
pkvsyncd upgrade --yes

# 鐗瑰畾銇儶銉兗銈广倰銉€銈︺兂銉兗銉?
pkvsyncd upgrade --yes --version 1.4.5
```
