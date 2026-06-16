# CLI 鍙冭€?

[English](./cli-reference.md) | [绠€浣撲腑鏂嘳(./cli-reference.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./cli-reference.ja.md) | [頃滉淡鞏碷(./cli-reference.ko.md)

鏂囦欢鐗堟湰锛歷1.4.3銆?

`pkvsyncd` 鏄?PKV Sync 鐨勪己鏈嶅櫒甯搁绋嬪紡鍩疯妾旓紝鎻愪緵 HTTP/WebSocket 鍚屾 API銆佺鐞嗕粙闈€丮CP 浼烘湇鍣紝浠ュ強涓€灏忕祫缍亱鐢ㄧ殑瀛愬懡浠ゃ€?

## 鍏ㄥ煙閬搁爡

涓嬪垪鏃楁閬╃敤鏂兼墍鏈夊瓙鍛戒护锛?

- `-c, --config <PATH>`锛歍OML 瑷畾妾旇矾寰戙€傞爯瑷€硷細`/etc/pkv-sync/config.toml`銆?
- `-h, --help`锛氶’绀鸿鏄庛€?
- `-V, --version`锛氬嵃鍑?CLI 鐗堟湰銆?

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 瀛愬懡浠?

`pkvsyncd` 鎻愪緵涔濆€嬪瓙鍛戒护銆傛渶甯哥敤鐨勭董閬嬫祦绋嬫槸 `serve`銆乣genkey`銆乣migrate up`銆乣user add`銆乣backup` 鑸?`restore`銆?

## pkvsyncd serve

鍟熷嫊 HTTP 浼烘湇鍣ㄣ€?

### 鐢ㄦ硶

```text
pkvsyncd serve
```

### 瑾槑

鍩疯灏嶅鐨勫悓姝?HTTP 鐩ｈ伣鍣ㄣ€佺鐞嗕粙闈€丼SE 涓叉祦銆丟it smart HTTP 璺敱锛屼互鍙婏紙瑷畾鍟熺敤鏅傜殑锛塎CP HTTP endpoint銆傜洠鑱藉櫒鏈冪秮瀹氬埌 `config.toml` 涓殑 `[server].bind_addr`銆傝珛浠?systemd 鍓嶆櫙绋嬪簭鎴栧鍣ㄦ柟寮忓煼琛屻€?

### 绡勪緥

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

璩囨枡搴伔绉诲懡浠ゃ€傜洰鍓嶅彧鏀彺 `up` 涓€绋搷浣溿€?

### 鐢ㄦ硶

```text
pkvsyncd migrate up
```

### 瑾槑

灏?`[storage].db_path` 鎵€鎸囩殑璩囨枡搴紝濂楃敤 `server/migrations/` 鐩寗涓嬫墍鏈夊皻鏈煼琛岀殑 SQLite 閬风Щ銆傚彲閲嶈鍩疯锛屽凡濂楃敤鐨勯伔绉绘渻琚暐閬庛€侶TTP 浼烘湇鍣ㄥ暉鍕曟檪涔熸渻鑷嫊鍩疯寰呭鐢ㄧ殑閬风Щ锛屽洜姝ゆ墜鍕曞煼琛?`migrate up` 閫氬父鍙湪鍐烽倓鍘熸祦绋嬫垨鐐洪洟绶氬倷浠藉仛閬风Щ鏅傛墠闇€瑕併€?

### 绡勪緥

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

鐢㈢敓涓€绲勫彲鐢ㄦ柤 `[server].deployment_key` 鐨勯毃姗熼儴缃查噾閼般€?

### 鐢ㄦ硶

```text
pkvsyncd genkey
```

### 瑾槑

鍚?stdout 鍗板嚭涓€绲勪互瀵嗙⒓瀛镐簜鏁哥敘鐢熺殑 `k_*` token銆傝珛灏囪┎鍊艰布鍒?`config.toml`锛屼甫閫忛亷浣犺嚜鏈夌殑瀹夊叏閫氶亾鍒嗙櫦绲﹀鎺涳紡绠＄悊绔殑瀹㈡埗绔€?

### 绡勪緥

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

浣跨敤鑰呯鐞嗗懡浠ゃ€傞仼鍚堢敤鏂肩董閬嬪堡绱氱殑寰╁師锛堝繕瑷樺瘑纰笺€佸赋铏熻鍋滅敤锛変互鍙婁互鎸囦护绋垮暉鍕曟瑕佹搷浣滃摗甯宠櫉銆?

### 鐢ㄦ硶

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 瀛愬懡浠?

- `add <USERNAME> [--admin]`锛氬缓绔嬩娇鐢ㄨ€咃紝涓︿互浜掑嫊鏂瑰紡鎻愮ず杓稿叆瀵嗙⒓銆?
- `passwd <USERNAME>`锛氶噸瑷娇鐢ㄨ€呭瘑纰硷紝涓︽彁绀鸿几鍏ユ柊鍊笺€?
- `list`锛氬垪鍑烘墍鏈変娇鐢ㄨ€咃紝鍖呭惈鍏剁鐞嗗摗锛忓暉鐢ㄧ媭鎱嬭垏寤虹珛鏅傞枔銆?
- `set-active <USERNAME> --active <true|false>`锛氬仠鐢ㄦ垨閲嶆柊鍟熺敤浣跨敤鑰呫€傝鍋滅敤鐨勪娇鐢ㄨ€呬粛淇濇湁鑷韩鐨?token锛屼絾鐒℃硶鐧诲叆鎴栧悓姝ャ€?

### 绡勪緥

```bash
# 鐐虹穵鎬ュ瓨鍙栧缓绔嬬鐞嗗摗甯宠櫉
pkvsyncd user add alice --admin

# 閲嶈ō蹇樿鐨勫瘑纰?
pkvsyncd user passwd alice

# 鍋滅敤闆㈣伔浣跨敤鑰呬絾涓嶅埅闄ゅ叾璩囨枡
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

灏?PKV Sync vault 鐨?bare git repository 灞曢枊鐐虹纰熶笂鐨勬櫘閫氭獢妗堟ü銆?

### 鐢ㄦ硶

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 閬搁爡

- `-o, --output <DIR>`锛氳几鍑虹洰閷勶紝蹇呴爤涓嶅瓨鍦ㄦ垨鐐虹┖銆?
- `--at <SHA>`锛氶倓鍘熷埌鎸囧畾 commit锛岄爯瑷偤 HEAD銆?

### 瑾槑

璁€鍙?vault 鍦?`data_dir/vaults/<vault-id>` 涓嬬殑 bare git repository锛屼甫灏囨瘡鍊嬫獢妗堝鍏ヨ几鍑虹洰閷勶細

- 鏂囧瓧妾旀渻鍘熸ǎ瀵叆銆?
- 浠?`pkvsync_pointer` JSON 鍎插瓨鐨勪簩閫蹭綅妾旓紝鏈冨緸浼烘湇鍣ㄧ殑 blob 鍎插瓨鍗€锛坄data_dir/blobs/`锛夎瑁藉闅涚殑 blob銆?

姝ゅ懡浠ゅ悓姝ュ煼琛岋紝涓嶉渶瑕佷己鏈嶅櫒姝ｅ湪閬嬭銆傚畠鐩存帴寰炶ō瀹氱殑 `data_dir` 涓嬬殑纾佺 git repository 鑸?blob 鍎插瓨鍗€璁€鍙栬硣鏂欍€?

### 绡勪緥

```bash
# 閭勫師鏈€鏂扮増鏈?
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 閭勫師鎸囧畾 commit
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 绲愭潫纰?

- `0`锛氭垚鍔熴€?
- `1`锛氶尟瑾わ紝渚嬪杓稿嚭鐩寗闈炵┖銆佹壘涓嶅埌 vault銆乥lob 缂哄け鎴?commit SHA 鐒℃晥銆?

> Vault ID 鐐?32 鍊嬪瓧鍏冪殑灏忓鍗佸叚閫蹭綅瀛椾覆锛堜笉鍚牬鎶樿櫉锛夈€備笂杩扮瘎渚嬬殕鎺＄敤鐪熷鏍煎紡鐨?ID锛涚鐞嗕粙闈㈣垏 `pkvsyncd user list` 涔熸渻椤ず鏈夋晥鐨?ID銆?

## pkvsyncd backup

灏囦己鏈嶅櫒璩囨枡蹇収鎴愬彲鏀滅殑鍌欎唤鐩寗銆?

### 鐢ㄦ硶

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 閬搁爡

- `-o, --output <DIR>`锛氬倷浠借几鍑虹洰閷勶紝蹇呴爤涓嶅瓨鍦ㄦ垨鐐虹┖銆?
- `--data-dir <DIR>`锛氶洟绶氭搷浣滄檪鐢ㄤ互瑕嗗鐨勮硣鏂欑洰閷勩€傞爯瑷偤宸茶級鍏ヨō瀹氫腑鐨?`[storage].data_dir`銆?
- `--gzip`锛氬湪鍌欎唤鐩寗鏃侀澶栧缓绔嬩竴浠?`.tar.gz` 澹撶府妾斻€?
- `--include-config`锛氭妸宸茶級鍏ョ殑 `config.toml` 涓€浣靛鍏ュ倷浠姐€傞爯瑷倷浠芥渻鐪佺暐瑷畾妾旓紝鍥犵偤鍏朵腑鍙兘鍖呭惈閮ㄧ讲閲戦懓鍜屾湰姗熺瀵嗐€?

### 瑾槑

灏?SQLite 璩囨枡搴紙閫忛亷 VACUUM INTO 閫茶锛夈€佹瘡鍊?vault 鐨?bare git repository锛屼互鍙?blob 鍎插瓨鍗€锛屽揩鐓у埌涓€鍊嬬崹绔嬬洰閷勶紝涓﹀鍏?`MANIFEST.json`銆傚倷浠芥湡闁?HTTP 浼烘湇鍣ㄥ彲绻肩簩閬嬭锛沺ush銆乥lob 涓婂偝銆佸洖婊俱€乿ault 鍒櫎鍜?GC 绛夊劜瀛樺鍏ユ渻鍦?data-dir 蹇収閹栦箣寰屾帓闅婏紝鐩村埌鍌欎唤瀹屾垚銆?

闋愯ō鎯呮硜涓嬶紝鍌欎唤鏈冪渷鐣?`config.toml`锛涘彧鏈夊湪浣犳槑纰鸿淇濆瓨瑷畾涓︿繚璀峰叾涓瀵嗘檪锛屾墠鍔犲叆 `--include-config`銆?

### 绡勪緥

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

灏囧倷浠界洰閷勯倓鍘熷埌璩囨枡鐩寗涓€?

### 鐢ㄦ硶

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 閬搁爡

- `-i, --input <DIR>`锛氬寘鍚?`MANIFEST.json` 鐨勫倷浠界洰閷勩€?
- `--data-dir <DIR>`锛氱敤浠ヨ瀵殑鐩璩囨枡鐩寗銆傞爯瑷偤 `[storage].data_dir`銆?
- `--force`锛氬湪閭勫師鍓嶆竻绌洪潪绌虹殑鐩璩囨枡鐩寗銆?

### 瑾槑

椹楄瓑鍌欎唤鐨?`MANIFEST.json`锛屽皣 SQLite 璩囨枡搴€佸悇 vault repository 鑸?blob 鍎插瓨鍗€瑜囪＝鍒扮洰妯欒硣鏂欑洰閷勩€傞倓鍘熷墠璜嬪厛鍋滄 HTTP 浼烘湇鍣ㄣ€傝嫢閭勫師鐨勫倷浠芥槸鐢辫純鑸婄増鏈殑浼烘湇鍣ㄦ墍鐢㈢敓锛岄倓鍘熷畬鎴愬緦璜嬪啀鍩疯涓€娆?`pkvsyncd migrate up`銆?

### 绡勪緥

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

椹楄瓑鍚?vault 鐨?git repository 鑸囧収瀹瑰畾鍧€鐨?blob銆?

### 鐢ㄦ硶

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 閬搁爡

- `--data-dir <DIR>`锛氱敤浠ヨ瀵殑璩囨枡鐩寗銆?
- `--no-fail`锛氬嵆浣块璀夌櫦鐝鹃尟瑾わ紝浠嶅洖鍌崇祼鏉熺⒓ 0銆傞仼鍚堝儏闇€瑷橀寗鑰屼笉甯屾湜瑙哥櫦鍛婅鐨勭洠鎺ф寚浠ょ銆?

### 瑾槑

灏?`data_dir/vaults/` 涔嬩笅鐨勬瘡鍊?vault锛?

- 灏?bare repository 鍩疯 `git fsck --strict`銆?
- 璧拌í HEAD 妯癸紝涓﹂璀夋瘡鍊?`pkvsync_pointer` 閮藉彲瑙ｆ瀽鍒板皪鎳?blob锛屼笖瑭?blob 鍦ㄧ纰熶笂鐨?SHA-256 鑸囧叾妾斿悕涓€鑷淬€?

鎸?vault 閫愪竴鍥炲牨閷鏁搁噺銆傚彧瑕佷换涓€ vault 鏈夐尟瑾わ紝渚夸互闈為浂绲愭潫纰肩祼鏉燂紱闄ら潪鍔犱笂 `--no-fail`銆?

### 绡勪緥

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

鍟熷嫊渚?AI 宸ュ叿浣跨敤鐨?MCP锛圡odel Context Protocol锛変己鏈嶅櫒銆?

### 鐢ㄦ硶

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 閬搁爡

- `--transport <stdio|http>`锛氬偝杓告ā寮忋€傞爯瑷偤 `stdio`銆?
- `--vault <VAULT-ID>`锛歴tdio 妯″紡蹇呭～锛屾寚瀹氳鍚戝鎴剁鏆撮湶鐨勫柈涓€ vault銆?
- `--token <PKS-TOKEN>`锛歴tdio 浣跨敤鐨?bearer 瑁濈疆 token銆傜渷鐣ユ檪鏈冩敼鐢?`PKV_TOKEN` 鐠板璁婃暩銆?
- `--bind <ADDR>`锛欻TTP 鐩ｈ伣浣嶅潃銆傞爯瑷偤 `127.0.0.1:6711`銆?

### 瑾槑

`stdio` 妯″紡寰?stdin 璁€鍙?JSON-RPC锛屼甫鍚?stdout 瀵叆 JSON-RPC銆俙http` 妯″紡鍦?`/mcp` 鎻愪緵鐒＄媭鎱嬬殑 Streamable HTTP MCP endpoint銆傚叐绋ā寮忕殕鏆撮湶鍚屼竴绲勫伐鍏凤細`list_vaults`銆乣list_files`銆乣read_file`銆乣read_file_at_commit`銆乣search`銆乣link_graph`銆乣changes_since`銆乣write_file`銆乣delete_file`銆乣write_files` 鑸?`move_file`銆俙write_files` 閬╁悎鍘熷瓙鐨勫闋?wiki 绶ㄨ集锛宍move_file` 閬╁悎淇濈暀姝峰彶鐨勯噸鏂板懡鍚嶆垨姝告獢绉诲嫊銆傚鍏ラ宸ュ叿鏈夐€熺巼闄愬埗锛屾瘡绲?`(token, vault)` 姣忓垎閻樹笂闄?60 娆″鍏ワ紝涓斾竴鍊?`write_files` 鎵规鍙秷鑰椾竴娆″鍏ヨ閷勩€傛悳灏嬭珛姹傛渶澶氭巸鎻?5000 鍊嬪彲瑕?tree 妾旀銆佽繑鍥?500 姊濆尮閰嶏紝涓﹀湪鐢熺敘鐠板鎼滃皨鏂囧瓧绱▓閬斿埌 256 MiB 寰屽仠姝€俙link_graph` 鏈€澶氭巸鎻?5000 鍊嬪彲瑕嬫枃瀛楁獢锛屼甫浣跨敤鍚屼竴鍊嬬敓鐢㈡枃瀛楅爯绠楋紱`changes_since` 鏈€澶氳繑鍥?5000 姊濆彲瑕嬭畩鏇淬€傝秴閬?64 MiB 鐨勪簩閫蹭綅/blob 璁€鍙栧洖鎳夋渻琚嫆绲曪紝鑰屼笉鏄 base64 灞曢枊閫?JSON銆?

`http` 妯″紡瑕佹眰姣忓€?request 閮藉繀闋堝付涓婁己鏈嶅櫒閮ㄧ讲閲戦懓鐨?header锛岃垏涓€鑸悓姝?API 鐩稿悓銆?


閫欏€嬪瓙鍛戒护浠嶇劧鏄崹绔?MCP 閫茬▼銆傝嫢瑕佹妸鍚屼竴鍊?Streamable HTTP transport 鎺涘埌涓绘湇鍕欑鍙ｏ紝璜嬭ō瀹?`[mcp].embed_in_serve = true` 涓﹀煼琛?`pkvsyncd serve`銆?
### 绡勪緥

```bash
# stdio锛宼oken 渚嗚嚜鐠板璁婃暩
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 鏈 Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

灏?PKV Sync 鐨?release binary 涓嬭級鍒扮洰鍓嶅彲鍩疯妾旀梺閭娿€?

### 鐢ㄦ硶

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 閬搁爡

- `--dry-run`锛氬彧椤ず閬镐腑鐨?release銆乤sset 鑸囩洰妯欒矾寰戯紝涓嶅闅涗笅杓夈€?
- `--yes`锛氱暐閬庝簰鍕曠⒑瑾嶆彁绀恒€?
- `--version <VERSION>`锛氫笅杓夋寚瀹?release锛屼緥濡?`1.4.3`锛岃€岄潪鏈€鏂扮増鏈€?

### 瑾槑

姝ゅ懡浠ゆ渻鐐虹洰鍓嶅钩鍙版寫閬稿皪鎳夌殑 release asset锛屼緷 `SHA256SUMS` 椹楄瓑涓嬭級鍏у锛屽皣 `pkvsyncd.new` 瀵叆鐩墠 binary 鐨勬梺閭婏紙Windows 鐐?`pkvsyncd.new.exe`锛夛紝涓﹀嵃鍑?systemd 鎴栨墜鍕曞垏鎻涚殑姝ラ銆傚畠涓嶆渻鐔辨浛鎻涙鍦ㄩ亱琛屼腑鐨勪己鏈嶅櫒銆?

Docker 鑸?Kubernetes 閮ㄧ讲鎳夋敼浠ユ媺鍙栨垨鏇存彌 image tag 鐨勬柟寮忓崌绱氾紝涓﹂噸鍟熸湇鍕欐垨閫茶 rollout銆傜暥鍛戒护鍋垫脯鍒板鍣ㄧ挵澧冩檪锛屽彧鏈冨嵃鍑轰互 image 鐐轰富鐨勫崌绱氭寚寮曚甫閫€鍑猴紝涓嶆渻瀵叆鏃佽矾 binary銆?

### 绡勪緥

```bash
# 闋愯鍗囩礆瑷堢暙
pkvsyncd upgrade --dry-run

# 涓嬭級鏈€鏂颁笖閫氶亷椹楄瓑鐨?binary
pkvsyncd upgrade --yes

# 涓嬭級鎸囧畾 release
pkvsyncd upgrade --yes --version 1.4.3
```
