# Obsidian Sync鞐愳劀 毵堨澊攴鸽爤鞚挫厴

[English](./migrate-from-obsidian-sync.md) | [绠€浣撲腑鏂嘳(./migrate-from-obsidian-sync.zh-CN.md) | [绻侀珨涓枃](./migrate-from-obsidian-sync.zh-Hant.md) | [鏃ユ湰瑾瀅(./migrate-from-obsidian-sync.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.3.

鞚?氍胳劀電?旮瓣硠 氩堨棴鞙茧 毵岆摖 齑堦赴 氩勳爠鞛呺媹雼? 瓿店皽 鞝勳棎 鞗愳柎氙?瓴€韱犽ゼ 甓岇灔頃╇媹雼?

鞚?臧€鞚措摐電?鞚措 Obsidian Sync毳?靷毄頃橂姅 Obsidian vault鞚?順勳灛 韺岇澕鞚?靸?PKV Sync vault搿?臧€鞝胳槫電?氚╇矔鞚?靹る獏頃╇媹雼?

毵堨澊攴鸽爤鞚挫厴鞚€ 鞚?鞛レ箻鞐?順勳灛 臁挫灛頃橂姅 韺岇澕毵?臧€鞝胳樀雼堧嫟. Obsidian Sync 旮半, 鞗愱博 氩勳爠 旮半, 靷牅霅?韺岇澕 旮半, 於╇弻 氅旐儉雿办澊韯半姅 臧€鞝胳槫歆€ 鞎婌姷雼堧嫟. PKV Sync 旮半鞚€ 靸?PKV vault毳?毵岆摐電?毵堨澊攴鸽爤鞚挫厴 commit鞐愳劀 鞁滌瀾霅╇媹雼?

毵堨澊攴鸽爤鞚挫厴鞚€ Obsidian Sync毳?牍勴櫆靹表檾頃橁卑雮?鞝滉卑頃橁卑雮?氤€瓴巾晿歆€ 鞎婌姷雼堧嫟. PKV Sync 瓴瓣臣毳?頇曥澑頃?霋?Obsidian Sync 靷毄鞚?欷戩頃橂牑氅?Obsidian鞐愳劀 靾橂彊鞙茧 雭勳劯鞖?

## 鞁滌瀾頃橁赴 鞝勳棎

- 毵堨澊攴鸽爤鞚挫厴鞐?靷毄頃?鞛レ箻鞐愳劀 Obsidian Sync 霃欔赴頇旉皜 雭濍偁 霑岅箤歆€ 旮半嫟毽诫媹雼?
- 毵堨澊攴鸽爤鞚挫厴 鞝勳棎 vault 韽措崝毳?靾橂彊鞙茧 氚膘梾頃╇媹雼?
- 臧€電ロ晿氅?臧€鞝胳槫電?霃欖晥 Obsidian鞚?雼晞 霊愱卑雮? 鞝侅柎霃?韺岇澕 韼胳鞚?頂柬暕雼堧嫟.
- 雽€靸?PKV Sync 靹滊矂 瓿勳爼鞚?毹检爛 毵岆摛瓯半倶 頇曥澑頃╇媹雼?

## 臧€鞝胳槫電?頃

PKV Sync電?靸?vault毳?毵岆摛瓿?順勳灛 臧€鞝胳槫旮?雮挫毄鞚?觳?PKV 旮半 頃鞙茧 commit頃╇媹雼?

鞚茧皹 Markdown 韺岇澕, 觳秬 韺岇澕, 鞚茧皹 vault 韺岇澕鞚€ PKV Sync鞚?臧曥牅 鞝滌櫢 攴滌箼鞐?瓯鸽Μ歆€ 鞎婋姅 頃?臧€鞝胳樀雼堧嫟.

## 瓯措剤霙半姅 頃

臧€鞝胳槫旮?霃勱惮電?Obsidian Sync 雮措秬 韺岇澕, PKV Sync plugin 鞛愳泊 靸來儨, OS 攵€靷半 韺岇澕, 搿滌滑 霟绊儉鞛?韺岇澕鞚?瓯措剤霚侂媹雼? 鞓?

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/` (plugin 鞛愳泊 靹れ爼瓿?token 鞝€鞛レ唽電?搿滌滑鞐愲 氤搓磤)
- `.trash/**`
- `.git/**`
- `.DS_Store` (macOS)
- `Thumbs.db` (Windows)
- `*.tmp`, `*.lock` 臧欖潃 鞛勳嫓 韺岇澕
- 鞛レ箻氤?workspace, cache, trash, 鞛勳嫓 韺岇澕

靹犿儩頃?`.obsidian` 靹れ爼 韺岇澕鞚€ 雮橃鞐?vault氤?`.obsidian` allowlist搿?霃欔赴頇旐暊 靾?鞛堨姷雼堧嫟. 鞛愳劯頃?攴滌箼鞚€ `.obsidian` 靹れ爼 霃欔赴頇?臧€鞚措摐毳?彀戈碃頃橃劯鞖?

## 毵堨澊攴鸽爤鞚挫厴 頉?

雼るジ 鞛レ箻鞐愳劀 靸?PKV vault毳?鞐搓碃 雲疙姼鞕€ 觳秬 韺岇澕鞚?鞓皵毳搓矊 氤挫澊電旍 頇曥澑頃╇媹雼? 頇曥澑鞚?雭濍偁 霑岅箤歆€ 靾橂彊 氚膘梾鞚?氤搓磤頃橃劯鞖?

Obsidian Sync鞕€ PKV Sync毳?臧欖潃 韽措崝鞐愳劀 瓿勳啀 鞁ろ枆頃滊嫟氅?氤€瓴?鞛戩梾鞚?鞁犾頃橁矊 頃橃劯鞖? 霊?霃欔赴頇?鞁滌姢韰滌澊 臧欖潃 韺岇澕鞐愳劀 於╇弻頃?靾?鞛堨溂氅? PKV Sync電?毵堨澊攴鸽爤鞚挫厴 commit 鞚错泟 氚涭潃 氤€瓴诫 旮半頃╇媹雼?
