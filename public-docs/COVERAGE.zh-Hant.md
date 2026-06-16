# PKV Sync 瑕嗚搵鐜囧熀绶?

[English](./COVERAGE.md) | [绠€浣撲腑鏂嘳(./COVERAGE.zh-CN.md) | 绻侀珨涓枃 | [鏃ユ湰瑾瀅(./COVERAGE.ja.md) | [頃滉淡鞏碷(./COVERAGE.ko.md)

鏂囦欢鐗堟湰锛歷1.4.5銆?

鏈爜瑷橀寗 Rust server 鑸?Obsidian 澶栨帥鐨?CI 瑕嗚搵鐜囧熀绶氥€侰I 鏈冨皣鏈€鏂拌钃嬬巼鍫卞憡鑸囨琛ㄦ瘮杓冿紱浠讳竴 component 涓嬮檷瓒呴亷鍏佽ū闁惧€兼檪锛屾鏌ユ渻澶辨晽銆?

Rust 瑕嗚搵鐜囧彧鍦?Ubuntu CI runner 涓婇€忛亷 `cargo tarpaulin --engine Llvm` 鐢㈢敓銆備笉瑕佸皣 Windows 鏈 tarpaulin 杓稿嚭鎻愬崌鐐烘鍩虹窔銆傞杸绂佸厑瑷辨瘡鍊?component 鏈€澶氫笅闄?5.0 鍊嬬櫨鍒嗛粸銆?

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` on `ubuntu-latest` | 85.80% |
| Obsidian 澶栨帥 | `vitest run --coverage` | 48.42% |

## 鍩虹窔鍒锋柊

Rust server 鍩虹窔宸插湪瑕嗚搵鐜囦换鍕欏緸 tarpaulin ptrace engine 鍒囨彌鍒?LLVM engine 寰岋紝鏍规摎 CI run `26963777091` 鍒锋柊銆侺LVM artifact 鍫卞憡 `85.80%`锛岀従鍦ㄩ€欐槸鏈夋晥鐨?Rust 闁€绂併€侽bsidian 澶栨帥瑕嗚搵鐜囦粛浣跨敤 CI run `26225831124` 涓凡瀵╅柋鐨?Vitest/V8 鍩虹窔锛坄48.42%`锛夈€?

## 鏀跨瓥

- Pull request 涓嶆噳璁撳凡杩借工 component 鐩稿皪鏈熀绶氫笅闄嶈秴閬?5.0 鍊嬬櫨鍒嗛粸锛涢渶瑕佽鏁存檪锛屽繀闋堟彁浜ゆ槑纰虹殑鍩虹窔鏇存柊銆?
- 鏂板 Rust 鎴栧鎺?module 鎳夎嚦灏戞湁 60% line coverage锛岄櫎闈炶┎ module 涓昏鏄?generated glue銆乁I wiring锛屾垨 CI 鐒℃硶閬嬭鐨勫钩鍙版暣鍚堛€?
- Rust server 鍩虹窔鏇存柊蹇呴爤渚嗚嚜宸插闁辩殑 Ubuntu tarpaulin artifact銆俉indows 鏈 tarpaulin 杓稿嚭涓嶄綔鐐烘鍊间締婧愩€?
- CI 涓殑 Rust 瑕嗚搵鐜囦娇鐢?tarpaulin 鐨?LLVM engine銆俻trace engine 鏇惧湪 Ubuntu runner 涓婂煼琛屾湰韬€氶亷鐨?Admin 鏁村悎娓│鏅?segfault锛涙矑鏈夌稜鑹?CI 閲嶇従鍓嶄笉瑕佸垏鍥炪€?
- 涓昏 release 閭婄晫鎳夐噸鏂拌▓绠楀熀绶氥€傚ぇ閲忛噸妲嬪皫鑷寸▼寮忕⒓鍦?module 闁撻伔绉绘檪锛屾噳鍦ㄥ悓涓€ commit 鏇存柊鏈枃鑸?coverage gate 鏈熸湜銆?
- 璞佸厤蹇呴爤鍦ㄦ湰鏂囨垨寮曞叆璞佸厤鐨?release notes 涓槑纰鸿鏄庛€?

Coverage gate 浠ヨ嫳鏂囨枃浠?`public-docs/COVERAGE.md` 浣滅偤杩借工鐨勪簨瀵︿締婧愩€傛瘡娆′慨鏀硅〃鏍兼檪锛岃珛鍚屾鏇存柊鎵€鏈夎獮瑷€閺″儚銆?
