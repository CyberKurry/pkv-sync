# PKV Sync Coverage Baseline

[English](./COVERAGE.md) | [绠€浣撲腑鏂嘳(./COVERAGE.zh-CN.md) | [绻侀珨涓枃](./COVERAGE.zh-Hant.md) | 鏃ユ湰瑾?| [頃滉淡鞏碷(./COVERAGE.ko.md)

銉夈偔銉ャ儭銉炽儓銉愩兗銈搞儳銉? v1.4.5銆?

銇撱伄銉氥兗銈搞伅銆丷ust server 銇?Obsidian plugin 銇?CI coverage baseline 銈掕閷层仐銇俱仚銆侰I 銇柊銇椼亜 coverage report 銈掋亾銇〃銇ㄦ瘮杓冦仐銆併亜銇氥倢銇嬨伄 component 銇岃ū瀹广仐銇嶃亜鍊ゃ倰瓒呫亪銇︿綆涓嬨仐銇熷牬鍚堛伀澶辨晽銇椼伨銇欍€?

Rust coverage 銇?Ubuntu CI runner 銇?`cargo tarpaulin --engine Llvm` 銇倛銇ｃ仸銇伩鐢熸垚銇椼伨銇欍€俉indows 銉兗銈儷銇?tarpaulin 鍑哄姏銈掋亾銇?baseline 銇槆鏍笺仐銇亜銇с亸銇犮仌銇勩€俫ate 銇?component 銇斻仺銇渶澶?5.0 percentage points 銇綆涓嬨倰瑷卞銇椼伨銇欍€?

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` on `ubuntu-latest` | 85.80% |
| Obsidian plugin | `vitest run --coverage` | 48.42% |

## Baseline Refresh

Rust server baseline 銇€乧overage job 銈?tarpaulin 銇?ptrace engine 銇嬨倝 LLVM engine 銇垏銈婃浛銇堛仧寰屻€丆I run `26963777091` 銇嬨倝 refresh 銇椼伨銇椼仧銆侺LVM artifact 銇?`85.80%` 銈掑牨鍛娿仐銇︺亰銈娿€佺従鍦ㄣ伅銇撱倢銇屾湁鍔广仾 Rust gate 銇с仚銆侽bsidian plugin coverage 銇€丆I run `26225831124` 銇?review 娓堛伩銇?Vitest/V8 baseline锛坄48.42%`锛夈倰寮曘亶缍氥亶浣跨敤銇椼伨銇欍€?

## Policy

- Pull request 銇€佽拷璺″璞?component 銈掋亾銇?baseline 銇嬨倝 5.0 percentage points 銈掕秴銇堛仸浣庝笅銇曘仜銇︺伅銇勩亼銇俱仜銈撱€傚繀瑕併仾鍫村悎銇€佹槑绀虹殑銇?baseline update 銈掑悓鏅傘伀鎻愬嚭銇椼伨銇欍€?
- 鏂般仐銇?Rust 銇俱仧銇?plugin module 銇€乬enerated glue銆丆I 銇у疅琛屻仹銇嶃仾銇?platform integration銆併伨銇熴伅涓汇伀 UI wiring 銇с仾銇勯檺銈娿€佸皯銇亸銇ㄣ倐 60% line coverage 銈掓寔銇ゃ伖銇嶃仹銇欍€?
- Rust server baseline update 銇€乺eview 娓堛伩銇?Ubuntu tarpaulin artifact 銇嬨倝鍙栧緱銇椼伨銇欍€俉indows 銉兗銈儷銇?tarpaulin 鍑哄姏銇娇鐢ㄣ仐銇俱仜銈撱€?
- CI 銇?Rust coverage 銇?tarpaulin 銇?LLVM engine 銈掍娇鐢ㄣ仐銇俱仚銆俻trace engine 銇?Ubuntu runner 銇с€佹湰鏉?pass 銇欍倠 Admin integration tests 銇疅琛屼腑銇?segfault 銇椼仧銇撱仺銇屻亗銈嬨仧銈併€乬reen CI 銇у啀鐝剧⒑瑾嶃仚銈嬨伨銇с伅鎴汇仌銇亜銇с亸銇犮仌銇勩€?
- major release 澧冪晫銇с伅 baseline 銈掑啀瑷堢畻銇椼伨銇欍€傚ぇ銇嶃仾 refactor 銇?code 銇?module 闁撱倰绉诲嫊銇欍倠鍫村悎銇€乧overage gate expectation 銇ㄣ亾銇枃鏇搞倰鍚屻仒 commit 銇ф洿鏂般仐銇俱仚銆?
- 渚嬪銇€併亾銇枃鏇搞伨銇熴伅銇濄伄渚嬪銈掑皫鍏ャ仚銈?release notes 銇ф槑绀恒仐銇俱仚銆?

Coverage gate 銇嫳瑾炴枃鏇?`public-docs/COVERAGE.md` 銈掕拷璺″璞°伄 source of truth 銇ㄣ仐銇﹁銇裤伨銇欍€傘亾銇〃銈掑鏇淬仐銇熷牬鍚堛伅銆併仚銇广仸銇█瑾?mirror 銈掑悓鏈熴仐銇︺亸銇犮仌銇勩€?
