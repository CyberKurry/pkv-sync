# CLI 鍙傝€?

[English](./cli-reference.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./cli-reference.zh-Hant.md) | [鏃ユ湰瑾瀅(./cli-reference.ja.md) | [頃滉淡鞏碷(./cli-reference.ko.md)

鏂囨。鐗堟湰锛歷1.4.3銆?

`pkvsyncd` 鏄?PKV Sync 鐨勬湇鍔＄瀹堟姢杩涚▼浜岃繘鍒舵枃浠躲€傚畠鎵胯浇 HTTP/WebSocket 鍚屾 API銆佺鐞嗙晫闈€丮CP 鏈嶅姟鍣紝浠ュ強涓€灏忕粍杩愮淮瀛愬懡浠ゃ€?

## 鍏ㄥ眬閫夐」

浠ヤ笅閫夐」瀵规墍鏈夊瓙鍛戒护鐢熸晥锛?

- `-c, --config <PATH>`锛歍OML 閰嶇疆鏂囦欢璺緞銆傞粯璁ゅ€硷細`/etc/pkv-sync/config.toml`銆?
- `-h, --help`锛氭樉绀哄府鍔╀俊鎭€?
- `-V, --version`锛氭墦鍗?CLI 鐗堟湰銆?

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 瀛愬懡浠?

`pkvsyncd` 鎻愪緵涔濅釜瀛愬懡浠ゃ€傛渶甯哥敤鐨勮繍缁存祦绋嬫槸 `serve`銆乣genkey`銆乣migrate up`銆乣user add`銆乣backup` 鍜?`restore`銆?

## pkvsyncd serve

鍚姩 HTTP 鏈嶅姟鍣ㄣ€?

### 姒傝堪

```text
pkvsyncd serve
```

### 璇存槑

杩愯鍏紑鐨勫悓姝?HTTP 鐩戝惉鍣ㄣ€佺鐞嗙晫闈€丼SE 娴併€丟it smart HTTP 璺敱锛屼互鍙婂湪宸查厤缃椂鐨?MCP HTTP 绔偣銆傜洃鍚櫒缁戝畾鍒?`config.toml` 涓殑 `[server].bind_addr`銆傝灏嗗叾浣滀负鍓嶅彴杩涚▼鍦?systemd 鎴栧鍣ㄤ腑杩愯銆?

### 绀轰緥

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

鏁版嵁搴撹縼绉诲懡浠ゃ€傚敮涓€鏀寔鐨勬搷浣滄槸 `up`銆?

### 姒傝堪

```text
pkvsyncd migrate up
```

### 璇存槑

灏?`server/migrations/` 涓墍鏈夊緟鎵ц鐨?SQLite 杩佺Щ搴旂敤鍒?`[storage].db_path` 澶勭殑鏁版嵁搴撱€傚彲瀹夊叏鍦伴噸澶嶈繍琛岋紝宸插簲鐢ㄧ殑杩佺Щ浼氳璺宠繃銆侶TTP 鏈嶅姟鍣ㄥ湪鍚姩鏃朵篃浼氳繍琛屽緟鎵ц鐨勮縼绉伙紝鍥犳鎵嬪姩鎵ц `migrate up` 閫氬父鍙湪鍐锋仮澶嶆祦绋嬫垨杩佺Щ绂荤嚎澶囦唤鏃舵墠闇€瑕併€?

### 绀轰緥

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

鐢熸垚涓€涓€傚悎鐢ㄤ簬 `[server].deployment_key` 鐨勯殢鏈洪儴缃插瘑閽ャ€?

### 姒傝堪

```text
pkvsyncd genkey
```

### 璇存槑

鍚?stdout 鎵撳嵃涓€涓姞瀵嗗闅忔満鐨?`k_*` 浠ょ墝銆傚皢璇ュ€肩矘璐村埌 `config.toml` 涓紝骞堕€氳繃浣犺嚜宸辩殑瀹夊叏娓犻亾鍒嗗彂缁?plugin/admin 瀹㈡埛绔€?

### 绀轰緥

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

鐢ㄦ埛绠＄悊鍛戒护銆傞€傜敤浜庤繍缁存仮澶嶏紙蹇樿瀵嗙爜銆佽处鎴疯閿佸畾锛変互鍙婇€氳繃鑴氭湰鎵归噺鍒濆鍖栨绾ц繍缁磋处鎴枫€?

### 姒傝堪

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 瀛愬懡浠?

- `add <USERNAME> [--admin]`锛氬垱寤轰竴涓敤鎴凤紝骞朵互浜や簰鏂瑰紡鎻愮ず杈撳叆瀵嗙爜銆?
- `passwd <USERNAME>`锛氶噸缃煇鐢ㄦ埛鐨勫瘑鐮侊紝浜や簰寮忔彁绀鸿緭鍏ユ柊瀵嗙爜銆?
- `list`锛氬垪鍑烘墍鏈夌敤鎴凤紝鍖呮嫭鍏剁鐞嗗憳/鍚敤鐘舵€佷互鍙婂垱寤烘椂闂淬€?
- `set-active <USERNAME> --active <true|false>`锛氱鐢ㄦ垨閲嶆柊鍚敤鏌愪釜鐢ㄦ埛銆傝绂佺敤鐨勭敤鎴蜂繚鐣欏叾浠ょ墝锛屼絾鏃犳硶鐧诲綍鎴栧悓姝ャ€?

### 绀轰緥

```bash
# 鍒涘缓涓€涓敤浜庣揣鎬ヨ闂殑绠＄悊鍛樿处鎴?
pkvsyncd user add alice --admin

# 閲嶇疆蹇樿鐨勫瘑鐮?
pkvsyncd user passwd alice

# 鍦ㄤ笉鍒犻櫎鏁版嵁鐨勬儏鍐典笅绂佺敤涓€涓鑱岀敤鎴?
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

灏?PKV Sync 淇濋櫓搴撶殑瑁?git 浠撳簱灞曞紑涓虹鐩樹笂鐨勬櫘閫氭枃浠舵爲銆?

### 姒傝堪

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 閫夐」

- `-o, --output <DIR>`锛氳緭鍑虹洰褰曪紙蹇呴』涓嶅瓨鍦ㄦ垨涓虹┖锛夈€?
- `--at <SHA>`锛氬湪鎸囧畾 commit 澶勭墿鍖栵紙榛樿鍊硷細HEAD锛夈€?

### 璇存槑

璇诲彇 `data_dir/vaults/<vault-id>` 涓嬩繚闄╁簱鐨勮８ git 浠撳簱锛屽苟灏嗘瘡涓枃浠跺啓鍏ヨ緭鍑虹洰褰曪細

- 鏂囨湰鏂囦欢鎸夊師鏍峰啓鍏ャ€?
- 浠?`pkvsync_pointer` JSON 褰㈠紡瀛樺偍鐨勪簩杩涘埗鏂囦欢锛岄€氳繃浠庢湇鍔″櫒鐨?blob 瀛樺偍锛坄data_dir/blobs/`锛夊鍒跺疄闄呯殑 blob 鏉ヨВ鏋愩€?

璇ュ懡浠ゆ槸鍚屾鐨勶紝涓斾笉瑕佹眰鏈嶅姟鍣ㄦ鍦ㄨ繍琛屻€傚畠鐩存帴浠庡凡閰嶇疆鐨?`data_dir` 涓嬬殑纾佺洏 git 浠撳簱鍜?blob 瀛樺偍璇诲彇銆?

### 绀轰緥

```bash
# 鐗╁寲鏈€鏂扮増鏈?
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 鐗╁寲鐗瑰畾 commit
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 閫€鍑虹爜

- `0`锛氭垚鍔熴€?
- `1`锛氶敊璇紝渚嬪杈撳嚭鐩綍闈炵┖銆佷繚闄╁簱涓嶅瓨鍦ㄣ€乥lob 缂哄け鎴?commit SHA 鏃犳晥銆?

> 淇濋櫓搴?ID 鏄?32 浣嶇殑灏忓啓鍗佸叚杩涘埗锛堜笉鍚煭妯嚎锛夈€備笂闈㈢殑绀轰緥浣跨敤鐪熷疄褰㈡€佺殑 ID锛涚鐞嗙晫闈笌 `pkvsyncd user list` 浼氭樉绀烘湁鏁堢殑 ID銆?

## pkvsyncd backup

灏嗘湇鍔″櫒鏁版嵁蹇収鍒颁竴涓彲绉绘鐨勫浠界洰褰曚腑銆?

### 姒傝堪

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 閫夐」

- `-o, --output <DIR>`锛氬浠借緭鍑虹洰褰曪紙蹇呴』涓嶅瓨鍦ㄦ垨涓虹┖锛夈€?
- `--data-dir <DIR>`锛氱敤浜庣绾胯繍缁寸殑 data 鐩綍瑕嗙洊椤广€傞粯璁や娇鐢ㄥ姞杞界殑閰嶇疆涓殑 `[storage].data_dir`銆?
- `--gzip`锛氬湪澶囦唤鐩綍鏃侀澶栫敓鎴愪竴涓?`.tar.gz` 褰掓。鏂囦欢銆?
- `--include-config`锛氭妸宸插姞杞界殑 `config.toml` 涓€骞跺啓鍏ュ浠姐€傞粯璁ゅ浠戒細鐪佺暐閰嶇疆鏂囦欢锛屽洜涓哄叾涓彲鑳藉寘鍚儴缃插瘑閽ュ拰鏈満绉樺瘑銆?

### 璇存槑

灏?SQLite 鏁版嵁搴擄紙閫氳繃 VACUUM INTO锛夈€佹瘡涓繚闄╁簱鐨勮８ git 浠撳簱浠ュ強 blob 瀛樺偍锛屽揩鐓у埌涓€涓甫鏈?`MANIFEST.json` 鐨勮嚜鍖呭惈鐩綍涓€傚浠芥湡闂?HTTP 鏈嶅姟鍣ㄥ彲浠ョ户缁繍琛岋紱push銆乥lob 涓婁紶銆佸洖婊氥€佷繚闄╁簱鍒犻櫎鍜?GC 绛夊瓨鍌ㄥ啓鍏ヤ細鍦?data-dir 蹇収閿佷箣鍚庢帓闃燂紝鐩村埌澶囦唤瀹屾垚銆?

榛樿鎯呭喌涓嬶紝澶囦唤浼氱渷鐣?`config.toml`锛涘彧鏈夊湪浣犳槑纭淇濆瓨閰嶇疆骞朵繚鎶ゅ叾涓瀵嗘椂锛屾墠娣诲姞 `--include-config`銆?

### 绀轰緥

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

灏嗕竴涓浠界洰褰曟仮澶嶅埌鎸囧畾鐨?data 鐩綍涓€?

### 姒傝堪

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 閫夐」

- `-i, --input <DIR>`锛氬寘鍚?`MANIFEST.json` 鐨勫浠界洰褰曘€?
- `--data-dir <DIR>`锛氱洰鏍?data 鐩綍瑕嗙洊椤广€傞粯璁や娇鐢?`[storage].data_dir`銆?
- `--force`锛氬湪鎭㈠鍓嶆竻绌洪潪绌虹殑鐩爣 data 鐩綍銆?

### 璇存槑

鏍￠獙澶囦唤鐨?`MANIFEST.json`锛屽苟灏?SQLite 鏁版嵁搴撱€佷繚闄╁簱浠撳簱涓?blob 瀛樺偍澶嶅埗鍒扮洰鏍?data 鐩綍銆傚湪鎭㈠涔嬪墠璇峰厛鍋滄 HTTP 鏈嶅姟鍣ㄣ€傛仮澶嶅畬鎴愬悗锛屽鏋滀綘鎭㈠鐨勬槸鏇磋€佹湇鍔″櫒鐗堟湰鐢熸垚鐨勫浠斤紝璇疯繍琛?`pkvsyncd migrate up`銆?

### 绀轰緥

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

鏍￠獙淇濋櫓搴撶殑 git 浠撳簱鍜屽唴瀹瑰鍧€鐨?blob銆?

### 姒傝堪

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 閫夐」

- `--data-dir <DIR>`锛歞ata 鐩綍瑕嗙洊椤广€?
- `--no-fail`锛氬嵆浣挎牎楠屽彂鐜伴敊璇篃杩斿洖閫€鍑虹爜 0銆傞€傜敤浜庡笇鏈涗粎璁板綍鑰屼笉瑙﹀彂鍛婅鐨勭洃鎺ц剼鏈€?

### 璇存槑

瀵?`data_dir/vaults/` 涓嬬殑姣忎釜淇濋櫓搴擄細

- 鍦ㄨ８浠撳簱涓婅繍琛?`git fsck --strict`銆?
- 閬嶅巻 HEAD 鏍戯紝骞堕獙璇佹瘡涓?`pkvsync_pointer` 閮借兘瑙ｆ瀽鍒颁竴涓?blob锛屼笖鍏跺湪纾佺洏涓婄殑 SHA-256 涓庢枃浠跺悕鍖归厤銆?

鎸変繚闄╁簱鎶ュ憡閿欒璁℃暟銆傚綋浠讳綍淇濋櫓搴撳瓨鍦ㄩ敊璇椂浠ラ潪闆堕€€鍑虹爜閫€鍑猴紝闄ら潪璁剧疆浜?`--no-fail`銆?

### 绀轰緥

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

涓?AI 宸ュ叿鍚姩 MCP锛圡odel Context Protocol锛夋湇鍔″櫒銆?

### 姒傝堪

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 閫夐」

- `--transport <stdio|http>`锛氫紶杈撴ā寮忋€傞粯璁ゅ€硷細`stdio`銆?
- `--vault <VAULT-ID>`锛歴tdio 妯″紡蹇呭～锛涜鍚戝鎴风鏆撮湶鐨勫崟涓€淇濋櫓搴撱€?
- `--token <PKS-TOKEN>`锛歴tdio 妯″紡浣跨敤鐨?bearer 璁惧浠ょ墝銆傝嫢鐪佺暐锛屽垯浣跨敤 `PKV_TOKEN` 鐜鍙橀噺銆?
- `--bind <ADDR>`锛欻TTP 缁戝畾鍦板潃銆傞粯璁ゅ€硷細`127.0.0.1:6711`銆?

### 璇存槑

`stdio` 妯″紡浠?stdin 璇诲彇 JSON-RPC锛屽苟鍚?stdout 鍐欏叆 JSON-RPC銆俙http` 妯″紡鍦?`/mcp` 涓婃彁渚涗竴涓棤鐘舵€佺殑 Streamable HTTP MCP 绔偣銆備袱绉嶆ā寮忔毚闇茬浉鍚岀殑宸ュ叿闆嗭細`list_vaults`銆乣list_files`銆乣read_file`銆乣read_file_at_commit`銆乣search`銆乣link_graph`銆乣changes_since`銆乣write_file`銆乣delete_file`銆乣write_files` 鍜?`move_file`銆俙write_files` 閫傚悎鍘熷瓙鐨勫椤?wiki 缂栬緫锛宍move_file` 閫傚悎淇濈暀鍘嗗彶鐨勯噸鍛藉悕鎴栧綊妗ｇЩ鍔ㄣ€傚啓鍏ョ被宸ュ叿鎸?`(token, vault)` 闄愭祦涓烘瘡鍒嗛挓 60 娆″啓鍏ワ紝涓斾竴涓?`write_files` 鎵规鍙秷鑰椾竴娆″啓鍏ヨ褰曘€傛悳绱㈣姹傛渶澶氭壂鎻?5000 涓彲瑙?tree 鏂囦欢銆佽繑鍥?500 鏉″尮閰嶏紝骞跺湪鐢熶骇鐜鎼滅储鏂囨湰绱杈惧埌 256 MiB 鍚庡仠姝€俙link_graph` 鏈€澶氭壂鎻?5000 涓彲瑙佹枃鏈枃浠讹紝骞朵娇鐢ㄥ悓涓€涓敓浜ф枃鏈绠楋紱`changes_since` 鏈€澶氳繑鍥?5000 鏉″彲瑙佸彉鏇淬€傝秴杩?64 MiB 鐨勪簩杩涘埗/blob 璇诲彇鍝嶅簲浼氳鎷掔粷锛岃€屼笉鏄 base64 灞曞紑杩?JSON銆?

`http` 妯″紡瑕佹眰姣忎釜璇锋眰閮芥惡甯︽湇鍔″櫒閮ㄧ讲瀵嗛挜璇锋眰澶达紝涓庡父瑙勫悓姝?API 涓€鑷淬€?


杩欎釜瀛愬懡浠や粛鐒舵槸鐙珛 MCP 杩涚▼銆傝嫢瑕佹妸鍚屼竴涓?Streamable HTTP transport 鎸傚埌涓绘湇鍔＄鍙ｏ紝璇疯缃?`[mcp].embed_in_serve = true` 骞惰繍琛?`pkvsyncd serve`銆?
### 绀轰緥

```bash
# 浣跨敤鐜鍙橀噺涓殑 token 鍚姩 stdio
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 鏈湴 Streamable HTTP 绔偣
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

鍦ㄥ綋鍓嶅彲鎵ц鏂囦欢鏃佷笅杞戒竴涓?PKV Sync 鍙戣鐗堜簩杩涘埗鏂囦欢銆?

### 姒傝堪

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 閫夐」

- `--dry-run`锛氭樉绀烘墍閫夌殑鍙戣鐗堛€佽祫浜у拰鐩爣璺緞锛屼絾涓嶄笅杞戒换浣曟枃浠躲€?
- `--yes`锛氳烦杩囦氦浜掑紡纭鎻愮ず銆?
- `--version <VERSION>`锛氫笅杞芥寚瀹氱増鏈紙渚嬪 `1.4.3`锛夛紝鑰屼笉鏄渶鏂板彂琛岀増銆?

### 璇存槑

璇ュ懡浠や负褰撳墠骞冲彴閫夋嫨瀵瑰簲鐨勫彂琛岀増璧勪骇锛岄拡瀵?`SHA256SUMS` 鏍￠獙涓嬭浇缁撴灉锛屽湪褰撳墠浜岃繘鍒舵枃浠舵梺鍐欏叆 `pkvsyncd.new`锛圵indows 涓婁负 `pkvsyncd.new.exe`锛夛紝骞舵墦鍗?systemd/鎵嬪姩鍒囨崲姝ラ銆傚畠涓嶄細鐑浛鎹㈡鍦ㄨ繍琛岀殑鏈嶅姟鍣ㄣ€?

Docker 涓?Kubernetes 閮ㄧ讲搴旈€氳繃鎷夊彇鎴栦慨鏀归暅鍍忔爣绛惧苟閲嶅惎鏈嶅姟/婊氬姩鏇存柊鏉ュ崌绾с€傚綋璇ュ懡浠ゆ娴嬪埌瀹瑰櫒鐜鏃讹紝浼氭墦鍗板熀浜庨暅鍍忕殑鎸囧淇℃伅骞堕€€鍑猴紝涓嶄細鍐欏叆浠讳綍浜岃繘鍒舵枃浠躲€?

### 绀轰緥

```bash
# 棰勮鍗囩骇璁″垝
pkvsyncd upgrade --dry-run

# 涓嬭浇鏈€鏂扮殑宸叉牎楠屼簩杩涘埗鏂囦欢
pkvsyncd upgrade --yes

# 涓嬭浇鎸囧畾鐗堟湰
pkvsyncd upgrade --yes --version 1.4.3
```
