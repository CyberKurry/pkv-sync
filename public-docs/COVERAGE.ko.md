# PKV Sync Coverage Baseline

[English](./COVERAGE.md) | [简体中文](./COVERAGE.zh-CN.md) | [繁體中文](./COVERAGE.zh-Hant.md) | [日本語](./COVERAGE.ja.md) | 한국어

이 문서는 Rust server와 Obsidian plugin의 CI coverage baseline을 기록합니다. CI는 새 coverage report를 이 표와 비교하며, 어떤 component라도 허용 threshold보다 더 떨어지면 실패합니다.

Rust coverage는 Ubuntu CI runner에서 `cargo tarpaulin`으로만 생성합니다. Windows 로컬 tarpaulin 출력은 이 baseline으로 승격하지 않습니다. gate는 component별 최대 5.0 percentage points 하락을 허용합니다.

| Component | Report source | Baseline |
| --- | --- | ---: |
| Rust server | `cargo tarpaulin -p pkv-sync-server --out Json` on `ubuntu-latest` | 90.95% |
| Obsidian plugin | `vitest run --coverage` | 48.42% |

## Baseline Refresh

현재 baseline은 v0.5.1 사이클의 CI run `26225831124`에서 마지막으로 갱신되었으며, v1.0.0에 그대로 이어집니다. 이후 테스트 범위는 확장되었지만 어느 component도 하락하지 않았습니다. Rust server coverage는 Ubuntu tarpaulin artifact(`90.95%`)입니다. Obsidian plugin coverage는 같은 run의 Vitest/V8 summary(`48.42%`)이며, 이 run에는 unload regression test가 도달하는 plugin entrypoint와 UI import graph가 포함되어 이전 plugin baseline보다 측정 범위가 넓습니다.

v1.0.0 Release workflow가 완료되면 release 이후 CI run으로 v1.0.0 baseline refresh를 후속 patch로 진행할 계획입니다. 그 전까지는 v0.5.1 수치가 유효 gate로 남으며 component별 5.0 pp 하락 규칙이 계속 적용됩니다.

## Policy

- Pull request는 추적 대상 component를 이 baseline보다 5.0 percentage points 넘게 낮추면 안 됩니다. 필요한 경우 명시적인 baseline update를 함께 제출해야 합니다.
- 새 Rust 또는 plugin module은 generated glue, CI에서 실행할 수 없는 platform integration, 또는 대부분 UI wiring인 경우가 아니라면 최소 60% line coverage를 포함해야 합니다.
- Rust server baseline update는 review된 Ubuntu tarpaulin artifact에서 가져와야 합니다. Windows 로컬 tarpaulin 출력은 사용하지 않습니다.
- major release 경계에서는 baseline을 다시 계산합니다. 큰 refactor로 code가 module 사이를 이동하면 coverage gate expectation과 이 문서를 같은 commit에서 업데이트해야 합니다.
- 예외는 이 문서 또는 예외를 도입한 release notes에 명시해야 합니다.

Coverage gate는 영어 문서 `public-docs/COVERAGE.md`를 추적 source of truth로 읽습니다. 이 표를 변경할 때마다 모든 언어 mirror를 동기화하세요.
