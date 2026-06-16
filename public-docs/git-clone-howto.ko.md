# PKV vault毳?Git clone頃橁赴

[English](./git-clone-howto.md) | [绠€浣撲腑鏂嘳(./git-clone-howto.zh-CN.md) | [绻侀珨涓枃](./git-clone-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./git-clone-howto.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.3.

PKV Sync電?臧?vault毳?HTTPS毳?韱淀暅 read-only Git repository搿?雲胳稖頃?靾?鞛堨姷雼堧嫟.

## Prerequisites

- Server admin鞚?Sync & Storage settings鞐愳劀 鈥淕it smart HTTP鈥濍ゼ 頇滌劚頇旐枅鞀惦媹雼?
- Server鞐愳劀 `git` binary毳?靷毄頃?靾?鞛堨姷雼堧嫟.
- 鞙犿毃頃?device token鞚?鞛堨姷雼堧嫟.

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

旖滊 鞎烄潣 underscore電?username鞛呺媹雼? 鞏措枻 臧掛澊鞏措弰 霅╇媹雼? password 鞙勳箻鞚?token毵?靷毄霅╇媹雼?

### Example

server臧€ `sync.example.com`, vault ID臧€ `6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`, device token鞚?`pks_0f1e2d3c4b5a6978...`霛茧┐ 雼れ潓鞚?鞁ろ枆頃╇媹雼?

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID電?32鞛?靻岆鞛?hex鞛呺媹雼?雽€鞁?鞐嗢潓). Admin WebUI鞕€ `pkvsyncd user list`臧€ 鞙犿毃頃?ID毳?氤挫棳欷嶋媹雼? `abc123` 臧欖潃 placeholder電?`400 invalid_vault_id`搿?瓯半秬霅╇媹雼?

## Materialize

clone 頉勳棎電?PKV Sync server臧€ 韥?韺岇澕鞚?氤勲弰搿?鞝€鞛ロ晿旮?霑岆鞐?blob files臧€ pointer JSON鞙茧 氤挫瀰雼堧嫟. 雼れ潓鞚?鞁ろ枆頃╇媹雼?

```bash
pkvsyncd materialize <vault-id> -o ./output
```

pointer files毳?鞁れ牅 binary content搿?氚旉靖鞏?鞕勳爠頌?靷毄頃?靾?鞛堧姅 搿滌滑 vault copy毳?毵岆摥雼堧嫟.

## Notes

- HTTP毳?韱淀暅 repository電?**read-only**鞛呺媹雼? Git鞙茧 氤€瓴届偓頃潉 push頃?靾?鞐嗢姷雼堧嫟.
- 氤€瓴届潃 PKV Sync plugin鞐愳劀 靾橅枆頃橁碃 鞚茧皹 sync API搿?push頃橃劯鞖?
- Server admin鞚?Git smart HTTP毳?牍勴櫆靹表檾頃橂┐ clone 霕愲姅 fetch臧€ HTTP 503鞚?氚橅櫂頃╇媹雼?
