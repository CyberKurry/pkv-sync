# Upgrade notes: 0.x 銇嬨倝 1.0 銇?

[English](./upgrade-notes-v1.0.md) | [绠€浣撲腑鏂嘳(./upgrade-notes-v1.0.zh-CN.md) | [绻侀珨涓枃](./upgrade-notes-v1.0.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./upgrade-notes-v1.0.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

PKV Sync 1.0 銇渶鍒濄伄 stable release 銇с仚銆傚悓鏅傘伀銆佷粖寰屻伄 1.x maintenance 銇仧銈併伀
SQLite migration baseline 銈?reset 銇椼伨銇欍€?

## 閲嶈銇?database note

PKV Sync 1.0 銇崢涓€銇?`0001_initial.sql` baseline migration 銈掑嚭鑽枫仐銇俱仚銆?
0.x release 銇т綔鎴愩仌銈屻仧 SQLite database 銇?1.0.0 銇搞偆銉炽儣銉兗銈?upgrade 銇с亶銇俱仜銈撱€?

0.x server 銈掗亱鐢ㄣ仐銇︺亜銈嬪牬鍚堛伅銆佹銇亜銇氥倢銇嬨伄绲岃矾銈掗伕銈撱仹銇忋仩銇曘亜銆?

1. 鏃㈠瓨 deployment 銇Щ琛屾簴鍌欍伄 backup銆乵aterialize銆乪xport 銇繀瑕併仾闁撱仩銇戙€佹渶绲?0.8.x patch release 銇仺銇┿倎銈嬨€?
2. 鍚?vault 銈?backup 銇俱仧銇?materialize 銇椼€佹柊銇椼亜 1.0 data directory 銇ц捣鍕曘仐銆?
   user 銇?vault 銈掍綔銈婄洿銇椼仸銇嬨倝 contents 銈掓柊 server 銇?import 銇俱仧銇?push 銇欍倠銆?
3. migration rehearsal 銈掕│銇欏墠銇€?.x data root 銇畬鍏ㄣ仾 `pkvsyncd backup` 銈掍繚绠°仚銈嬨€?

鏃㈠瓨銇?0.x `metadata.db` 銇?1.0 binary 銈?Docker image 銈掔洿鎺ュ悜銇戙仾銇勩仹銇忋仩銇曘亜銆?

## 1.0 銇屽畨瀹氬寲銇欍倠 surface

1.0 浠ュ緦銆佹銇?surface 銇?semantic versioning 銇緭銇勩伨銇欍€?

- `public-docs/openapi.yaml` 銇杓夈仌銈屻仧 public REST routes銆?
- MCP how-to 銇杓夈仌銈屻仧 MCP stdio 銇?Streamable HTTP tool behavior銆?
- 1.x fresh database 鐢?SQLite migrations銆備粖寰屻伄 1.x migrations 銇亾銇?v1 baseline 浠ュ緦
  append-only 銇с仚銆?
- vault 銇斻仺銇?git repository layout 銇?content-addressed blob storage銆?
- CLI subcommand 銇ㄦ棦瀛?flag銆?
- Obsidian plugin settings 銇?sync behavior銆傞€氬父銇?backward-compatible 銇?1.x feature
  addition 銇亗銈娿伨銇欍€?

OpenAPI 銇杓夈仌銈屻仸銇勩仾銇?route銆併仧銇ㄣ亪銇?Admin Web UI form handler 銇?internal
implementation detail 銇с仚銆?

## 鎺ㄥエ銇曘倢銈?0.x 銇嬨倝 1.0 銇搞伄鎵嬮爢

1. 鍙兘銇с亗銈屻伆銆佹棫 deployment 銈掋伨銇氭渶绲?0.8.x patch release 銇告洿鏂般仐銆乥ackup銆乵aterialize銆乪xport 銇簴鍌欍伀銇犮亼浣裤亜銇俱仚銆?
2. `pkvsyncd backup --output <backup-dir>` 銈掑疅琛屻仐銆佺祼鏋溿倰瀹夊叏銇繚绠°仐銇俱仚銆?
3. 鍚?vault 銇仱銇勩仸銆佹渶鏂般伄 Obsidian client銆乣git clone`銆併伨銇熴伅
   `pkvsyncd materialize <vault-id> --output <dir>` 銇х従鍦ㄣ伄 file tree 銈掍綔鎴愩仐銇俱仚銆?
4. 鏃?server 銈掑仠姝仐銇俱仚銆?
5. 绌恒伄 `data_dir` 銇?`metadata.db` 銇?PKV Sync 1.0 銈掕捣鍕曘仐銇俱仚銆?
6. `/setup` 銈掑畬浜嗐仐銆乽ser 銇?vault 銈掍綔銈婄洿銇椼仸銇嬨倝銆乵aterialized vault contents 銈?
   push 銇俱仧銇?import 銇椼伨銇欍€?
7. user 銇?Obsidian plugin 銈?1.0.0 銇告洿鏂般仐銇︺倐銈夈亜銇俱仚銆?

## Plugin compatibility

1.0 server 銇?supported plugin 銇ㄣ仾銈嬨伄銇€乻erver 銇?bundled 銇曘倢銇?1.0 Obsidian plugin 銇с仚銆?
鍙ゃ亜 v0.8.x plugin 銈?core sync API 銇悓銇樸仹銇欍亴銆佹柊銇椼亜淇銇?self-update hardening 銇?
1.0+ 銇с伄銇跨董鎸併仌銈屻伨銇欍€?

## 0.x 銇嬨倝銇?breaking changes

- migration 銇屽崢涓€銇?v1 baseline 銇?squash 銇曘倢銇熴仧銈併€?.x SQLite database 銇?
  in-place upgrade 銇曘倢銇俱仜銈撱€?
- first-run setup 銇?browser-based 銇伨銇俱仹銇欍€俧resh server 銇?random admin password 銈?
  log 銇嚭鍔涖仐銇俱仜銈撱€?

vault file contents銆乬it history銆乥lob 銇?backup/materialize/recreate/import workflow 銇?
鎸併仭瓒娿仜銇俱仚銆?

## Known caveats

- native per-vault E2EE 銇?1.0 銇瘎鍥插銇с仚銆俢lient-side encrypted file contents 銇?
  浠娿仚銇愬繀瑕併仹銆乸laintext path 銈掑彈銇戝叆銈屻倝銈屻倠鍫村悎銇?
  [`git-crypt`](./git-crypt-howto.ja.md) 銈掍娇銇ｃ仸銇忋仩銇曘亜銆?
- `/metrics` 銇?default 銇?disabled 銇с€佹湁鍔瑰寲銇椼仸銈?production authentication gates 銇屽繀瑕併仹銇欍€?
- production 銇с伅 `public_host` 銈掕ō瀹氥仐銇︺亸銇犮仌銇勩€俢onfigured HTTPS public origin 銈掓焙瀹氥仹銇嶃仾銇勫牬鍚堛€?
  admin POST 銇剰鍥崇殑銇?fail-closed 銇椼伨銇欍€?
