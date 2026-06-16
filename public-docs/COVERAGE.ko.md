# PKV Sync Coverage Baseline

[English](./COVERAGE.md) | [绠€浣撲腑鏂嘳(./COVERAGE.zh-CN.md) | [绻侀珨涓枃](./COVERAGE.zh-Hant.md) | [鏃ユ湰瑾瀅(./COVERAGE.ja.md) | 頃滉淡鞏?

氍胳劀 氩勳爠: v1.4.5.

鞚?氍胳劀電?Rust server鞕€ Obsidian plugin鞚?CI coverage baseline鞚?旮半頃╇媹雼? CI電?靸?coverage report毳?鞚?響滌檧 牍勱祼頃橂┌, 鞏措枻 component霛茧弰 項堨毄 threshold氤措嫟 雿?霒柎歆€氅?鞁ろ尐頃╇媹雼?

Rust coverage電?Ubuntu CI runner鞐愳劀 `cargo tarpaulin --engine Llvm`鞙茧毵?靸濎劚頃╇媹雼? Windows 搿滌滑 tarpaulin 於滊牓鞚€ 鞚?baseline鞙茧 鞀龟博頃橃 鞎婌姷雼堧嫟. gate電?component氤?斓滊寑 5.0 percentage points 頃橂澖鞚?項堨毄頃╇媹雼?

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` on `ubuntu-latest` | 85.80% |
| Obsidian plugin | `vitest run --coverage` | 48.42% |

## Baseline Refresh

Rust server baseline鞚€ coverage job鞚?tarpaulin ptrace engine鞐愳劀 LLVM engine鞙茧 鞝勴櫂頃?霋?CI run `26963777091`鞐愳劀 臧膘嫚頄堨姷雼堧嫟. LLVM artifact電?`85.80%`毳?氤搓碃頄堨溂氅? 鞚挫牅 鞚搓矁鞚?鞙犿毃頃?Rust gate鞛呺媹雼? Obsidian plugin coverage電?CI run `26225831124`鞐愳劀 review霅?Vitest/V8 baseline(`48.42%`)鞚?瓿勳啀 靷毄頃╇媹雼?

## Policy

- Pull request電?於旍爜 雽€靸?component毳?鞚?baseline氤措嫟 5.0 percentage points 雱橁矊 雮稊氅?鞎?霅╇媹雼? 頃勳殧頃?瓴届毎 氇呾嫓鞝侅澑 baseline update毳?頃粯 鞝滌稖頃挫暭 頃╇媹雼?
- 靸?Rust 霕愲姅 plugin module鞚€ generated glue, CI鞐愳劀 鞁ろ枆頃?靾?鞐嗠姅 platform integration, 霕愲姅 雽€攵€攵?UI wiring鞚?瓴届毎臧€ 鞎勲媹霛茧┐ 斓滌唽 60% line coverage毳?韽暔頃挫暭 頃╇媹雼?
- Rust server baseline update電?review霅?Ubuntu tarpaulin artifact鞐愳劀 臧€鞝胳檧鞎?頃╇媹雼? Windows 搿滌滑 tarpaulin 於滊牓鞚€ 靷毄頃橃 鞎婌姷雼堧嫟.
- CI鞚?Rust coverage電?tarpaulin LLVM engine鞚?靷毄頃╇媹雼? ptrace engine鞚€ Ubuntu runner鞐愳劀 鞗愲灅 韱店臣頃橂姅 Admin integration tests毳?鞁ろ枆頃橂崢 欷?segfault頃?鞝侅澊 鞛堨溂氙€搿? green CI 鞛槃 鞐嗢澊 霅橂弻毽 毵堨劯鞖?
- major release 瓴疥硠鞐愳劀電?baseline鞚?雼れ嫓 瓿勳偘頃╇媹雼? 韥?refactor搿?code臧€ module 靷澊毳?鞚措彊頃橂┐ coverage gate expectation瓿?鞚?氍胳劀毳?臧欖潃 commit鞐愳劀 鞐呺嵃鞚错姼頃挫暭 頃╇媹雼?
- 鞓堨櫢電?鞚?氍胳劀 霕愲姅 鞓堨櫢毳?霃勳瀰頃?release notes鞐?氇呾嫓頃挫暭 頃╇媹雼?

Coverage gate電?鞓侅柎 氍胳劀 `public-docs/COVERAGE.md`毳?於旍爜 source of truth搿?鞚届姷雼堧嫟. 鞚?響滊ゼ 氤€瓴巾暊 霑岆雼?氇摖 鞏胳柎 mirror毳?霃欔赴頇旐晿靹胳殧.
