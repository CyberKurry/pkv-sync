# PKV Sync 覆盖率基线

[English](./COVERAGE.md) | 简体中文 | [繁體中文](./COVERAGE.zh-Hant.md) | [日本語](./COVERAGE.ja.md) | [한국어](./COVERAGE.ko.md)

本页记录 Rust 服务端和 Obsidian 插件的 CI 覆盖率基线。CI 会把最新覆盖率报告与此表对比；任一组件的下降幅度超过允许阈值时，检查会失败。

Rust 覆盖率只在 Ubuntu CI runner 上通过 `cargo tarpaulin --engine Llvm` 生成。不要把 Windows 本地 tarpaulin 输出提升为该基线。门禁允许每个组件最多下降 5.0 个百分点。

| 组件 | 报告来源 | 基线 |
| --- | --- | ---: |
| Rust 服务端 | 在 `ubuntu-latest` 上运行 `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` | 85.80% |
| Obsidian 插件 | `vitest run --coverage` | 48.42% |

## 基线刷新

Rust 服务端基线已在覆盖率任务从 tarpaulin ptrace engine 切换到 LLVM engine 后，根据 CI run `26963777091` 刷新。LLVM artifact 报告 `85.80%`，现在这是有效的 Rust 门禁。Obsidian 插件覆盖率仍使用 CI run `26225831124` 中已审阅的 Vitest/V8 基线（`48.42%`）。

## 策略

- PR 不应让已跟踪组件相对本基线下降超过 5.0 个百分点；确需调整时，必须提交明确的基线更新。
- 新增 Rust 或插件模块应至少有 60% 行覆盖率；主要由生成代码、UI 连接层或 CI 无法运行的平台集成组成的模块除外。
- Rust 服务端基线更新必须来自已审阅的 Ubuntu tarpaulin artifact。不要使用 Windows 本地 tarpaulin 输出更新该值。
- CI 中的 Rust 覆盖率使用 tarpaulin 的 LLVM engine。ptrace engine 曾在 Ubuntu runner 上执行本身通过的 Admin 集成测试时 segfault；没有绿色 CI 复现前不要切回。
- 主要 release 边界应重新计算基线。大规模重构导致代码在模块间迁移时，应在同一提交中同步更新本文档和覆盖率门禁预期。
- 豁免必须在本文档或引入该豁免的 release notes 中明确说明。

覆盖率门禁以英文文档 `public-docs/COVERAGE.md` 作为跟踪的事实来源。每次修改该表时，请同步更新所有语言镜像。
