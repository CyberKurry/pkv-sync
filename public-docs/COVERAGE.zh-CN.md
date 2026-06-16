# PKV Sync 瑕嗙洊鐜囧熀绾?

[English](./COVERAGE.md) | 绠€浣撲腑鏂?| [绻侀珨涓枃](./COVERAGE.zh-Hant.md) | [鏃ユ湰瑾瀅(./COVERAGE.ja.md) | [頃滉淡鞏碷(./COVERAGE.ko.md)

鏂囨。鐗堟湰锛歷1.4.5銆?

鏈〉璁板綍 Rust 鏈嶅姟绔拰 Obsidian 鎻掍欢鐨?CI 瑕嗙洊鐜囧熀绾裤€侰I 浼氭妸鏈€鏂拌鐩栫巼鎶ュ憡涓庢琛ㄥ姣旓紱浠讳竴缁勪欢鐨勪笅闄嶅箙搴﹁秴杩囧厑璁搁槇鍊兼椂锛屾鏌ヤ細澶辫触銆?

Rust 瑕嗙洊鐜囧彧鍦?Ubuntu CI runner 涓婇€氳繃 `cargo tarpaulin --engine Llvm` 鐢熸垚銆備笉瑕佹妸 Windows 鏈湴 tarpaulin 杈撳嚭鎻愬崌涓鸿鍩虹嚎銆傞棬绂佸厑璁告瘡涓粍浠舵渶澶氫笅闄?5.0 涓櫨鍒嗙偣銆?

| 缁勪欢 | 鎶ュ憡鏉ユ簮 | 鍩虹嚎 |
| --- | --- | ---: |
| Rust 鏈嶅姟绔?| 鍦?`ubuntu-latest` 涓婅繍琛?`cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` | 85.80% |
| Obsidian 鎻掍欢 | `vitest run --coverage` | 48.42% |

## 鍩虹嚎鍒锋柊

Rust 鏈嶅姟绔熀绾垮凡鍦ㄨ鐩栫巼浠诲姟浠?tarpaulin ptrace engine 鍒囨崲鍒?LLVM engine 鍚庯紝鏍规嵁 CI run `26963777091` 鍒锋柊銆侺LVM artifact 鎶ュ憡 `85.80%`锛岀幇鍦ㄨ繖鏄湁鏁堢殑 Rust 闂ㄧ銆侽bsidian 鎻掍欢瑕嗙洊鐜囦粛浣跨敤 CI run `26225831124` 涓凡瀹￠槄鐨?Vitest/V8 鍩虹嚎锛坄48.42%`锛夈€?

## 绛栫暐

- PR 涓嶅簲璁╁凡璺熻釜缁勪欢鐩稿鏈熀绾夸笅闄嶈秴杩?5.0 涓櫨鍒嗙偣锛涚‘闇€璋冩暣鏃讹紝蹇呴』鎻愪氦鏄庣‘鐨勫熀绾挎洿鏂般€?
- 鏂板 Rust 鎴栨彃浠舵ā鍧楀簲鑷冲皯鏈?60% 琛岃鐩栫巼锛涗富瑕佺敱鐢熸垚浠ｇ爜銆乁I 杩炴帴灞傛垨 CI 鏃犳硶杩愯鐨勫钩鍙伴泦鎴愮粍鎴愮殑妯″潡闄ゅ銆?
- Rust 鏈嶅姟绔熀绾挎洿鏂板繀椤绘潵鑷凡瀹￠槄鐨?Ubuntu tarpaulin artifact銆備笉瑕佷娇鐢?Windows 鏈湴 tarpaulin 杈撳嚭鏇存柊璇ュ€笺€?
- CI 涓殑 Rust 瑕嗙洊鐜囦娇鐢?tarpaulin 鐨?LLVM engine銆俻trace engine 鏇惧湪 Ubuntu runner 涓婃墽琛屾湰韬€氳繃鐨?Admin 闆嗘垚娴嬭瘯鏃?segfault锛涙病鏈夌豢鑹?CI 澶嶇幇鍓嶄笉瑕佸垏鍥炪€?
- 涓昏 release 杈圭晫搴旈噸鏂拌绠楀熀绾裤€傚ぇ瑙勬ā閲嶆瀯瀵艰嚧浠ｇ爜鍦ㄦā鍧楅棿杩佺Щ鏃讹紝搴斿湪鍚屼竴鎻愪氦涓悓姝ユ洿鏂版湰鏂囨。鍜岃鐩栫巼闂ㄧ棰勬湡銆?
- 璞佸厤蹇呴』鍦ㄦ湰鏂囨。鎴栧紩鍏ヨ璞佸厤鐨?release notes 涓槑纭鏄庛€?

瑕嗙洊鐜囬棬绂佷互鑻辨枃鏂囨。 `public-docs/COVERAGE.md` 浣滀负璺熻釜鐨勪簨瀹炴潵婧愩€傛瘡娆′慨鏀硅琛ㄦ椂锛岃鍚屾鏇存柊鎵€鏈夎瑷€闀滃儚銆?
