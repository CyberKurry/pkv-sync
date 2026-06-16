# PKV Sync鞐愳劀 git-crypt 靷毄頃橁赴

[English](./git-crypt-howto.md) | [绠€浣撲腑鏂嘳(./git-crypt-howto.zh-CN.md) | [绻侀珨涓枃](./git-crypt-howto.zh-Hant.md) | [鏃ユ湰瑾瀅(./git-crypt-howto.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.3.

> **Note:** native end-to-end encryption(E2EE)鞚?鞝滉车霅橁赴 鞝勱箤歆€鞚?鞛勳嫓 臧€鞚措摐鞛呺媹雼? PKV Sync server電?鞐爠頌?filenames鞕€ commit metadata毳?氤?靾?鞛堨姷雼堧嫟.

## Overview

[git-crypt](https://github.com/AGWA/git-crypt)電?Git repository 鞎堨棎靹?transparent file encryption鞚?鞝滉车頃╇媹雼? PKV Sync電?vault毳?Git repository搿?雲胳稖頃?靾?鞛堨溂氙€搿? 氙缄皭頃?韺岇澕鞚?server鞐?霃勲嫭頃橁赴 鞝勳棎 git-crypt搿?鞎旐樃頇旐暊 靾?鞛堨姷雼堧嫟.

## Setup

### 1. git-crypt 靹れ箻

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. clone頃?vault鞐愳劀 git-crypt 齑堦赴頇?

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 鞎旐樃頇旐暊 韺岇澕 靹れ爼

`.gitattributes`毳?毵岆摛瓯半倶 韼胳頃╇媹雼?

```gitattributes
# 旮半掣鞙茧 氇摖 韺岇澕 鞎旐樃頇?
* filter=git-crypt diff=git-crypt

# 雼?.gitattributes 鞛愳泊電?鞎旐樃頇旐晿歆€ 鞎婌潓
.gitattributes !filter !diff
```

靹犿儩鞝?鞎旐樃頇旊ゼ 甓岇灔頃╇媹雼?

```gitattributes
# 韸轨爼 pattern毵?鞎旐樃頇?
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. collaborator鞕€ key 瓿奠湢

symmetric key毳?export頃╇媹雼?

```bash
git-crypt export-key ../vault-key
```

臧?collaborator臧€ import頃╇媹雼?

```bash
git-crypt unlock ../vault-key
```

## Limitations

- **Filenames電?鞎旐樃頇旊悩歆€ 鞎婌姷雼堧嫟.** PKV Sync server電?file paths鞕€ directory structure毳?氤?靾?鞛堨姷雼堧嫟.
- **git-crypt電?Git client 飒届棎靹?霃欖瀾頃╇媹雼?** Server電?ciphertext blobs毳?鞝€鞛ロ暕雼堧嫟. key 鞐嗢澊 clone頃橂┐ encrypted files電?攵堩埇氇呿暅 binary data搿?氤挫瀰雼堧嫟.
- **Key management電?靾橂彊鞛呺媹雼?** key毳?鞛冹溂氅?encrypted files毳?氤店惮頃?靾?鞐嗢姷雼堧嫟.
- **Git clone workflow鞐愳劀毵?霃欖瀾頃╇媹雼?** PKV Sync Obsidian plugin鞚€ git-crypt毳?鞚错暣頃橃 鞎婌姷雼堧嫟. encrypted files電?vault毳?clone頃橁碃 Git鞙茧 歆侅爲 雼る鞎?頃╇媹雼?
- **`pkvsyncd materialize`電?git-crypt毳?鞚胳嫕頃橃 鞎婌姷雼堧嫟.** PKV Sync臧€ `pkvsync_pointer` JSON鞙茧 鞝€鞛ロ暅 韺岇澕(欤茧 text-extension list氤措嫟 韥?binaries)鞚€ materialize 鞁?靹滊矂鞚?blob store鞐愳劀 resolve霅橃柎 raw bytes搿?霃勳癌頃╇媹雼? git-crypt鞚?filter電?韥措澕鞚挫柛韸?飒届棎靹?鞚措ゼ 氤挫 氇豁晿氙€搿? `*.pdf`雮?攴?鞕?blob鞐?鞝€鞛ル悩電?頇曥灔鞛愲ゼ git-crypt搿?鞎旐樃頇旐暣霃?旮半寑頃?ciphertext stream鞚?毵岆摛鞏挫歆€ 鞎婌姷雼堧嫟. git-crypt patterns電?PKV Sync臧€ text搿?旆笁頃橂姅 韺岇澕 順曥嫕(server鞐愳劀 靹れ爼頃橂姅 `text_extensions` list, 旮半掣臧? `md`, `canvas`, `base`, `json`, `txt`, `css`)鞙茧 鞝滍暅頃橃劯鞖?

## Recommended Workflow

1. 鞚检儊 note-taking鞐愲姅 Obsidian plugin瓿?鞎旐樃頇旐晿歆€ 鞎婌潃 韺岇澕鞚?靷毄頃╇媹雼?
2. E2EE臧€ 頃勳殧頃?氙缄皭頃?韺岇澕鞐愲姅 Git clone瓿?git-crypt毳?靷毄頃╇媹雼?
3. git-crypt key毳?鞎堨爠頃橁矊 backup頃╇媹雼?
