# PKV Sync Coverage Baseline

English | [简体中文](./COVERAGE.zh-CN.md)

This page tracks the CI coverage baseline for the Rust server and Obsidian plugin. CI compares fresh coverage reports against this table and fails when any component drops by more than the allowed threshold.

Rust coverage is generated only on the Ubuntu CI runner with `cargo tarpaulin`. Do not promote Windows-local tarpaulin output into this baseline. The gate allows a maximum drop of 5.0 percentage points per component.

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --out Json` on `ubuntu-latest` | 0.00% |
| Obsidian plugin | `vitest run --coverage` | 62.86% |

## Policy

- A pull request must not drop a tracked component by more than 5.0 percentage points below this baseline without a documented baseline update.
- New Rust or plugin modules should ship with at least 60% line coverage, unless the module is mostly generated glue, UI wiring that is covered manually, or platform integration that cannot run in CI.
- The Rust server baseline is a bootstrap value until the first Ubuntu tarpaulin artifact is reviewed and committed. Windows-local tarpaulin output is not used for this value.
- Recompute the baseline at major release boundaries. Intentional refactors that move substantial code between modules should update this document in the same commit as the coverage gate expectation.
- Exemptions must be explicit in this document or in the release notes for the change that introduces them.

The coverage gate reads this English document as the tracked source of truth. Keep the Chinese mirror in `public-docs/COVERAGE.zh-CN.md` synchronized whenever this table changes.
