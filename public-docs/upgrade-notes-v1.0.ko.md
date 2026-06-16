# Upgrade notes: 0.x鞐愳劀 1.0鞙茧

[English](./upgrade-notes-v1.0.md) | [绠€浣撲腑鏂嘳(./upgrade-notes-v1.0.zh-CN.md) | [绻侀珨涓枃](./upgrade-notes-v1.0.zh-Hant.md) | [鏃ユ湰瑾瀅(./upgrade-notes-v1.0.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.5.

PKV Sync 1.0鞚€ 觳?stable release鞛呺媹雼? 霕愴暅 頄ロ泟 1.x maintenance毳?鞙勴暣 SQLite migration
baseline鞚?reset頃╇媹雼?

## 欷戩殧頃?database note

PKV Sync 1.0鞚€ 雼澕 `0001_initial.sql` baseline migration鞚?鞝滉车頃╇媹雼? 0.x release搿?毵岆摖
SQLite database電?1.0.0鞙茧 in-place upgrade頃?靾?鞐嗢姷雼堧嫟.

0.x server毳?鞖挫榿 欷戩澊霛茧┐ 雼れ潓 瓴诫 欷?頃橂倶毳?靹犿儩頃橃劯鞖?

1. 旮办〈 deployment電?migration 欷€牍勲ゼ 鞙勴暅 backup, materialize, export鞐?頃勳殧頃?霃欖晥毵?斓滌 0.8.x patch release鞐?鞙犾頃╇媹雼?
2. 臧?vault毳?backup 霕愲姅 materialize頃橁碃, 靸?1.0 data directory搿?鞁滌瀾頃?霋?user鞕€ vault毳?
   雼れ嫓 毵岆摛瓿?contents毳?靸?server搿?import 霕愲姅 push頃╇媹雼?
3. migration rehearsal鞚?鞁滊弰頃橁赴 鞝勳棎 0.x data root鞚?鞝勳泊 `pkvsyncd backup`鞚?氤搓磤頃╇媹雼?

旮办〈 0.x `metadata.db`鞐?1.0 binary 霕愲姅 Docker image毳?歆侅爲 鞐瓣舶頃橃 毵堨劯鞖?

## 1.0鞐愳劀 鞎堨爼頇旊悩電?surface

1.0攵€韯?雼れ潓 surface電?semantic versioning鞚?霐半雼堧嫟.

- `public-docs/openapi.yaml`鞐?氍胳劀頇旊悳 public REST routes.
- MCP how-to鞐?氍胳劀頇旊悳 MCP stdio 氚?Streamable HTTP tool behavior.
- 1.x fresh database鞖?SQLite migrations. 鞚错泟 1.x migrations電?鞚?v1 baseline 鞚错泟
  append-only鞛呺媹雼?
- vault氤?git repository layout瓿?content-addressed blob storage.
- CLI subcommands鞕€ 旮办〈 flags.
- Obsidian plugin settings鞕€ sync behavior. 鞚茧皹鞝侅澑 backward-compatible 1.x feature addition鞚€
  鞛堨潉 靾?鞛堨姷雼堧嫟.

OpenAPI鞐?氍胳劀頇旊悩歆€ 鞎婌潃 route, 鞓堧ゼ 霌れ柎 Admin Web UI form handler電?internal implementation
detail鞛呺媹雼?

## 甓岇灔 0.x to 1.0 鞝堨皑

1. 臧€電ロ晿氅?毹检爛 旮办〈 deployment毳?斓滌 0.8.x patch release搿?update頃橁碃, backup, materialize, export 欷€牍勳棎毵?靷毄頃╇媹雼?
2. `pkvsyncd backup --output <backup-dir>`毳?鞁ろ枆頃橁碃 瓴瓣臣毳?鞎堨爠頃橁矊 氤搓磤頃╇媹雼?
3. 臧?vault鞐?雽€頃?斓滌嫚 Obsidian client, `git clone`, 霕愲姅
   `pkvsyncd materialize <vault-id> --output <dir>`搿?順勳灛 file tree毳?毵岆摥雼堧嫟.
4. 旮办〈 server毳?欷戩頃╇媹雼?
5. 牍?`data_dir`鞕€ `metadata.db`搿?PKV Sync 1.0鞚?鞁滌瀾頃╇媹雼?
6. `/setup`鞚?鞕勲頃橁碃 user鞕€ vault毳?雼れ嫓 毵岆摖 霋? materialized vault contents毳?push 霕愲姅
   import頃╇媹雼?
7. user鞐愱矊 Obsidian plugin鞚?1.0.0鞙茧 update頃橂弰搿?鞎堧偞頃╇媹雼?

## Plugin compatibility

1.0 server鞐愳劀 supported plugin鞚€ server鞐?bundled霅?1.0 Obsidian plugin鞛呺媹雼? 鞓る灅霅?v0.8.x
plugin霃?core sync API電?臧欖毵? 靸堧鞖?靾橃爼瓿?self-update hardening鞚€ 1.0+鞐愳劀毵?鞙犾霅╇媹雼?

## 0.x鞐愳劀鞚?breaking changes

- migration鞚?雼澕 v1 baseline鞙茧 squash霅橃棃旮?霑岆鞐?0.x SQLite database電?in-place upgrade霅橃
  鞎婌姷雼堧嫟.
- first-run setup鞚€ browser-based毳?鞙犾頃╇媹雼? fresh server電?random admin password毳?log鞐?
  於滊牓頃橃 鞎婌姷雼堧嫟.

vault file contents, git history, blob鞚€ backup/materialize/recreate/import workflow搿?臧€鞝戈皥 靾?
鞛堨姷雼堧嫟.

## Known caveats

- native per-vault E2EE電?1.0 氩旍渼鞐?韽暔霅橃 鞎婌姷雼堧嫟. 歆€旮?client-side encrypted file contents臧€
  頃勳殧頃橁碃 plaintext path毳?氚涭晞霌れ澕 靾?鞛堧嫟氅?[`git-crypt`](./git-crypt-howto.ko.md)毳?靷毄頃橃劯鞖?
- `/metrics`電?default搿?disabled鞚措┌, 頇滌劚頇旐暣霃?production authentication gates臧€ 頃勳殧頃╇媹雼?
- production鞐愳劀電?`public_host`毳?靹れ爼頃橃劯鞖? configured HTTPS public origin鞚?瓴办爼頃?靾?鞐嗢溂氅?
  admin POST電?鞚橂弰鞝侅溂搿?fail-closed霅╇媹雼?
