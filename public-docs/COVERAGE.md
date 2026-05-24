# PKV Sync Coverage Baseline

English | [简体中文](./COVERAGE.zh-CN.md) | [繁體中文](./COVERAGE.zh-Hant.md) | [日本語](./COVERAGE.ja.md) | [한국어](./COVERAGE.ko.md)

This page tracks the CI coverage baseline for the Rust server and Obsidian plugin. CI compares fresh coverage reports against this table and fails when any component drops by more than the allowed threshold.

Rust coverage is generated only on the Ubuntu CI runner with `cargo tarpaulin`. Do not promote Windows-local tarpaulin output into this baseline. The gate allows a maximum drop of 5.0 percentage points per component.

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --out Json` on `ubuntu-latest` | 90.95% |
| Obsidian plugin | `vitest run --coverage` | 48.42% |

## Baseline Refresh

The v0.5.1 baseline was refreshed from CI run `26225831124`. Rust server
coverage is the Ubuntu tarpaulin artifact (`90.95%`). Obsidian plugin coverage
is the Vitest/V8 summary from the same run (`48.42%`); this run includes the
plugin entrypoint and UI import graph reached by the unload regression test, so
the measured surface is broader than the previous plugin baseline.

## Policy

- A pull request must not drop a tracked component by more than 5.0 percentage points below this baseline without a documented baseline update.
- New Rust or plugin modules should ship with at least 60% line coverage, unless the module is mostly generated glue, UI wiring that is covered manually, or platform integration that cannot run in CI.
- Rust server baseline updates must come from reviewed Ubuntu tarpaulin artifacts. Windows-local tarpaulin output is not used for this value.
- Recompute the baseline at major release boundaries. Intentional refactors that move substantial code between modules should update this document in the same commit as the coverage gate expectation.
- Exemptions must be explicit in this document or in the release notes for the change that introduces them.

The coverage gate reads this English document as the tracked source of truth. Keep every language mirror synchronized whenever this table changes.
