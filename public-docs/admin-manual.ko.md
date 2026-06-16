# PKV Sync 甏€毽瀽 靹る獏靹?

[English](./admin-manual.md) | [绠€浣撲腑鏂嘳(./admin-manual.zh-CN.md) | [绻侀珨涓枃](./admin-manual.zh-Hant.md) | [鏃ユ湰瑾瀅(./admin-manual.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.5.

鞚?氍胳劀電?旮瓣硠 氩堨棴鞙茧 毵岆摖 齑堦赴 氩勳爠鞛呺媹雼? 瓿店皽 鞝勳棎 鞗愳柎氙?瓴€韱犽ゼ 甓岇灔頃╇媹雼?

鞚?靹る獏靹滊姅 鞛愳泊 順胳姢韺?PKV Sync 靹滊矂鞚?鞚检儊鞝侅澑 甏€毽ゼ 雼る９雼堧嫟. 雱ろ姼鞗岉伂鞕€ 順胳姢韸?臧曧檾電?氚绊彫 臧曧檾 臧€鞚措摐霃?頃粯 鞚届柎 欤检劯鞖?

## 斓滌磮 鞁ろ枆

1. 氚绊彫 韨るゼ 靸濎劚頃╇媹雼?

   ```bash
   pkvsyncd genkey
   ```

2. `config.example.toml`鞚?旮半皹鞙茧 `/etc/pkv-sync/config.toml`鞚?毵岆摥雼堧嫟.
3. 靸堧鞖?1.x 雿办澊韯?霐旊爥韯半Μ鞖?v1 雿办澊韯半矤鞚挫姢 baseline鞚?齑堦赴頇旐暕雼堧嫟.

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. 靹滊矂毳?鞁滌瀾頃╇媹雼?

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 靸?雿办澊韯半矤鞚挫姢毳?觳橃潓 鞁滌瀾頃?霋?敫岆澕鞖办爛鞐愳劀 `/setup`鞚?鞐搓碃 觳?甏€毽瀽 瓿勳爼鞚?毵岆摥雼堧嫟. PKV Sync電?鞛勳潣鞚?甏€毽瀽 牍勲皜氩堩樃毳?stderr 霕愲姅 旎厡鞚措剤 搿滉犯鞐?於滊牓頃橃 鞎婌姷雼堧嫟.
6. setup鞚?雭濍倻 霋?鞚茧皹 甏€毽瀽 搿滉犯鞚胳棎電?`/admin/login`鞚?靷毄頃╇媹雼?

PKV Sync 1.0鞚€ 雼澕 v1 SQLite baseline鞚?靷毄頃╇媹雼? 0.x鞐愳劀 毵岆摖 雿办澊韯半矤鞚挫姢電?1.0.0鞙茧 鞚疙攲霠堨澊鞀?upgrade頃?靾?鞐嗢姷雼堧嫟. [`upgrade-notes-v1.0.ko.md`](./upgrade-notes-v1.0.ko.md) 鞝堨皑毳?霐半ゴ靹胳殧. 鞚?v1 baseline 鞚错泟 瓴岇嫓霅橂姅 1.x migrations電?append-only鞛呺媹雼?

## Admin Web 韺剱

鞐搓赴:

```text
https://sync.example.com/admin/login
```

鞗?韺剱鞐愲姅 雼れ潓鞚?韽暔霅╇媹雼?

- 鞁滌姢韰? 鞀ろ啝毽, vault, 靷毄鞛? 斓滉芳 頇滊彊 歆€響滉皜 鞛堧姅 雽€鞁滊炒霌?
- 瓴€靸夑臣 靸來儨 頃勴劙臧€ 鞛堧姅 靷毄鞛?氇╇
- 牍勲皜氩堩樃 鞛劋鞝? 頇滌劚/甏€毽瀽 鞝滌柎, token 頇曥澑鞚?鞙勴暅 靷毄鞛?靸侅劯 韼橃澊歆€
- token鞚?雮橃棿, 靸濎劚, 觳犿殞頃橂姅 鞝勳棴 鞛レ箻 token 韼橃澊歆€
- 靻岇湢鞛? 韺岇澕 靾? 韥赴, 毵堨毵?霃欔赴頇? reconcile, 靷牅 鞛戩梾 氚?vault氤?霃欔赴頇?靹れ爼鞚?鞛堧姅 vault 旃措摐
- 韺岇澕 氙鸽Μ氤搓赴, 韺岇澕氤?旮半 韮€鞛勲澕鞚? unified diff 霠岆崝毵侅潉 歆€鞗愴晿電?鞚疥赴 鞝勳毄 vault 韺岇澕 敫岆澕鞖办爛
- 靹犿儩鞝?毵岆 鞁滉皠鞚?鞛堧姅 齑堧寑 靸濎劚, 頇滌劚 齑堧寑 氇╇, 靷毄頃橃 鞎婌潃 齑堧寑 靷牅
- General, Security, Sync & Storage, Network搿?氍鹅澑 霟绊儉鞛?靹れ爼. 鞐呺嵃鞚错姼 頇曥澑 on/off鞕€ 臧勱博霃?韽暔霅╇媹雼?
- 霃欔赴頇? vault 靾橂獏 欤缄赴, 鞚疥赴 鞝勳毄 韮愳儔 頄夓潉 鞁れ牅 靷毄鞛愳檧 鞛戩梾鞙茧 頃勴劙毵來晿電?頇滊彊 搿滉犯
- Blob 臧€牍勳 旎爥靺?韸鸽Μ瓯?
- 鞓侅柎, 欷戧淡鞏?臧勳泊, 欷戧淡鞏?氩堨泊, 鞚茧掣鞏? 頃滉淡鞏?鞏胳柎 鞝勴櫂

1.2.1鞐愳劀電?靷毄鞛?靸侅劯 韱店硠臧€ 鞁れ牅 vault 靾橃檧 毵堨毵?霃欔赴頇?鞁滉皝鞐?旮半皹頃橂┌, 旮瓣皠 霛茧波鞚€ 頃粯 鞝滉车霅橂姅 氇摖 admin 鞏胳柎鞐?毵炾矊 順勳頇旊惄雼堧嫟. reconciliation 氚?metadata repair 觳橂Μ霃?臧€電ロ暅 瓴届毎 歃濍秳 觳橂Μ 霕愲姅 batch 觳橂Μ毳?靷毄頃╇媹雼?

韮€鞛勳姢韮攧, 旮瓣皠, 氚旍澊韸?韥赴, 臧€霃?鞁滉皠, 頇滊彊 雿办澊韯半姅 靷瀸鞚?鞚疥赴 靿毚 順曥嫕鞙茧 響滌嫓霅╇媹雼? 旮半掣 鞁滉皠雽€電?`Asia/Shanghai`鞚措┌ 靹れ爼鞐愳劀 氤€瓴巾暊 靾?鞛堨姷雼堧嫟.

## 鞐呺嵃鞚错姼 鞎岆

PKV Sync電?旮半掣鞝侅溂搿?24鞁滉皠毵堧嫟 GitHub release毳?頇曥澑頃╇媹雼? 雿?靸?靹滊矂 release臧€ 鞛堨溂氅?雽€鞁滊炒霌滌棎 順勳灛 氩勳爠, 斓滌嫚 氩勳爠, release notes 毵來伂, 歆ъ潃 鞖旍暯鞚?鞛堧姅 氚半剤毳?響滌嫓頃╇媹雼?

`config.toml`鞚?`[update_check].enabled`鞕€ `[update_check].interval_seconds`電?靸?雿办澊韯半矤鞚挫姢鞚?觳?鞁滌瀾 霑?霟绊儉鞛?靹れ爼鞙茧 seed霅╇媹雼? 鞚错泟鞐愲姅 Admin WebUI Settings 韼橃澊歆€臧€ 鞖办劆頃╇媹雼? **Network** 靹轨厴鞐愳劀 鞐呺嵃鞚错姼 頇曥澑鞚?旒滉卑雮?雭勱碃 臧勱博鞚?氚旉縺 靾?鞛堨溂氅? 氚标犯霛检毚霌?鞛戩梾鞚€ 雼れ潓 欤缄赴鞐愳劀 靸?霟绊儉鞛?臧掛潉 雼れ嫓 鞚届姷雼堧嫟. 順勳灛 牍勴櫆靹?靸來儨霛茧┐ 雼れ嫓 旒?霋?鞎?60齑?鞎堨棎 氚橃榿霅╇媹雼? `[update_check].repo`電?鞐愳柎臧?mirror 氚绊彫毳?鞙勴暅 鞝曥爜 `config.toml` 頃勲摐搿?鞙犾霅╇媹雼?

```toml
[update_check]
enabled = false
interval_seconds = 86400
repo = "cyberkurry/pkv-sync"
```

鞐呺嵃鞚错姼 頇曥澑鞚€ 鞝曤炒 鞝滉车鞖╈瀰雼堧嫟. PKV Sync電?鞁ろ枆 欷戩澑 靹滊矂 氚旍澊雱堧Μ雮?旎厡鞚措剤 鞚措歆€毳?鞛愲彊鞙茧 甑愳泊頃橃 鞎婌姷雼堧嫟.

## 靷毄鞛?甏€毽?

- **Users** 霕愲姅 CLI鞐愳劀 靷毄鞛愲ゼ 毵岆摥雼堧嫟.
- 靷毄鞛?鞚措鞚€ 3-32鞛愳潣 ASCII 氍胳瀽, 靾瀽, `_`, `-`, `.`鞚挫柎鞎?頃╇媹雼?
- 甏€毽瀽臧€ 靸濎劚/鞛劋鞝曧晿電?牍勲皜氩堩樃, 瓿店皽 霌彪 牍勲皜氩堩樃, 靷毄鞛愱皜 歆侅爲 氤€瓴巾晿電?牍勲皜氩堩樃電?氇憪 12鞛?鞚挫儊鞚挫柎鞎?頃橂┌ 雽€氍胳瀽, 靻岆鞛? 靾瀽毳?韽暔頃挫暭 頃╇媹雼?
- Users 韼橃澊歆€鞚?瓴€靸夑臣 靸來儨 頃勴劙搿?響滊ゼ 膦來瀽 靾?鞛堨姷雼堧嫟.
- 靷毄鞛?靸侅劯 韼橃澊歆€鞐愳劀 牍勲皜氩堩樃毳?鞛劋鞝曧晿瓿? 瓿勳爼鞚?頇滌劚頇?霕愲姅 牍勴櫆靹表檾頃橁碃, 甏€毽瀽 甓岉暅鞚?鞀龟博 霕愲姅 臧曤摫頃橁碃, 頃措嫻 靷毄鞛愳潣 鞛レ箻 token鞚?頇曥澑頃?靾?鞛堨姷雼堧嫟.
- 雮橃鞐?臧愳偓 旮半鞚?頃勳殧頃?靾?鞛堨溂氅?靷毄鞛愲ゼ 靷牅頃橂姅 雽€鞁?牍勴櫆靹表檾頃橃劯鞖?
- Admin WebUI電?靷毄鞛愲ゼ 牍勴櫆靹表檾頃橁卑雮?甏€毽瀽毳?臧曤摫頃橁赴 鞝勳棎 頇曥澑 雽€頇旍儊鞛愲ゼ 響滌嫓頃╇媹雼? 鞛愳嫚鞚?甏€毽瀽 靹胳厴 牍勴櫆靹表檾鞕€ 毵堨毵?甏€毽瀽 臧曤摫鞚€ 彀嫧霅橂┌ 靷毄鞛?靸侅劯 韼橃澊歆€鞐?順勳頇旊悳 頂茧摐氚膘澊 響滌嫓霅╇媹雼?
- 雮晞 鞛堧姅 氇摖 甏€毽瀽 瓿勳爼鞚?牍勴櫆靹表檾頃橃 毵堨劯鞖?

Admin WebUI鞐愳劀 牍勲皜氩堩樃毳?鞛劋鞝曧晿氅?頃措嫻 靷毄鞛愳潣 旮办〈 鞛レ箻 token鞚?觳犿殞霅╇媹雼? 靷毄鞛愲姅 雼れ嫓 搿滉犯鞚疙暣鞎?頃╇媹雼?

CLI 雽€觳?氇呺牴:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 鞛レ箻 Token

鞛レ箻 bearer token鞚€ 鞚胳霅?靷毄 鞁?臧膘嫚霅橂┌ 90鞚?霃欖晥 靷毄頃橃 鞎婌溂氅?毵岆霅橁碃, 臧?token鞐愲姅 365鞚检潣 鞝堧寑 靾橂獏鞚?鞛堨姷雼堧嫟. 靷毄鞛愲姅 鞛愳嫚鞚?token鞚?觳犿殞頃?靾?鞛堦碃, 甏€毽瀽電?氇摖 靷毄鞛愳潣 token鞚?觳犿殞頃?靾?鞛堨姷雼堧嫟.

鞖挫榿 彀戈碃 靷暛:

- Token 韽夒鞚€ 靸濎劚 鞁?頃?氩堧 響滌嫓霅╇媹雼?
- 雿办澊韯半矤鞚挫姢鞐愲姅 SHA-256 token hash毵?鞝€鞛ル惄雼堧嫟.
- 甏€毽瀽 token 氇╇ endpoint鞕€ 響滊姅 瓿店皽 token 氅旐儉雿办澊韯半 響滌嫓頃橂┌, 韽夒 token鞚措倶 雮措秬 毵岆/觳犿殞 頃勲摐電?氚橅櫂頃橃 鞎婌姷雼堧嫟.
- 氇摖 鞚胳霅?鞖旍箔鞚€ token 毵岆 鞁滉皠鞚?頃措嫻 鞖旍箔 鞁滉皝鞙茧攵€韯?90鞚?霋る 鞐办灔頃橂悩 token 靸濎劚 頉?365鞚检潉 雱橃 鞎婌姷雼堧嫟.
- 臧欖潃 鞎堨爼鞝侅澑 頂岆煬攴胳澑 鞛レ箻 ID鞐愳劀 雼れ嫓 搿滉犯鞚疙晿氅?攴?鞛レ箻鞚?鞚挫爠 頇滌劚 token鞚?雽€觳措惄雼堧嫟.
- 頇滊彊 頄夓棎靹?彀胳“頃橂姅 觳犿殞霅?token鞚€ 頇滊彊 旮半鞚?氤挫〈頃?毂?鞝曤Μ頃?靾?鞛堨姷雼堧嫟.

## Vault

Admin WebUI鞐愳劀 vault毳?靷牅頃橂牑氅?於旉皜 頇曥澑 雽€頇旍儊鞛愱皜 頃勳殧頃╇媹雼? 彀胳“霅橃 鞎婋姅 blob鞚?garbage collection 鞝勱箤歆€ 雮潉 靾?鞛堧崝霛茧弰 靷牅電?韺岅创鞝?鞛戩梾鞙茧 旆笁頃橃劯鞖?

vault毳?靷牅頃橂┐ 雼れ潓鞚?鞝滉卑霅╇媹雼?

- vault 雿办澊韯半矤鞚挫姢 頄?
- 頃措嫻 頄夓棎靹?cascade霅橂姅 甏€霠?氅旐儉雿办澊韯?頄?
- `data_dir/vaults/<vault-id>` 鞎勲灅鞚?氚膘棓霌?bare Git 鞝€鞛レ唽
- 氅旊毽潣 vault氤?push 鞛犼笀

Blob 韺岇澕鞚€ 旖橅厫旄?欤检唽 歆€鞝?氚╈嫕鞚措┌, 臧€牍勳 旎爥靺橃澊 鞙犾槇 旮瓣皠鞚?歆€雮?彀胳“霅橃 鞎婌潓鞚?頇曥澑頃?霑岅箤歆€ 雮晞 鞛堨潉 靾?鞛堨姷雼堧嫟.

欷戨嫧霅?鞛戩梾 頉?韺岇澕 靾? 韥赴 霕愲姅 blob 彀胳“臧€ 鞛橂 氤挫澊氅?vault 氅旐儉雿办澊韯?reconciliation鞚?靷毄頃橃劯鞖? Reconciliation鞚€ tree entry鞐愳劀 blob pointer hash毳?歆侅爲 鞚疥碃 blob 彀胳“ 氤店惮毳?batch 觳橂Μ頃橂瘈搿?pointer 韺岇澕鞚?頃橂倶鞌?雼れ嫓 鞐?頃勳殧臧€ 鞐嗢姷雼堧嫟.

### Vault氤?霃欔赴頇?靹れ爼

**Vaults**鞐愳劀 vault 旃措摐鞚?**Settings**毳?鞐挫柎 vault氤?`extra_sync_globs` allowlist毳?韼胳頃╇媹雼? 鞚?靹れ爼鞚€ 靹犿儩霅?`.obsidian` 靹れ爼 韺岇澕鞚?韽暔頃?靾箑 瓴诫 欷?霃欔赴頇?臧€電ロ暅 頃鞚?鞝滌柎頃╇媹雼?

靸?vault電?甓岇灔 starter allowlist毳?鞛愲彊鞙茧 氚涭姷雼堧嫟. 旮办〈 vault電?甏€毽瀽 霕愲姅 vault 靻岇湢鞛愱皜 starter list毳?鞝侅毄頃?霑岅箤歆€ 牍勳柎 鞛堨姷雼堧嫟. **Apply starter allowlist** 鞛戩梾鞚€ 韰岆, CSS snippets, 雼稌韨? 鞎?頇橁步靹れ爼, 氇枒 頇橁步靹れ爼, 頇滌劚頇旊悳 頂岆煬攴胳澑 氇╇鞐?雽€頃?甓岇灔 氇╇鞚?鞌侂媹雼?

### 鞚疥赴 鞝勳毄 韺岇澕 旮半

**Vaults**鞐愳劀 vault 旃措摐鞚?**Browse files**毳?鞐诫媹雼? 敫岆澕鞖办爛電?順勳灛 HEAD 韺岇澕鞚?韥赴鞕€ 韰嶌姢韸?氚旍澊雱堧Μ 膦呺鞕€ 頃粯 雮橃棿頃╇媹雼? 韺岇澕鞚?鞐措┐ 韰嶌姢韸?韺岇澕鞚€ 鞚疥赴 鞝勳毄 氙鸽Μ氤搓赴毳?響滌嫓頃橁碃 **History** 氚?**Diff with previous** 毵來伂毳?鞝滉车頃╇媹雼?

旮半 韼橃澊歆€電?頃措嫻 韺岇澕鞚?commit鞚?雮橃棿頃橁碃, 臧?commit 鞁滌爯鞚?韺岇澕瓿?頃措嫻 diff搿?鞐瓣舶頃╇媹雼? diff 韼橃澊歆€電?unified diff 頄夓潉 於旉皜/靷牅/hunk 靸夓儊鞙茧 霠岆崝毵來暕雼堧嫟. 氚旍澊雱堧Μ 韺岇澕鞚€ 氅旐儉雿办澊韯半 響滌嫓頃橁碃 氚旍澊雱堧Μ diff 雮挫毄鞚€ 霠岆崝毵來晿歆€ 鞎婌姷雼堧嫟. 順勳灛 霃欔赴頇?頃勴劙鞐愳劀 瓯半秬霅?瓴诫電?韺岇澕 韮愳儔, commit list, history, diff 頇旊┐鞐愳劀 靾波歆戨媹雼?

韺岇澕, 旮半, diff 韮愳儔鞚€ `view_commit`, `view_history`, `view_diff` 頇滊彊 頄夓潉 旮半頃╇媹雼? Vault rollback controls電?Admin history鞐愳劀 靷毄頃?靾?鞛堨姷雼堧嫟. 雽€靸?commit鞚?頇曥澑頃?霋?靷毄頃橃劯鞖? rollback鞚€ 靹犿儩頃?旮半 歆€鞝愳棎靹?靸?vault 靸來儨毳?毵岆摥雼堧嫟.

## 齑堧寑鞕€ 霌彪

**Settings**鞐愳劀 霌彪鞚?靹れ爼頃╇媹雼?

- `disabled`: 甏€毽瀽毵?瓿勳爼鞚?毵岆摥雼堧嫟
- `invite_only`: 靷毄鞛愱皜 齑堧寑 旖旊摐搿?霌彪頃╇媹雼?
- `open`: 氚绊彫 URL鞚?臧€歆?雸勱惮雮?霌彪頃?靾?鞛堨姷雼堧嫟

齑堧寑 靸濎劚 鞁?靹犿儩鞝侅溂搿?氙鸽灅 毵岆 鞁滉皠鞚?歆€鞝曧暊 靾?鞛堨姷雼堧嫟. Admin WebUI電?靷瀸鞚?鞚诫姅 雮犾-鞁滉皠 鞛呺牓鞚?靷毄頃橁碃 雮措秬鞝侅溂搿?Unix 齑堧ゼ 鞝€鞛ロ暕雼堧嫟. 靷毄霅?齑堧寑電?admin API鞐愳劀 靷牅頃?靾?鞐嗢溂氅?臧愳偓 旮半鞙茧 氤搓磤頃橃劯鞖?

`open`鞚€ 歆ъ潃 鞁滉皠 彀?霕愲姅 於旉皜 氇媹韯半瓿?靻嶋弰 鞝滍暅鞚?鞛堧姅 瓿店皽 氚绊彫鞐愳劀毵?靷毄頃橃劯鞖?

## 霟绊儉鞛?靹れ爼

Settings 韼橃澊歆€電?SQLite鞐?鞝€鞛ル悳 臧掛潉 韼胳頃╇媹雼? 氤€瓴?靷暛鞚€ 靸?鞖旍箔鞐?歃夓嫓 鞝侅毄霅橂┌ 鞝€鞛?鞁?氅旊毽?旌愳嫓臧€ 臧膘嫚霅╇媹雼?

**General** 鈥?靹滊矂 鞚措, 旮半掣 鞁滉皠雽€, `enable_metrics` 氅旐姼毽?鞀れ渼旃? 頇滌劚頇旐晿氅?`/metrics`毳?靷毄頃?靾?鞛堨毵?氚绊彫 韨?middleware, 頂岆煬攴胳澑 User-Agent guard, 甏€毽瀽 bearer token鞚?瓿勳啀 頃勳殧頃╇媹雼?

**Security** 鈥?霌彪 氇摐(`disabled` / `invite_only` / `open`), 搿滉犯鞚?鞁ろ尐 鞛勱硠臧? 鞁ろ尐 彀? 鞛犼笀 旮瓣皠. 搿滉犯鞚?靻嶋弰 鞝滍暅旮半姅 鞁ろ尐 須熿垬鞕€ 歆勴枆 欷戩澑 牍勲皜氩堩樃 瓴€歃濎潉 氇憪 瓿勳偘頃橂瘈搿?霃欖嫓 於旍浮 韽＜搿?鞛勱硠臧掛潉 鞖绊殞頃?靾?鞐嗢姷雼堧嫟. 鞚胳霅?霃欔赴頇?API 瓴诫電?瓴诫, 氅旍劀霌? 韥措澕鞚挫柛韸?IP, bearer 鞛レ箻 token氤勲 60齑堧嫻 斓滊寑 600臧?鞖旍箔鞚?瓿犾爼 彀?鞝滍暅鞚?鞝侅毄頃╇媹雼? 鞁ろ尐頃?bearer token 鞚胳 鞁滊弰霃?韥措澕鞚挫柛韸?IP氤勲 60齑堧嫻 斓滊寑 120須岆 鞝滍暅霅橂瘈搿?臧€歆?token鞚?氚旉繑 臧€氅?鞁ろ尐 鞓堨偘鞚?鞖绊殞頃?靾?鞐嗢姷雼堧嫟.

**Sync & Storage**
- 斓滊寑 韺岇澕 韥赴(旮半掣臧?`100 MiB`). Blob upload request body電?鞚?runtime 靹れ爼鞚?雿?雴掛棳霃?頃儊 hard storage cap(頂勲雿曥厴 `512 MiB`)鞙茧 鞝滍暅霅╇媹雼?
- 歆€鞗愲悩電?韰嶌姢韸?頇曥灔鞛?鈥?氇╇ 氚栰潣 韺岇澕鞚€ 氚旍澊雱堧Μ blob鞙茧 觳橂Μ霅╇媹雼? 鞚?氇╇鞚€ Admin WebUI鞐愳劀 鞚疥赴 鞝勳毄鞙茧 響滌嫓霅╇媹雼? 氤€瓴届澊 頃勳殧頃橂┐ `text_extensions` 霟绊儉鞛?靹れ爼 頄?霕愲姅 SQLite `runtime_config` 韰岇澊敫旍潉 歆侅爲 韼胳)鞙茧 靾橃爼頃橃劯鞖?
- 於旉皜 exclude glob 鈥?雮挫灔 `.obsidian/`, `.trash/`, `.conflict-*`, `.git/` 鞝滌櫢 氇╇鞚?氤挫檮頃橂姅 甏€毽瀽 臁办爼 臧€電?韺劥
- 旮半 UI鞕€ diff 鞐旊摐韽澑韸?韱犼竴
- **Auto-merge text**(`enable_auto_merge`, 旮半掣 旒滌): 頇滌劚頇旐晿氅?靹滊矂電?conflict 韺岇澕鞚?鞊瓣赴 鞝勳棎 3-way 霛检澑 氤戫暕鞚?鞁滊弰頃╇媹雼? 瓴轨箻歆€ 鞎婋姅 韼胳鞚€ 旯旊仈頃橁矊 氤戫暕霅橂┌, 瓴轨箻電?韼胳鞚€ 鞐爠頌?merge 毵堨护臧€ 韽暔霅?conflict 韺岇澕鞚?毵岆摥雼堧嫟.
- **Push debounce**(`push_debounce_ms`, 旮半掣臧?`250`): 搿滌滑 韼胳鞚?鞎堨爼霅?霋?push頃橁赴 鞝勱箤歆€ 旮半嫟毽姅 鞁滉皠鞛呺媹雼? 雮稊氅?膦呺嫧 臧?歆€鞐办澊 欷勱碃, 雴掛澊氅?push雼?雿?毵庫潃 鞛呺牓鞚?氍鹅姷雼堧嫟
- **Inline SSE content cap**(`inline_content_max_bytes`, 旮半掣臧?`8192`, 斓滊寑 `65536`): 鞚?韥赴 鞚错晿鞚?韰嶌姢韸?氤€瓴届潃 SSE 鞚措菠韸?鞎堨棎 鞁る牑 靾橃嫚 頂岆煬攴胳澑鞚?氤勲弰 pull 鞐嗢澊 鞝侅毄頃?靾?鞛堨姷雼堧嫟. 雿?韥?韺岇澕鞚€ pull搿?雽€觳措惄雼堧嫟
- **SSE heartbeat**(`sse_heartbeat_seconds`, 旮半掣臧?`30`): 鞙犿湸 SSE 鞐瓣舶鞚?毽矂鞀?頂勲鞁滌棎靹?雭婈赴歆€ 鞎婋弰搿?頃橂姅 鞚措菠韸?鞀ろ姼毽?keep-alive鞛呺媹雼? 霃欖嫓 SSE 甑弲鞚€ 旮半掣鞝侅溂搿?靷毄鞛愲嫻 16臧滊 鞝滍暅霅橂┌ 鞝勳棴 靸來暅 1024毳?鞙犾頃╇媹雼? 鞐措牑 鞛堧姅 鞚措菠韸?鞀ろ姼毽检潃 bearer token鞚?欤缄赴鞝侅溂搿?鞛瞼歃濏晿氅? token 觳犿殞雮?瓿勳爼 牍勴櫆靹表檾 頉?雼灆雼堧嫟.
- **Git smart HTTP**(`enable_git_smart_http`, 旮半掣臧?旰检): 旒滊┐ 甓岉暅 鞛堧姅 鞛レ箻臧€ `git clone https://_:<token>@host/git/<vault-id>`毳?靷毄頃?靾?鞛堨姷雼堧嫟. 靹滊矂鞐愲姅 `PATH` 鞎堨潣 `git` 氚旍澊雱堧Μ霃?頃勳殧頃橂┌, 瓿店皽 `/api/config` capability電?霊?臁瓣贝鞚?氇憪 氚橃榿頃╇媹雼?

**Network and update checks** 鈥?`public_host`, bind address, trusted proxies, `[update_check].repo`電?鞁滌瀾 鞁?`config.toml`鞐愳劀 鞚届姷雼堧嫟. 鞐呺嵃鞚错姼 頇曥澑 頇滌劚 靸來儨鞕€ 臧勱博鞚€ SQLite鞐?鞝€鞛ル悩電?霟绊儉鞛?靹れ爼鞛呺媹雼? 項堨毄 氩旍渼電?60齑堧秬韯?30鞚缄箤歆€鞛呺媹雼?

## 頇滊彊

頇滊彊 搿滉犯電?push, pull, create_vault, delete_vault, view_commit, view_history, view_diff 臧欖潃 霃欔赴頇? vault 靾橂獏 欤缄赴, 鞚疥赴 鞝勳毄 韮愳儔 鞛戩梾鞚?旮半頃╇媹雼? 韽暔 頃:

- user
- vault
- action
- device name
- file count
- byte size
- client IP
- User-Agent
- details
- timestamp

頇滊彊 頃勴劙毳?靷毄頃?韸轨爼 靷毄鞛愲倶 鞛戩梾 鞙犿槙鞚?頇曥澑頃?靾?鞛堨姷雼堧嫟.

`create_vault`鞕€ `delete_vault`電?甏€毽?韺剱, 頂岆煬攴胳澑, API鞚?vault 靸濎劚/靷牅 鞛戩梾鞐愳劀 鞓惦媹雼?

## 靹滊矂 URL 瓿奠湢

靹滊矂 霕愲姅 Admin WebUI臧€ 於滊牓頃橂姅 URL鞚?瓿奠湢頃╇媹雼?

```text
https://sync.example.com/k_xxx/
```

氙缄皭頃?鞝曤炒搿?雼る（靹胳殧. 靷毄鞛?牍勲皜氩堩樃電?鞎勲媹歆€毵?氚绊彫 韨るゼ 韽暔頃橂┌ 頂岆煬攴胳澑 API 韸鸽灅頂届潣 觳?氩堨Ц 靷爠 鞚胳 甏€氍胳瀰雼堧嫟.

## 鞙犾氤挫垬 觳错伂毽姢韸?

- 鞖挫榿 鞀る儏靸缝棎電?`pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]`鞚?靷毄頃╇媹雼? 於滊牓 霐旊爥韯半Μ電?鞐嗞卑雮?牍勳柎 鞛堨柎鞎?頃╇媹雼? 氇呺牴鞚€ `VACUUM INTO`搿?SQLite毳?鞀る儏靸讽晿瓿? `vaults/`鞕€ `blobs/`毳?氤奠偓頃橂┌, pkvsyncd 氩勳爠, 旎错彫雱岉姼 hash, 韥赴, 臧滌垬臧€ 雼搓复 `MANIFEST.json`鞚?鞌侂媹雼? 旮半掣 氚膘梾鞚€ `config.toml`鞚?靸濍灥頃╇媹雼? 氚绊彫 韨れ檧 旮绊儉 搿滌滑 牍勲皜鞚?鞝€鞛ロ晿瓿?氤错樃頃橂牑電?瓴届毎鞐愲 `--include-config`毳?於旉皜頃橃劯鞖?
- `pkvsyncd restore --input <backup-dir> --data-dir <dir>`搿?鞐嗞卑雮?牍?雿办澊韯?霐旊爥韯半Μ鞐?氤奠洂頃╇媹雼? 雽€靸侅潉 毹检爛 牍勳泴霃?霅滊嫟電?瓴冹潉 頇曥澑頃?瓴届毎鞐愲 `--force`毳?於旉皜頃橃劯鞖? restore電?氤奠偓 鞝勳棎 manifest hash毳?頇曥澑頃橁碃 鞚错泟 verify毳?鞁ろ枆頃╇媹雼?
- 鞙犾氤挫垬 頉?霕愲姅 順胳姢韸?鞀ろ啝毽 靷碃 頉?`pkvsyncd verify [--data-dir <dir>]`毳?鞁ろ枆頃╇媹雼? 彀胳“霅?blob 韺岇澕鞚?瓴€靷晿瓿? 瓿犾晞 blob鞚?氤搓碃頃橂┌, `git2`搿?vault git 鞝€鞛レ唽毳?瓴€歃濏晿瓿? 雸勲澖, 靻愳儊, git 鞓る臧€ 鞛堨溂氅?0鞚?鞎勲媽 臧掛溂搿?膦呺頃╇媹雼? `--no-fail`鞚€ 氤搓碃靹滊ゼ 鞙犾頃橂悩 靹标车 膦呺 旖旊摐毳?臧曥牅頃╇媹雼?
- `pkvsyncd materialize <vault-id> -o <dir>`搿?vault鞚?HEAD毳?鞚茧皹 韺岇澕 韸鸽Μ搿?雮措炒雰呺媹雼?韰嶌姢韸?韺岇澕鞚€ 攴鸽寑搿? 氚旍澊雱堧Μ blob鞚€ blob store鞐愳劀 頃挫劃). 鞓ろ攧霛检澑 雮措炒雮搓赴, 鞛勳嫓 臧愳偓 霕愲姅 旖滊摐 毵堨澊攴鸽爤鞚挫厴鞐?鞙犾毄頃╇媹雼? 瓿缄卑 commit鞚?materialize頃橂牑氅?`--at <commit-sha>`鞕€ 頃粯 靷毄頃橃劯鞖?
- `[mcp].embed_in_serve = true`毳?靹れ爼頃橂┐ 氅旍澑 `pkvsyncd serve` 韽姼鞚?`/mcp`鞐愳劀 鞚疥赴/鞊瓣赴 MCP Streamable HTTP endpoint毳?雲胳稖頃╇媹雼? 霕愲姅 `pkvsyncd mcp --transport http --bind 127.0.0.1:6711`鞚?霃呺 MCP 頂勲靹胳姢搿?鞁ろ枆頃?靾?鞛堨姷雼堧嫟. 雼澕 vault stdio 靹胳厴鞚€ `pkvsyncd mcp --vault <id>`毳?靷毄頃橃劯鞖?
- 雽€霟?觳秬 靷牅 頉?blob 臧€牍勳 旎爥靺橃潉 鞁ろ枆頃╇媹雼?
- 搿滉犯鞕€ 頇滊彊鞐愳劀 氚橂车霅橂姅 `401`, `403`, `404`, `409`, `429` 鞚戨嫷鞚?頇曥澑頃╇媹雼?
- 靹滊矂 氚旍澊雱堧Μ, 頂岆煬攴胳澑 韺偆歆€, Docker 鞚措歆€, 毽矂鞀?頂勲鞁? 順胳姢韸?OS毳?斓滌嫚 靸來儨搿?鞙犾頃╇媹雼?
- release tag毳?毵岆摛旮?鞝勳棎 CI毳?頇曥澑頃╇媹雼?
- 臧?release鞐?Linux amd64, Linux arm64, Windows x64, 頂岆煬攴胳澑 zip, checksums, GHCR Docker 鞚措歆€ tag臧€ 韽暔霅橃柎 鞛堧姅歆€ 頇曥澑頃╇媹雼?
