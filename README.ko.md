# PKV Sync

**Obsidian 氤柬姼毳?歆侅爲 順胳姢韺呿晿靹胳殧.** PKV Sync電?鞛愳泊 靹滊矂鞐愳劀 霃欖瀾頃橂┌,
Obsidian 氤柬姼毳?頊措寑韽? 韮滊笖毽? 雿办姢韥啽 靷澊鞐愳劀 霃欔赴頇旐暕雼堧嫟. 氚旍澊雱堧Μ
頃橂倶, SQLite 雿办澊韯半矤鞚挫姢 頃橂倶, 氤柬姼毵堧嫟 bare git 鞝€鞛レ唽 頃橂倶臧€ 鞝勲秬鞛呺媹雼?
韥措煬鞀ろ劙霃? S3霃? 毵る媹歆€霌?韥措澕鞖半摐霃?鞐嗢姷雼堧嫟. 靹れ箻頃橁碃, Obsidian鞐愳劀
臧€毽偆霃勲 靹れ爼頃橂┐ 雲疙姼臧€ 霃欔赴頇旊惄雼堧嫟.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

氍胳劀 氩勳爠: v1.4.3.

[English](./README.md) | [绠€浣撲腑鏂嘳(./README.zh-CN.md) | [绻侀珨涓枃](./README.zh-Hant.md) | [鏃ユ湰瑾瀅(./README.ja.md) | 頃滉淡鞏?

## 旮半姤

- **雼れ 靷毄鞛? 雼れ 氤柬姼** 霃欔赴頇旊ゼ 鞚胳霅?旮瓣赴 靷澊鞐愳劀 歆€鞗愴晿氅?
  氤柬姼氤?push lock 瓿?氅彪摫 鞛嫓霃勲ゼ 鞝滉车頃╇媹雼?
- **鞁れ嫓臧?push.** 鞛戩潃 韼胳鞚€ Server-Sent Events 搿?1 齑?鞚措偞鞐?霃勳癌頃╇媹雼?
  韽措鞚€ 鞎堨爠毵濎溂搿?雮晞 鞛堨姷雼堧嫟.
- **Git 鞚?雼澕 source of truth.** 氇摖 氤柬姼電?bare git 鞝€鞛レ唽鞚措瘈搿?
  韺岇澕氤?鞚措牓, unified diff, 雼澕 韺岇澕 氤奠洂鞚?頂岆煬攴胳澑瓿?鞏措摐氙?韺剱鞐愳劀
  氚旊 霃欖瀾頃╇媹雼?
- **於╇弻 鞎堨爠.** 頂岆煬攴胳澑鞚€ 搿滌滑 韼胳鞚?臁办毄頌?雿柎鞊办 鞎婌姷雼堧嫟.
  於╇弻鞚€ `.conflict-*` 韺岇澕搿?雲胳稖霅橁碃 頃?氩堨潣 韥措Ν鞙茧 頃搓舶頃?靾?鞛堨姷雼堧嫟.
- **鞏措摐氙?韺剱**鞚€ 5 臧?鞏胳柎(English, 绠€涓? 绻佷腑, 鏃ユ湰瑾? 頃滉淡鞏?搿?
  靷毄鞛? 旮瓣赴 韱犿伆, 氤柬姼, 齑堧寑, 頇滊彊, 敫旊… GC 毳?甏€毽晿氅?韺岅创鞝侅澑 氤柬姼鞕€
  靷毄鞛?鞛戩梾鞐愲姅 頇曥澑 雽€頇旍儊鞛愲ゼ 響滌嫓頃╇媹雼?
- **AI臧€ 鞚届潉 靾?鞛堧姅 vault.** MCP電?stdio, 霃呺 Streamable HTTP, 霕愲姅 `pkvsyncd serve`鞐?雮挫灔霅?`/mcp` 霛检毎韸鸽 鞚疥赴/鞊瓣赴 霃勱惮毳?雲胳稖頃╇媹雼?
- **旮半掣鞝侅溂搿?瓴疥硠臧€ 鞛堨姷雼堧嫟.** 甏€毽瀽臧€ 靸濎劚锛忟灛靹れ爼頃橂姅 牍勲皜氩堩樃電?setup瓿?臧欖潃 臧曧暅 鞝曥眳鞚?靷毄頃橁碃, token 韽夒鞚€ 頃?氩堧 響滌嫓霅橂┌, upload鞕€ MCP response鞐愲姅 韥赴 靸來暅鞚?鞛堦碃, live SSE stream鞚€ 觳犿殞霅?token鞚?鞛瞼歃濏暕雼堧嫟.
- **鞚橂弰鞝侅溂搿?雼垳頃╇媹雼?** 氚旍澊雱堧Μ 頃橂倶, SQLite 氅旐儉雿办澊韯?DB 頃橂倶,
  氤柬姼毵堧嫟 bare git 鞝€鞛レ唽 頃橂倶, 觳秬 韺岇澕毵堧嫟 content-addressed 敫旊… 頃橂倶.

## Docker Compose 搿?牍犽ゴ瓴?鞁滌瀾

甓岇灔 瓴诫鞛呺媹雼? `deploy/caddy/` 鞚?Caddy 臧€ Let's Encrypt 搿?HTTPS 毳?
觳橂Μ頃橁碃, PKV Sync 電?compose 雱ろ姼鞗岉伂 鞎?`127.0.0.1:6710` 鞐?毹鸽毳措┌
瓿奠毄 鞚疙劙雱缝潣 韽夒 HTTP 毳?歆侅爲 氚涭 鞎婌姷雼堧嫟.

霃勲鞚?鞚措(鞓? `sync.example.com`)鞚?頃勳殧頃╇媹雼? A锛廇AAA 霠堨綌霌滉皜 靹滊矂毳?
臧€毽紲鞎?頃橁碃, 鞚疙劙雱缝棎靹?`80` 瓿?`443` 韽姼鞐?鞝戧芳頃?靾?鞛堨柎鞎?頃╇媹雼?
(韽姼 80 鞚€ ACME HTTP-01 瓴€歃濎棎 靷毄霅╇媹雼?.

1. 氚绊彫 韨るゼ 靸濎劚頃╇媹雼?

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. `docker-compose.yml` 鞓嗢棎 `config.toml` 鞚?霊‰媹雼?

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # genkey 於滊牓鞙茧 氚旉靖靹胳殧
   public_host    = "sync.example.com"   # 頃勳垬, 鞏措摐氙?POST 臧€ 霃欖瀾頃橂牑氅?頃勳殧

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge network

   [mcp]
   embed_in_serve = false                # true鞚措┐ 鞚?靹滊矂鞐?/mcp毳?毵堨毚韸?
   ```

3. `deploy/caddy/Caddyfile` 鞚?韼胳頃?`sync.example.com` 鞚?鞁れ牅
   霃勲鞚胳溂搿?氚旉繅雼堧嫟.

4. 鞀ろ儩鞚?霛勳泚雼堧嫟.

   ```bash
   docker compose up -d
   ```

   敫岆澕鞖办爛鞐愳劀 `https://sync.example.com/setup` 鞚?鞐搓碃 觳?甏€毽瀽
   瓿勳爼鞚?毵岆摥雼堧嫟.

5. `pkv-sync-plugin.zip` 鞚?Obsidian 鞐?靹れ箻頃╇媹雼?
   (`<vault>/.obsidian/plugins/pkv-sync/`). 頇滌劚頇旐晿瓿?鞏措摐氙?韺剱鞚?
   share URL 鞚?攵欖棳 雱ｌ潃 霋? 搿滉犯鞚?霕愲姅 臧€鞛呿晿瓿?氤柬姼毳?瓿犽雼堧嫟.

鞐呺嵃鞚错姼電?`docker compose pull && docker compose up -d` 鞛呺媹雼? 雱れ澊韹半笇
靹れ箻, 毽矂鞀?頂勲鞁?韸滊嫕(Caddy锛廚ginx锛廡raefik), `public_host` 鞚橂,
氚膘梾锛忞车鞗? 霐旍姢韥?鞎旐樃頇旊姅
[氚绊彫 臧曧檾 臧€鞚措摐](./public-docs/deployment-hardening.ko.md)毳?
彀戈碃頃橃劯鞖?

## MCP 氚绊彫 氇摐

PKV Sync電?MCP Streamable HTTP transport毳?霊?臧€歆€ 氚╈嫕鞙茧 雲胳稖頃╇媹雼? 雮挫灔
氇摐電?氇呾嫓鞝侅溂搿?旒媹雼? `[mcp].embed_in_serve = true`毳?靹れ爼頃橂┐ `pkvsyncd
serve`臧€ 氅旍澑 靹滊矂 韽姼鞐?`/mcp`毳?毵堨毚韸疙晿瓿?臧欖潃 TLS 膦呺, 毽矂鞀?頂勲鞁?
氚绊彫 韨? bearer token 瓴€歃濎潉 瓿奠湢頃╇媹雼? 霃呺 氇摐電?旮办〈觳橂熂 氤勲弰 頂勲靹胳姢鞛呺媹雼?
`pkvsyncd mcp --transport http --bind 127.0.0.1:6711`. MCP 瓴╇Μ, 鞝勳毄 bind
address, 霕愲姅 霃呺 scaling鞚?頃勳殧頃?霑?鞙犾毄頃╇媹雼?

## Obsidian 頂岆煬攴胳澑

搿滌滑 韺岇澕鞚?source of truth 鞛呺媹雼? 頂岆煬攴胳澑鞚€ 霐旍姢韥?鞙勳潣 韽夒矓頃?
Obsidian 氤柬姼毳?鞚疥碃 鞊半┌, 頂勲鞁?韺岇澕鞁滌姢韰滌潉 毵岆摛歆€ 鞎婌姷雼堧嫟. 頂岆煬攴胳澑
氙缄皭頃橃 鞎婌潃 靹れ爼瓿?霃欔赴頇?鞚鸽嵄鞀る姅
`<vault>/.obsidian/plugins/pkv-sync/data.json` 鞐?鞝€鞛ル惄雼堧嫟. 搿滉犯鞚?靸來儨,
頇滌劚 bearer 旮瓣赴 韱犿伆, 氚绊彫 韨? 鞎堨爼鞝侅澑 旮瓣赴 ID電?Obsidian鞚?旮瓣赴 搿滌滑
鞝€鞛レ唽鞐?鞝€鞛ル惄雼堧嫟. Obsidian 旮瓣赴 搿滌滑 鞝€鞛レ唽, 韽夒 氚膘梾, 鞚挫爠 氩勳爠鞐愳劀 雮潃
頂岆煬攴胳澑 `data.json` 靷掣鞚€ 氙缄皭 鞝曤炒搿?旆笁頃橃劯鞖? 旮瓣赴 韱犿伆鞚€ 靷毄頃?霑岆雼?
臧膘嫚霅橁碃 90 鞚?霃欖晥 牍勴櫆靹?靸來儨鞚措┐ 毵岆霅橂┌, 臧?韱犿伆鞐愲姅 365 鞚检潣 鞝堧寑 靾橂獏鞚?
鞛堨姷雼堧嫟. 臧欖潃 旮瓣赴鞐愳劀 雼れ嫓 搿滉犯鞚疙晿氅?頇滌劚 韱犿伆鞚?甑愳泊霅╇媹雼?

氇呺牴 韺旊爤韸? 韺岇澕 鞚措牓, 膦岇毎 diff, 於╇弻 頃搓舶, 靹犿儩鞝?`.obsidian` 霃欔赴頇?
旮瓣赴 甏€毽? 鞛愱皜 鞐呺嵃鞚错姼 臧欖潃 鞚检儊 旮半姤鞚€
[靷毄鞛?毵る壌鞏糫(./public-docs/user-manual.ko.md)鞐愳劀 鞎堧偞頃╇媹雼?

## 順勳灛 鞁滌爯鞚?鞎旐樃頇?

PKV Sync 1.0 鞚€ 鞎勳 native end-to-end encryption 鞚?鞝滉车頃橃 **鞎婌姷雼堧嫟**.
靹滊矂電?氤柬姼 雮挫毄鞚?鞚届潉 靾?鞛堨姷雼堧嫟. 氤柬姼氤?native E2EE 電?1.x 搿滊摐毵奠棎
opt-in 氇摐搿?瓿勴殟霅橃柎 鞛堨姷雼堧嫟. 鞎旐樃頇旊姅 Git-native PKV 毳?鞊鸽 鞛堦矊
毵岆摐電?靹滊矂 旄?旮半姤(鞚措牓 diff, 3-way 鞛愲彊 氤戫暕, 鞚鸽澕鞚?SSE payload, MCP
鞚疥赴锛忟摪旮?瓿?trade-off 臧€ 鞛堦赴 霑岆鞛呺媹雼?

E2EE 臧€ 霃勳瀰霅橁赴 鞝勳棎 頃勳殧頃橂嫟氅? 氤柬姼鞐?
[`git-crypt`](https://github.com/AGWA/git-crypt) 鞚?鞏轨溂靹胳殧. 響滌嫓霅?
瓴诫電?靹滊矂臧€ 氤淀樃頇旐暊 靾?鞐嗠姅 ciphertext 敫旊…鞙茧 霃勲嫭頃╇媹雼? 韺岇澕
鞚措鞚€ 靹滊矂鞐?韽夒鞙茧 雮姷雼堧嫟(雽€攵€攵勳潣 鞙勴槕 氇嵏鞐愳劀 靾橃毄 臧€電ロ暕雼堧嫟).
響滌 `git clone` 瓿?`pkvsyncd materialize` 電?韨るゼ 臧€歆?韥措澕鞚挫柛韸胳棎靹?
瓿勳啀 霃欖瀾頃╇媹雼?

鞁れ牅 氚绊彫鞐愳劀電?HTTPS 霋れ棎靹?鞁ろ枆頃橁碃, `trusted_proxies` 毳?鞝滍暅頃橂┌,
雿办澊韯?霐旍姢韥檧 氚膘梾鞚?鞎旐樃頇旐晿靹胳殧. 鞛愳劯頃?雮挫毄鞚€
[氚绊彫 臧曧檾 臧€鞚措摐](./public-docs/deployment-hardening.ko.md)毳?
彀戈碃頃橃劯鞖?

## 彀娟碃 瓿勳嫚 瓯粹€?

| 欤检牅 | 氍胳劀 |
| --- | --- |
| 鞚检儊鞝侅澑 頂岆煬攴胳澑 靷毄 | [靷毄鞛?毵る壌鞏糫(./public-docs/user-manual.ko.md) |
| 靹滊矂 鞖挫榿瓿?霟绊儉鞛?靹れ爼 | [甏€毽瀽 毵る壌鞏糫(./public-docs/admin-manual.ko.md) |
| 氇摖 CLI 氇呺牴瓿?頂岆灅攴?| [CLI 霠堩嵓霟办姢](./public-docs/cli-reference.ko.md) |
| 0.x 鞐愳劀 1.0 鞙茧 鞐呹犯霠堨澊霌?| [1.0 鞐呹犯霠堨澊霌?雲疙姼](./public-docs/upgrade-notes-v1.0.ko.md) |
| 毽矂鞀?頂勲鞁? TLS, 氚膘梾, 頃橂摐雼?| [氚绊彫 臧曧檾](./public-docs/deployment-hardening.ko.md) |
| HTTP API 瓿勳暯 | [OpenAPI 氇呾劯](./public-docs/openapi.yaml) |
| MCP 靹れ爼瓿?霃勱惮 氇╇ | [MCP 靷毄氩昡(./public-docs/mcp-howto.ko.md) |
| LLM鞚?鞙犾甏€毽晿電?Wiki workflow | [LLM Wiki 靷毄氩昡(./public-docs/llm-wiki-howto.ko.md) |
| Obsidian Sync 鞐愳劀 鞚挫爠 | [鞚挫爠 臧€鞚措摐](./public-docs/migrate-from-obsidian-sync.ko.md) |
| 氤挫晥 鞝滊炒 | [SECURITY.md](./SECURITY.md) |
| 毽措Μ鞀?鞚措牓 | [CHANGELOG.md](./CHANGELOG.md) |

## 靸來儨

PKV Sync 1.4.3鞚€ 臧愳偓 靾橃爼鞚?鞐办啀鞙茧, 鞝曧檿靹膘棎 欷戩爯鞚?霊‰媹雼? 臧愳嫓 氚标犯霛检毚霌?鞛戩梾鞚?鞝曥儊 膦呺 鞁?欷戩霅橁碃, 鞓る灅霅?DashMap 頃鞚?欤缄赴鞝侅溂搿?鞝曤Μ霅橂┌, 鞛愲彊 氤戫暕鞚?雸勲澖 臧濎泊鞕€ Git 鞚检嫓 鞓る毳?鞓皵毳搓矊 甑秳頃橁碃, 氅彪摫 旌愳嫓臧€ 氅旐儉雿办澊韯?韸鸽灉鞛厴 鞁ろ尐 頉?氚橂摐鞁?旮半霅橂┌, 霃欖嫓 韰嶌姢韸?靸濎劚鞚?於╇弻 韺岇澕 鞀龟博鞙茧 氤挫〈霅╇媹雼? 攵堩晞鞖?旖旊摐(氙胳偓鞖?項嵓, i18n 韨? Docker 霠堨澊鞏?霃?鞝曤Μ霅橃棃鞀惦媹雼?

PKV Sync 1.0 鞚€ 觳?鞎堨爼 毽措Μ鞀れ瀰雼堧嫟. 瓿店皽 REST API, CLI 響滊┐, 鞝€鞛レ唽
霠堨澊鞎勳泝, 頂岆煬攴胳澑 韺偆歆€, Docker 鞚措歆€電?臧欖潃 semver 搿?甏€毽惄雼堧嫟.
1.X.Y 電?瓿店皽 響滊┐鞐愳劀 頃橃渼 順疙櫂鞚?鞙犾頃橁碃, OpenAPI 氇呾劯臧€ 鞝曥嫕 順疙櫂靹?
瓿勳暯鞛呺媹雼? 0.x 毽措Μ鞀る 毵岆摖 SQLite 雿办澊韯半矤鞚挫姢電?1.0.0 鞙茧 in-place
鞐呹犯霠堨澊霌滍暊 靾?鞐嗢姷雼堧嫟.
[1.0 鞐呹犯霠堨澊霌?雲疙姼](./public-docs/upgrade-notes-v1.0.ko.md)毳?霐半ゴ靹胳殧.

GitHub 毽措Μ鞀る雼?Linux amd64锛廰rm64 氚旍澊雱堧Μ, Windows x64 氚旍澊雱堧Μ,
氅€韹?鞎勴偆韰嶌矘 GHCR Docker 鞚措歆€, Obsidian 頂岆煬攴胳澑 zip, `SHA256SUMS` 臧€
頃粯 瓴岇嫓霅╇媹雼?

## 臧滊皽 觳错伂

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 電?Linux 鞕€ Windows 鞐愳劀 鞝勳泊 Rust 毵ろ姼毽姢, 頂岆煬攴胳澑
test锛弔ypecheck锛廱uild锛弍ackage, Docker 牍岆摐, 毽措Μ鞀?氚旍澊雱堧Μ
smoke 韰岇姢韸鸽ゼ 鞁ろ枆頃╇媹雼?

## 霛检澊靹犾姢

AGPL-3.0-only. [LICENSE](./LICENSE) 毳?彀戈碃頃橃劯鞖?
