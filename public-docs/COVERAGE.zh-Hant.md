# PKV Sync 覆蓋率基線

[English](./COVERAGE.md) | [简体中文](./COVERAGE.zh-CN.md) | 繁體中文 | [日本語](./COVERAGE.ja.md) | [한국어](./COVERAGE.ko.md)

本頁記錄 Rust server 與 Obsidian 外掛的 CI 覆蓋率基線。CI 會將最新覆蓋率報告與此表比較；任一 component 下降超過允許閾值時，檢查會失敗。

Rust 覆蓋率只在 Ubuntu CI runner 上透過 `cargo tarpaulin --engine Llvm` 產生。不要將 Windows 本機 tarpaulin 輸出提升為此基線。門禁允許每個 component 最多下降 5.0 個百分點。

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` on `ubuntu-latest` | 90.95% |
| Obsidian 外掛 | `vitest run --coverage` | 48.42% |

## 基線刷新

目前基線最近一次刷新來自 v0.5.1 週期的 CI run `26225831124`，並原樣延續到 v1.0.0：之後測試面積有所擴張，但兩個 component 都沒有下降。Rust server 覆蓋率採用 Ubuntu tarpaulin artifact（`90.95%`）。Obsidian 外掛覆蓋率採用同一次 run 的 Vitest/V8 summary（`48.42%`）；這次 run 包含卸載回歸測試觸達的外掛 entrypoint 與 UI import graph，因此量測範圍比前一個外掛基線更廣。

v1.0.0 發布後將計畫透過後續 patch，從發布後的 CI run 刷新 v1.0.0 基線；在此之前，v0.5.1 數字仍是有效的門禁，且 per-component 5.0 個百分點下降規則仍然適用。

## 政策

- Pull request 不應讓已追蹤 component 相對本基線下降超過 5.0 個百分點；需要調整時，必須提交明確的基線更新。
- 新增 Rust 或外掛 module 應至少有 60% line coverage，除非該 module 主要是 generated glue、UI wiring，或 CI 無法運行的平台整合。
- Rust server 基線更新必須來自已審閱的 Ubuntu tarpaulin artifact。Windows 本機 tarpaulin 輸出不作為此值來源。
- CI 中的 Rust 覆蓋率使用 tarpaulin 的 LLVM engine。ptrace engine 曾在 Ubuntu runner 上執行本身通過的 Admin 整合測試時 segfault；沒有綠色 CI 重現前不要切回。
- 主要 release 邊界應重新計算基線。大量重構導致程式碼在 module 間遷移時，應在同一 commit 更新本文與 coverage gate 期望。
- 豁免必須在本文或引入豁免的 release notes 中明確說明。

Coverage gate 以英文文件 `public-docs/COVERAGE.md` 作為追蹤的事實來源。每次修改表格時，請同步更新所有語言鏡像。
