# CLI 霠堩嵓霟办姢

[English](./cli-reference.md) | [绠€浣撲腑鏂嘳(./cli-reference.zh-CN.md) | [绻侀珨涓枃](./cli-reference.zh-Hant.md) | [鏃ユ湰瑾瀅(./cli-reference.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.5.

`pkvsyncd`電?PKV Sync 靹滊矂 雿半 氚旍澊雱堧Μ鞛呺媹雼? HTTP/WebSocket 霃欔赴頇?API, 甏€毽瀽 UI, MCP 靹滊矂, 攴鸽Μ瓿?靻岇垬鞚?鞖挫榿鞖?靹滊笇旎るЖ霌滊ゼ 順胳姢韺呿暕雼堧嫟.

## 旮€搿滊矊 鞓奠厴

雼れ潓 頂岆灅攴鸽姅 氇摖 靹滊笇旎るЖ霌滌棎 瓿淀喌鞙茧 鞝侅毄霅╇媹雼?

- `-c, --config <PATH>`: TOML 靹れ爼 韺岇澕 瓴诫鞛呺媹雼? 旮半掣臧? `/etc/pkv-sync/config.toml`.
- `-h, --help`: 霃勳泙毵愳潉 響滌嫓頃╇媹雼?
- `-V, --version`: CLI 氩勳爠鞚?於滊牓頃╇媹雼?

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 靹滊笇旎るЖ霌?

`pkvsyncd`電?9臧滌潣 靹滊笇旎るЖ霌滊ゼ 鞝滉车頃╇媹雼? 臧€鞛?鞛愳＜ 靷毄霅橂姅 鞖挫榿 頋愲鞚€ `serve`, `genkey`, `migrate up`, `user add`, `backup`, `restore`鞛呺媹雼?

## pkvsyncd serve

HTTP 靹滊矂毳?鞁滌瀾頃╇媹雼?

### 臧滌殧

```text
pkvsyncd serve
```

### 靹る獏

韻茧笖毽?霃欔赴頇?HTTP 毽姢雱? 甏€毽瀽 UI, SSE 鞀ろ姼毽? Git smart HTTP 霛检毎韸? 攴鸽Μ瓿?靹れ爼霅?瓴届毎 MCP HTTP 鞐旊摐韽澑韸鸽ゼ 鞁ろ枆頃╇媹雼? 毽姢雱堧姅 `config.toml`鞚?`[server].bind_addr`鞐?氚旍澑霐╇惄雼堧嫟. systemd 鞎勲灅雮?旎厡鞚措剤 鞎堨棎靹?韽犯霛检毚霌?頂勲靹胳姢搿?鞁ろ枆頃橃嫮鞁滌槫.

### 鞓堨嫓

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

雿办澊韯半矤鞚挫姢 毵堨澊攴鸽爤鞚挫厴 旎るЖ霌滌瀰雼堧嫟. 鞙犾澕頃?鞛戩梾鞚€ `up`鞛呺媹雼?

### 臧滌殧

```text
pkvsyncd migrate up
```

### 靹る獏

`server/migrations/`鞐?鞛堧姅 氇摖 氙胳爜鞖?SQLite 毵堨澊攴鸽爤鞚挫厴鞚?`[storage].db_path`鞚?雿办澊韯半矤鞚挫姢鞐?雽€頃?鞁ろ枆頃╇媹雼? 鞛嫟頄夗暣霃?鞎堨爠頃橂┌, 鞚措 鞝侅毄霅?毵堨澊攴鸽爤鞚挫厴鞚€ 瓯措剤霚侂媹雼? HTTP 靹滊矂 霕愴暅 鞁滌瀾 鞁滌爯鞐?氙胳爜鞖?毵堨澊攴鸽爤鞚挫厴鞚?鞁ろ枆頃橂瘈搿? 靾橂彊 `migrate up`鞚€ 鞚茧皹鞝侅溂搿?旖滊摐 氤店惮 頋愲鞚措倶 鞓ろ攧霛检澑 氚膘梾鞚?毵堨澊攴鸽爤鞚挫厴頃?霑岇棎毵?頃勳殧頃╇媹雼?

### 鞓堨嫓

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

`[server].deployment_key`鞐?鞝來暕頃?氍挫瀾鞙?氚绊彫 韨るゼ 靸濎劚頃╇媹雼?

### 臧滌殧

```text
pkvsyncd genkey
```

### 靹る獏

鞎旐樃頃欖爜鞙茧 氍挫瀾鞙勳澑 `k_*` 韱犿伆鞚?stdout鞙茧 於滊牓頃╇媹雼? 攴?臧掛潉 `config.toml`鞐?攵欖棳雱ｊ碃 鞛愳泊鞝侅澑 鞎堨爠頃?毂勲剱鞚?韱淀暣 頂岆煬攴胳澑/甏€毽瀽 韥措澕鞚挫柛韸胳棎 瓿奠湢頃橃嫮鞁滌槫.

### 鞓堨嫓

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

靷毄鞛?甏€毽?旎るЖ霌滌瀰雼堧嫟. 鞖挫榿 氤店惮(牍勲皜氩堩樃 攵勳嫟, 瓿勳爼 鞛犼笀) 氚?氤挫“ 鞖挫榿鞛?瓿勳爼鞚?鞀ろ伂毽巾姼 旮半皹 攵€韸胳姢韸鸽灅頃戩棎 鞙犾毄頃╇媹雼?

### 臧滌殧

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 靹滊笇旎るЖ霌?

- `add <USERNAME> [--admin]`: 靷毄鞛愲ゼ 靸濎劚頃橂┌, 牍勲皜氩堩樃毳?雽€頇旐槙鞙茧 鞛呺牓氚涭姷雼堧嫟.
- `passwd <USERNAME>`: 靷毄鞛愳潣 牍勲皜氩堩樃毳?鞛劋鞝曧晿氅? 靸?臧掛潉 雽€頇旐槙鞙茧 鞛呺牓氚涭姷雼堧嫟.
- `list`: 氇摖 靷毄鞛愲ゼ 甏€毽瀽/頇滌劚 靸來儨 氚?靸濎劚 鞁滉皝瓿?頃粯 雮橃棿頃╇媹雼?
- `set-active <USERNAME> --active <true|false>`: 靷毄鞛愲ゼ 牍勴櫆靹表檾頃橁卑雮?雼れ嫓 頇滌劚頇旐暕雼堧嫟. 牍勴櫆靹表檾霅?靷毄鞛愲姅 韱犿伆鞚€ 鞙犾霅橃毵?搿滉犯鞚胳澊雮?霃欔赴頇旊姅 攵堦皜電ロ暕雼堧嫟.

### 鞓堨嫓

```bash
# 牍勳儊 鞝戧芳鞖?甏€毽瀽 瓿勳爼 靸濎劚
pkvsyncd user add alice --admin

# 鞛婌柎氩勲Π 牍勲皜氩堩樃 鞛劋鞝?
pkvsyncd user passwd alice

# 雿办澊韯半ゼ 靷牅頃橃 鞎婈碃 霒犽倶電?靷毄鞛?牍勴櫆靹表檾
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

PKV Sync 氤柬姼鞚?bare git 鞝€鞛レ唽毳?霐旍姢韥潣 鞚茧皹 韺岇澕 韸鸽Μ搿?韼检硱雰呺媹雼?

### 臧滌殧

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 鞓奠厴

- `-o, --output <DIR>`: 於滊牓 霐旊爥韯半Μ鞛呺媹雼?臁挫灛頃橃 鞎婈卑雮?牍勳柎 鞛堨柎鞎?頃╇媹雼?.
- `--at <SHA>`: 韸轨爼 commit 鞁滌爯鞐愳劀 materialize頃╇媹雼?旮半掣臧? HEAD).

### 靹る獏

`data_dir/vaults/<vault-id>` 鞎勲灅鞚?氤柬姼 bare git 鞝€鞛レ唽毳?鞚届柎 臧?韺岇澕鞚?於滊牓 霐旊爥韯半Μ鞐?旮半頃╇媹雼?

- 韰嶌姢韸?韺岇澕鞚€ 攴鸽寑搿?旮半霅╇媹雼?
- `pkvsync_pointer` JSON鞙茧 鞝€鞛ル悳 氚旍澊雱堧Μ 韺岇澕鞚€ 靹滊矂鞚?blob 鞝€鞛レ唽(`data_dir/blobs/`)鞐愳劀 鞁れ牅 blob鞚?氤奠偓頃橃棳 頃挫劃霅╇媹雼?

鞚?旎るЖ霌滊姅 霃欔赴鞁濎澊氅?靹滊矂臧€ 鞁ろ枆 欷戩澕 頃勳殧臧€ 鞐嗢姷雼堧嫟. 靹れ爼霅?`data_dir` 鞎勲灅鞚?鞓敂鞀ろ伂 git 鞝€鞛レ唽鞕€ blob 鞝€鞛レ唽鞐愳劀 歆侅爲 鞚届姷雼堧嫟.

### 鞓堨嫓

```bash
# 斓滌嫚 氩勳爠 materialize
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 韸轨爼 commit materialize
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 膦呺 旖旊摐

- `0`: 靹标车.
- `1`: 於滊牓 霐旊爥韯半Μ臧€ 牍勳柎 鞛堨 鞎婌潓, 氤柬姼毳?彀眷潉 靾?鞐嗢潓, blob 雸勲澖, 鞛橂霅?commit SHA 霌膘潣 鞓る.

> 氤柬姼 ID電?32鞛愲Μ 靻岆鞛?16歆勳垬鞛呺媹雼?雽€鞁?鞐嗢潓). 鞙?鞓堨嫓電?鞁れ牅 順曥嫕鞚?ID毳?靷毄頃╇媹雼? 甏€毽瀽 UI鞕€ `pkvsyncd user list`鞐?鞙犿毃頃?ID臧€ 響滌嫓霅╇媹雼?

## pkvsyncd backup

靹滊矂 雿办澊韯半ゼ 頊措寑 臧€電ロ暅 氚膘梾 霐旊爥韯半Μ搿?鞀る儏靾忢暕雼堧嫟.

### 臧滌殧

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 鞓奠厴

- `-o, --output <DIR>`: 氚膘梾 於滊牓 霐旊爥韯半Μ鞛呺媹雼?臁挫灛頃橃 鞎婈卑雮?牍勳柎 鞛堨柎鞎?頃╇媹雼?.
- `--data-dir <DIR>`: 鞓ろ攧霛检澑 鞛戩梾鞚?鞙勴暅 雿办澊韯?霐旊爥韯半Μ 鞓る矂霛检澊霌滌瀰雼堧嫟. 旮半掣臧掛潃 搿滊摐霅?靹れ爼鞚?`[storage].data_dir`鞛呺媹雼?
- `--gzip`: 氚膘梾 霐旊爥韯半Μ 鞓嗢棎 `.tar.gz` 鞎勳勾鞚措笇霃?頃粯 靸濎劚頃╇媹雼?
- `--include-config`: 搿滊摐頃?`config.toml`鞚?氚膘梾鞐?韽暔頃╇媹雼? 旮半掣 氚膘梾鞚€ 氚绊彫 韨れ檧 搿滌滑 牍勲皜鞚?霌れ柎 鞛堨潉 靾?鞛堨柎 config毳?鞝滌櫢頃╇媹雼?

### 靹る獏

SQLite 雿办澊韯半矤鞚挫姢(VACUUM INTO 靷毄), 氇摖 氤柬姼鞚?bare git 鞝€鞛レ唽, 攴鸽Μ瓿?blob 鞝€鞛レ唽毳?`MANIFEST.json`鞚?韽暔霅?鞛愳泊 鞕勱舶順?霐旊爥韯半Μ搿?鞀る儏靾忢暕雼堧嫟. 氚膘梾 欷戩棎霃?HTTP 靹滊矂電?瓿勳啀 鞁ろ枆霅?靾?鞛堨毵? push, blob upload, rollback, vault deletion, GC 臧欖潃 storage write電?data-dir snapshot lock 霋れ棎靹?雽€旮绊晿瓿?氚膘梾鞚?雭濍倻 霋?歆勴枆霅╇媹雼?

旮半掣鞝侅溂搿?氚膘梾鞚€ `config.toml`鞚?靸濍灥頃╇媹雼? 靹れ爼鞚?鞝€鞛ロ晿瓿?攴?鞎堨潣 牍勲皜鞚?氤错樃頃橂牑電?瓴届毎鞐愲 `--include-config`毳?於旉皜頃橃劯鞖?

### 鞓堨嫓

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

氚膘梾 霐旊爥韯半Μ毳?雿办澊韯?霐旊爥韯半Μ鞐?氤奠洂頃╇媹雼?

### 臧滌殧

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 鞓奠厴

- `-i, --input <DIR>`: `MANIFEST.json`鞚?韽暔霅?氚膘梾 霐旊爥韯半Μ鞛呺媹雼?
- `--data-dir <DIR>`: 雽€靸?雿办澊韯?霐旊爥韯半Μ 鞓る矂霛检澊霌滌瀰雼堧嫟. 旮半掣臧掛潃 `[storage].data_dir`鞛呺媹雼?
- `--force`: 氤奠洂 鞝勳棎 牍勳柎 鞛堨 鞎婌潃 雽€靸?雿办澊韯?霐旊爥韯半Μ毳?牍勳泚雼堧嫟.

### 靹る獏

氚膘梾鞚?`MANIFEST.json`鞚?瓴€歃濏暅 霋?SQLite DB, 氤柬姼 鞝€鞛レ唽, blob 鞝€鞛レ唽毳?雽€靸?雿办澊韯?霐旊爥韯半Μ搿?氤奠偓頃╇媹雼? 氤奠洂 鞝勳棎 HTTP 靹滊矂毳?欷戩頃橃嫮鞁滌槫. 雿?鞓る灅霅?靹滊矂 氩勳爠鞐愳劀 毵岆摛鞏挫 氚膘梾鞚?氤奠洂頃橂姅 瓴届毎, 氤奠洂 頉?`pkvsyncd migrate up`鞚?鞁ろ枆頃橃嫮鞁滌槫.

### 鞓堨嫓

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

氤柬姼 git 鞝€鞛レ唽鞕€ 旖橅厫旄?欤检唽 歆€鞝?blob鞚?瓴€歃濏暕雼堧嫟.

### 臧滌殧

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 鞓奠厴

- `--data-dir <DIR>`: 雿办澊韯?霐旊爥韯半Μ 鞓る矂霛检澊霌滌瀰雼堧嫟.
- `--no-fail`: 瓴€歃濎棎靹?鞓る臧€ 氚滉铂霅橂崝霛茧弰 膦呺 旖旊摐 0鞚?氚橅櫂頃╇媹雼? 韼橃澊歆?鞐嗢澊 搿滉犯毵?雮赴霠る姅 氇媹韯半 鞀ろ伂毽巾姼鞐?鞙犾毄頃╇媹雼?

### 靹る獏

`data_dir/vaults/` 鞎勲灅鞚?臧?氤柬姼鞐?雽€頃?雼れ潓鞚?靾橅枆頃╇媹雼?

- bare 鞝€鞛レ唽鞐愳劀 `git fsck --strict`毳?鞁ろ枆頃╇媹雼?
- HEAD 韸鸽Μ毳?靾滍殞頃橂┌ 氇摖 `pkvsync_pointer`臧€ 攴?韺岇澕氇呹臣 鞚检箻頃橂姅 鞓敂鞀ろ伂 SHA-256鞚?臧€歆?blob鞙茧 頃挫劃霅橂姅歆€ 瓴€歃濏暕雼堧嫟.

氤柬姼氤?鞓る 臧滌垬毳?氤搓碃頃╇媹雼? 鞏措枻 氤柬姼霛茧弰 鞓る臧€ 鞛堨溂氅?0鞚?鞎勲媽 旖旊摐搿?膦呺頃橂┌, `--no-fail`鞚?靹れ爼霅?瓴届毎鞐愲姅 攴鸽爣歆€ 鞎婌姷雼堧嫟.

### 鞓堨嫓

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

AI 霃勱惮毳?鞙勴暅 MCP(Model Context Protocol) 靹滊矂毳?鞁滌瀾頃╇媹雼?

### 臧滌殧

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 鞓奠厴

- `--transport <stdio|http>`: 鞝勳啞 氇摐鞛呺媹雼? 旮半掣臧? `stdio`.
- `--vault <VAULT-ID>`: stdio鞐愳劀電?頃勳垬鞚措┌, 韥措澕鞚挫柛韸胳棎 雲胳稖霅橂姅 雼澕 氤柬姼鞛呺媹雼?
- `--token <PKS-TOKEN>`: stdio鞖?bearer 霐旊皵鞚挫姢 韱犿伆鞛呺媹雼? 靸濍灥頃橂┐ `PKV_TOKEN` 頇橁步 氤€靾橁皜 靷毄霅╇媹雼?
- `--bind <ADDR>`: HTTP 氚旍澑霐?欤检唽鞛呺媹雼? 旮半掣臧? `127.0.0.1:6711`.

### 靹る獏

`stdio` 氇摐電?stdin鞐愳劀 JSON-RPC毳?鞚疥碃 stdout鞙茧 JSON-RPC毳?鞌侂媹雼? `http` 氇摐電?`/mcp`鞐愳劀 氍挫儊韮?Streamable HTTP MCP 鞐旊摐韽澑韸鸽ゼ 鞝滉车頃╇媹雼? 霊?氇摐 氇憪 霃欖澕頃?韴挫厠, 歃?`list_vaults`, `list_files`, `read_file`, `read_file_at_commit`, `search`, `link_graph`, `changes_since`, `write_file`, `delete_file`, `write_files`, `move_file`鞚?雲胳稖頃╇媹雼? `write_files`電?鞐煬 wiki 韼橃澊歆€ 韼胳鞚?鞗愳瀽鞝侅溂搿?氍鹅潉 霑? `move_file`鞚€ 旮半鞚?氤挫〈頃橂姅 鞚措 氤€瓴届澊雮?氤搓磤 鞚措彊鞐?靷毄頃╇媹雼? 鞊瓣赴 韴挫潃 `(token, vault)`毵堧嫟 攵勲嫻 60須?鞊瓣赴搿?靻嶋弰 鞝滍暅霅橂┌, `write_files` batch電?鞊瓣赴 旮半 頃橂倶毵?靷毄頃╇媹雼? 瓴€靸?鞖旍箔鞚€ 斓滊寑 5000臧滌潣 響滌嫓 臧€電ロ暅 tree files毳?鞀れ簲頃橁碃 斓滊寑 500 matches毳?氚橅櫂頃橂┌, 頂勲雿曥厴鞐愳劀電?瓴€靸夗暅 text臧€ 256 MiB鞐?霃勲嫭頃橂┐ 欷戨嫧頃╇媹雼? `link_graph`電?臧欖潃 頂勲雿曥厴 text 鞓堨偘鞙茧 斓滊寑 5000臧滌潣 響滌嫓 臧€電ロ暅 text 韺岇澕鞚?鞀れ簲頃橁碃, `changes_since`電?斓滊寑 5000臧滌潣 響滌嫓 臧€電ロ暅 氤€瓴?頃鞚?氚橅櫂頃╇媹雼? 64 MiB毳?雱橂姅 binary/blob 鞚疥赴 鞚戨嫷鞚€ base64搿?JSON鞐?頇曥灔霅橂姅 雽€鞁?瓯半秬霅╇媹雼?

`http` 氇摐電?鞚茧皹 霃欔赴頇?API鞕€ 毵堨艾臧€歆€搿?氇摖 鞖旍箔鞐?靹滊矂 氚绊彫 韨?項る崝毳?韽暔頃挫暭 頃╇媹雼?


鞚?靹滊笇旎るЖ霌滊姅 瓿勳啀 霃呺 MCP 頂勲靹胳姢鞛呺媹雼? 臧欖潃 Streamable HTTP transport毳?氅旍澑 靹滊矂 韽姼鞐愳劀 鞝滉车頃橂牑氅?`[mcp].embed_in_serve = true`毳?靹れ爼頃橁碃 `pkvsyncd serve`毳?靷毄頃橃劯鞖?
### 鞓堨嫓

```bash
# 頇橁步鞐愳劀 臧€鞝胳槰 韱犿伆鞙茧 stdio 鞁ろ枆
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 搿滌滑 Streamable HTTP 鞐旊摐韽澑韸?
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

PKV Sync 毽措Μ鞀?氚旍澊雱堧Μ毳?順勳灛 鞁ろ枆 韺岇澕 鞓嗢棎 頃粯 雼れ毚搿滊摐頃╇媹雼?

### 臧滌殧

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 鞓奠厴

- `--dry-run`: 鞎勲瓴冸弰 雼れ毚搿滊摐頃橃 鞎婈碃 靹犿儩霅?毽措Μ鞀? 鞐愳厠, 雽€靸?瓴诫毳?響滌嫓頃╇媹雼?
- `--yes`: 雽€頇旐槙 頇曥澑 頂勲‖頂勴姼毳?瓯措剤霚侂媹雼?
- `--version <VERSION>`: 斓滌嫚 毽措Μ鞀?雽€鞁?`1.4.5` 臧欖潃 韸轨爼 毽措Μ鞀るゼ 雼れ毚搿滊摐頃╇媹雼?

### 靹る獏

鞚?旎るЖ霌滊姅 順勳灛 頂岆灚韽检棎 頃措嫻頃橂姅 毽措Μ鞀?鞐愳厠鞚?靹犿儩頃橁碃, 雼れ毚搿滊摐毳?`SHA256SUMS`鞕€ 雽€臁绊晿鞐?瓴€歃濏晿氅? 順勳灛 氚旍澊雱堧Μ 鞓嗢棎 `pkvsyncd.new`(Windows鞐愳劀電?`pkvsyncd.new.exe`)毳?旮半頃?霋? systemd/靾橂彊 甑愳泊 鞝堨皑毳?於滊牓頃╇媹雼? 鞁ろ枆 欷戩澑 靹滊矂毳?頃?毽攲霠堨澊鞀ろ晿歆€電?鞎婌姷雼堧嫟.

Docker 氚?Kubernetes 氚绊彫電?鞚措歆€ 韮滉犯毳?頀€頃橁卑雮?氤€瓴巾暅 雼れ潓 靹滊箘鞀るゼ 鞛嫓鞛戫晿瓯半倶 搿れ晞鞗冺晿電?氚╈嫕鞙茧 鞐呹犯霠堨澊霌滍暣鞎?頃╇媹雼? 旎厡鞚措剤 頇橁步鞚?臧愳頃橂┐ 鞚措歆€ 旮半皹 鞎堧偞毳?於滊牓頃橁碃 氚旍澊雱堧Μ毳?旮半頃橃 鞎婌潃 毂?膦呺頃╇媹雼?

### 鞓堨嫓

```bash
# 鞐呹犯霠堨澊霌?瓿勴殟 氙鸽Μ氤搓赴
pkvsyncd upgrade --dry-run

# 瓴€歃濍悳 斓滌嫚 氚旍澊雱堧Μ 雼れ毚搿滊摐
pkvsyncd upgrade --yes

# 韸轨爼 毽措Μ鞀?雼れ毚搿滊摐
pkvsyncd upgrade --yes --version 1.4.5
```
