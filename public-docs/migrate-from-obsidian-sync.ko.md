# Obsidian Sync에서 마이그레이션

[English](./migrate-from-obsidian-sync.md) | [简体中文](./migrate-from-obsidian-sync.zh-CN.md) | [繁體中文](./migrate-from-obsidian-sync.zh-Hant.md) | [日本語](./migrate-from-obsidian-sync.ja.md) | 한국어

문서 버전: v1.7.4.

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

이 가이드는 이미 Obsidian Sync를 사용하는 Obsidian vault의 현재 파일을 새 PKV Sync vault로 가져오는 방법을 설명합니다.

마이그레이션은 이 장치에 현재 존재하는 파일만 가져옵니다. Obsidian Sync 기록, 원격 버전 기록, 삭제된 파일 기록, 충돌 메타데이터는 가져오지 않습니다. PKV Sync 기록은 새 PKV vault를 만드는 마이그레이션 commit에서 시작됩니다.

마이그레이션은 Obsidian Sync를 비활성화하거나 제거하거나 변경하지 않습니다. PKV Sync 결과를 확인한 뒤 Obsidian Sync 사용을 중지하려면 Obsidian에서 수동으로 끄세요.

## 시작하기 전에

- 마이그레이션에 사용할 장치에서 Obsidian Sync 동기화가 끝날 때까지 기다립니다.
- 마이그레이션 전에 vault 폴더를 수동으로 백업합니다.
- 플러그인이 가져오는 동안 Obsidian을 열어 두고 완료될 때까지 파일 편집을 피합니다.
- 대상 PKV Sync 서버 계정을 먼저 만들거나 확인합니다.

## 가져오는 항목

PKV Sync는 새 vault를 만들고 현재 파일을 하나 이상의 커밋으로 가져옵니다. 대량 가져오기는 직렬화된 JSON 바이트 수와 변경 개수에 따라 나뉩니다.

일반 Markdown 파일, 첨부 파일, 일반 vault 파일은 PKV Sync의 강제 제외 규칙에 걸리지 않는 한 가져옵니다.

## 건너뛰는 항목

가져오기 도구는 Obsidian Sync 내부 파일, PKV Sync plugin 자체 상태, OS 부산물 파일, 로컬 런타임 파일을 건너뜁니다. 예:

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/**` (모든 플러그인. `data.json`에 평문 API token이 들어 있을 수 있습니다. PKV Sync 자체 상태는 로컬에만 보관)
- `.trash/**`
- `.git/**`
- `.DS_Store` (macOS)
- `Thumbs.db` (Windows)
- `*.tmp`, `*.lock` 같은 임시 파일
- 장치별 workspace, cache, trash, 임시 파일

선택한 `.obsidian` 설정 파일은 나중에 vault별 `.obsidian` allowlist로 동기화할 수 있습니다. 자세한 규칙은 `.obsidian` 설정 동기화 가이드를 참고하세요.

## 가져오기 실행 및 확인

연결하고 로그인한 뒤 Obsidian 명령 팔레트에서 현재 vault를 PKV Sync로 가져오는 명령을 실행합니다. 스캔 결과를 검토하고 새 원격 vault의 이름을 지정한 다음 마이그레이션을 시작합니다. 완료될 때까지 Obsidian을 열어 두세요.

플러그인은 첨부 파일을 업로드하고 파일을 배치로 나누어 푸시합니다. `Retry-After`가 없는 `413` 응답은 더 작은 배치로 재시도할 수 있습니다. 단일 파일의 전체 요청도 한도에 맞지 않으면 해당 경로를 표시하고 중지합니다. 수락된 배치는 대상 vault에 남으므로 실패하거나 취소한 뒤 일부 내용만 있는 원격 vault가 남을 수 있습니다.

업로드 중 창을 닫으면 진행 중인 요청이 끝난 뒤 후속 작업을 중지합니다. 연결이나 선택한 vault를 변경해도 이전 가져오기가 중지됩니다. 취소해도 서버가 이미 수락한 쓰기는 되돌리지 않습니다.

최종 저장 단계는 취소할 수 없다고 표시합니다. 이때 창을 닫으면 창만 숨깁니다. 새 vault 선택과 인덱스는 함께 저장합니다. 저장에 실패했지만 이전 상태 복원을 확인하면 이전 선택을 유지합니다. 디스크 상태를 확인할 수 없으면 동기화를 일시 중지하므로 저장소를 확인하고 플러그인을 다시 로드하세요. 저장된 선택이 유효하고 동기화가 재개된 경우에만 완료를 표시합니다. 저장 후 동기화 재개에 실패하면 저장된 선택과 인덱스를 유지하고 복구 오류를 표시합니다.

## 마이그레이션 후

다른 장치에서 새 PKV vault를 열고 노트와 첨부 파일이 올바르게 보이는지 확인합니다. 확인이 끝날 때까지 수동 백업을 보관하세요.

Obsidian Sync와 PKV Sync를 같은 폴더에서 계속 실행한다면 변경 작업을 신중하게 하세요. 두 동기화 시스템이 같은 파일에서 충돌할 수 있으며, PKV Sync는 마이그레이션 commit 이후 받은 변경만 기록합니다.
