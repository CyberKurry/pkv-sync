# PKV Sync Coverage Baseline

[English](./COVERAGE.md) | [简体中文](./COVERAGE.zh-CN.md) | [繁體中文](./COVERAGE.zh-Hant.md) | [日本語](./COVERAGE.ja.md) | 한국어

문서 버전: v1.0.13.

이 문서는 Rust server와 Obsidian plugin의 CI coverage baseline을 기록합니다. CI는 새 coverage report를 이 표와 비교하며, 어떤 component라도 허용 threshold보다 더 떨어지면 실패합니다.

Rust coverage는 Ubuntu CI runner에서 `cargo tarpaulin --engine Llvm`으로만 생성합니다. Windows 로컬 tarpaulin 출력은 이 baseline으로 승격하지 않습니다. gate는 component별 최대 5.0 percentage points 하락을 허용합니다.

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --engine Llvm --out Json` on `ubuntu-latest` | 85.80% |
| Obsidian plugin | `vitest run --coverage` | 48.42% |

## Baseline Refresh

Rust server baseline은 coverage job을 tarpaulin ptrace engine에서 LLVM engine으로 전환한 뒤 CI run `26963777091`에서 갱신했습니다. LLVM artifact는 `85.80%`를 보고했으며, 이제 이것이 유효한 Rust gate입니다. Obsidian plugin coverage는 CI run `26225831124`에서 review된 Vitest/V8 baseline(`48.42%`)을 계속 사용합니다.

## Policy

- Pull request는 추적 대상 component를 이 baseline보다 5.0 percentage points 넘게 낮추면 안 됩니다. 필요한 경우 명시적인 baseline update를 함께 제출해야 합니다.
- 새 Rust 또는 plugin module은 generated glue, CI에서 실행할 수 없는 platform integration, 또는 대부분 UI wiring인 경우가 아니라면 최소 60% line coverage를 포함해야 합니다.
- Rust server baseline update는 review된 Ubuntu tarpaulin artifact에서 가져와야 합니다. Windows 로컬 tarpaulin 출력은 사용하지 않습니다.
- CI의 Rust coverage는 tarpaulin LLVM engine을 사용합니다. ptrace engine은 Ubuntu runner에서 원래 통과하는 Admin integration tests를 실행하던 중 segfault한 적이 있으므로, green CI 재현 없이 되돌리지 마세요.
- major release 경계에서는 baseline을 다시 계산합니다. 큰 refactor로 code가 module 사이를 이동하면 coverage gate expectation과 이 문서를 같은 commit에서 업데이트해야 합니다.
- 예외는 이 문서 또는 예외를 도입한 release notes에 명시해야 합니다.

Coverage gate는 영어 문서 `public-docs/COVERAGE.md`를 추적 source of truth로 읽습니다. 이 표를 변경할 때마다 모든 언어 mirror를 동기화하세요.
