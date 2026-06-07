# PKV Sync Coverage Baseline

[English](./COVERAGE.md) | [简体中文](./COVERAGE.zh-CN.md) | [繁體中文](./COVERAGE.zh-Hant.md) | 日本語 | [한국어](./COVERAGE.ko.md)

ドキュメントバージョン: v1.0.14。

このページは、Rust server と Obsidian plugin の CI coverage baseline を記録します。CI は新しい coverage report をこの表と比較し、いずれかの component が許容しきい値を超えて低下した場合に失敗します。

Rust coverage は Ubuntu CI runner で `cargo tarpaulin --engine Llvm` によってのみ生成します。Windows ローカルの tarpaulin 出力をこの baseline に昇格しないでください。gate は component ごとに最大 5.0 percentage points の低下を許容します。

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` on `ubuntu-latest` | 85.80% |
| Obsidian plugin | `vitest run --coverage` | 48.42% |

## Baseline Refresh

Rust server baseline は、coverage job を tarpaulin の ptrace engine から LLVM engine に切り替えた後、CI run `26963777091` から refresh しました。LLVM artifact は `85.80%` を報告しており、現在はこれが有効な Rust gate です。Obsidian plugin coverage は、CI run `26225831124` で review 済みの Vitest/V8 baseline（`48.42%`）を引き続き使用します。

## Policy

- Pull request は、追跡対象 component をこの baseline から 5.0 percentage points を超えて低下させてはいけません。必要な場合は、明示的な baseline update を同時に提出します。
- 新しい Rust または plugin module は、generated glue、CI で実行できない platform integration、または主に UI wiring でない限り、少なくとも 60% line coverage を持つべきです。
- Rust server baseline update は、review 済みの Ubuntu tarpaulin artifact から取得します。Windows ローカルの tarpaulin 出力は使用しません。
- CI の Rust coverage は tarpaulin の LLVM engine を使用します。ptrace engine は Ubuntu runner で、本来 pass する Admin integration tests の実行中に segfault したことがあるため、green CI で再現確認するまでは戻さないでください。
- major release 境界では baseline を再計算します。大きな refactor で code が module 間を移動する場合は、coverage gate expectation とこの文書を同じ commit で更新します。
- 例外は、この文書またはその例外を導入する release notes で明示します。

Coverage gate は英語文書 `public-docs/COVERAGE.md` を追跡対象の source of truth として読みます。この表を変更した場合は、すべての言語 mirror を同期してください。
