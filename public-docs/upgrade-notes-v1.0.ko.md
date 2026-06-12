# Upgrade notes: 0.x에서 1.0으로

[English](./upgrade-notes-v1.0.md) | [简体中文](./upgrade-notes-v1.0.zh-CN.md) | [繁體中文](./upgrade-notes-v1.0.zh-Hant.md) | [日本語](./upgrade-notes-v1.0.ja.md) | 한국어

문서 버전: v1.3.1.

PKV Sync 1.0은 첫 stable release입니다. 또한 향후 1.x maintenance를 위해 SQLite migration
baseline을 reset합니다.

## 중요한 database note

PKV Sync 1.0은 단일 `0001_initial.sql` baseline migration을 제공합니다. 0.x release로 만든
SQLite database는 1.0.0으로 in-place upgrade할 수 없습니다.

0.x server를 운영 중이라면 다음 경로 중 하나를 선택하세요.

1. 기존 deployment는 migration 준비를 위한 backup, materialize, export에 필요한 동안만 최종 0.8.x patch release에 유지합니다.
2. 각 vault를 backup 또는 materialize하고, 새 1.0 data directory로 시작한 뒤 user와 vault를
   다시 만들고 contents를 새 server로 import 또는 push합니다.
3. migration rehearsal을 시도하기 전에 0.x data root의 전체 `pkvsyncd backup`을 보관합니다.

기존 0.x `metadata.db`에 1.0 binary 또는 Docker image를 직접 연결하지 마세요.

## 1.0에서 안정화되는 surface

1.0부터 다음 surface는 semantic versioning을 따릅니다.

- `public-docs/openapi.yaml`에 문서화된 public REST routes.
- MCP how-to에 문서화된 MCP stdio 및 Streamable HTTP tool behavior.
- 1.x fresh database용 SQLite migrations. 이후 1.x migrations는 이 v1 baseline 이후
  append-only입니다.
- vault별 git repository layout과 content-addressed blob storage.
- CLI subcommands와 기존 flags.
- Obsidian plugin settings와 sync behavior. 일반적인 backward-compatible 1.x feature addition은
  있을 수 있습니다.

OpenAPI에 문서화되지 않은 route, 예를 들어 Admin Web UI form handler는 internal implementation
detail입니다.

## 권장 0.x to 1.0 절차

1. 가능하면 먼저 기존 deployment를 최종 0.8.x patch release로 update하고, backup, materialize, export 준비에만 사용합니다.
2. `pkvsyncd backup --output <backup-dir>`를 실행하고 결과를 안전하게 보관합니다.
3. 각 vault에 대해 최신 Obsidian client, `git clone`, 또는
   `pkvsyncd materialize <vault-id> --output <dir>`로 현재 file tree를 만듭니다.
4. 기존 server를 중지합니다.
5. 빈 `data_dir`와 `metadata.db`로 PKV Sync 1.0을 시작합니다.
6. `/setup`을 완료하고 user와 vault를 다시 만든 뒤, materialized vault contents를 push 또는
   import합니다.
7. user에게 Obsidian plugin을 1.0.0으로 update하도록 안내합니다.

## Plugin compatibility

1.0 server에서 supported plugin은 server에 bundled된 1.0 Obsidian plugin입니다. 오래된 v0.8.x
plugin도 core sync API는 같지만, 새로운 수정과 self-update hardening은 1.0+에서만 유지됩니다.

## 0.x에서의 breaking changes

- migration이 단일 v1 baseline으로 squash되었기 때문에 0.x SQLite database는 in-place upgrade되지
  않습니다.
- first-run setup은 browser-based를 유지합니다. fresh server는 random admin password를 log에
  출력하지 않습니다.

vault file contents, git history, blob은 backup/materialize/recreate/import workflow로 가져갈 수
있습니다.

## Known caveats

- native per-vault E2EE는 1.0 범위에 포함되지 않습니다. 지금 client-side encrypted file contents가
  필요하고 plaintext path를 받아들일 수 있다면 [`git-crypt`](./git-crypt-howto.ko.md)를 사용하세요.
- `/metrics`는 default로 disabled이며, 활성화해도 production authentication gates가 필요합니다.
- production에서는 `public_host`를 설정하세요. configured HTTPS public origin을 결정할 수 없으면
  admin POST는 의도적으로 fail-closed됩니다.
