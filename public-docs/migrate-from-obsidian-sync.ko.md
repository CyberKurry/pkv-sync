# Obsidian Sync에서 마이그레이션

[English](./migrate-from-obsidian-sync.md) | [简体中文](./migrate-from-obsidian-sync.zh-CN.md) | [繁體中文](./migrate-from-obsidian-sync.zh-Hant.md) | [日本語](./migrate-from-obsidian-sync.ja.md) | 한국어

문서 버전: v1.4.3.

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

이 가이드는 이미 Obsidian Sync를 사용하는 Obsidian vault의 현재 파일을 새 PKV Sync vault로 가져오는 방법을 설명합니다.

마이그레이션은 이 장치에 현재 존재하는 파일만 가져옵니다. Obsidian Sync 기록, 원격 버전 기록, 삭제된 파일 기록, 충돌 메타데이터는 가져오지 않습니다. PKV Sync 기록은 새 PKV vault를 만드는 마이그레이션 commit에서 시작됩니다.

마이그레이션은 Obsidian Sync를 비활성화하거나 제거하거나 변경하지 않습니다. PKV Sync 결과를 확인한 뒤 Obsidian Sync 사용을 중지하려면 Obsidian에서 수동으로 끄세요.

## 시작하기 전에

- 마이그레이션에 사용할 장치에서 Obsidian Sync 동기화가 끝날 때까지 기다립니다.
- 마이그레이션 전에 vault 폴더를 수동으로 백업합니다.
- 가능하면 가져오는 동안 Obsidian을 닫아 두거나, 적어도 파일 편집을 피합니다.
- 대상 PKV Sync 서버 계정을 먼저 만들거나 확인합니다.

## 가져오는 항목

PKV Sync는 새 vault를 만들고 현재 가져오기 내용을 첫 PKV 기록 항목으로 commit합니다.

일반 Markdown 파일, 첨부 파일, 일반 vault 파일은 PKV Sync의 강제 제외 규칙에 걸리지 않는 한 가져옵니다.

## 건너뛰는 항목

가져오기 도구는 Obsidian Sync 내부 파일, PKV Sync plugin 자체 상태, OS 부산물 파일, 로컬 런타임 파일을 건너뜁니다. 예:

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/` (plugin 자체 설정과 token 저장소는 로컬에만 보관)
- `.trash/**`
- `.git/**`
- `.DS_Store` (macOS)
- `Thumbs.db` (Windows)
- `*.tmp`, `*.lock` 같은 임시 파일
- 장치별 workspace, cache, trash, 임시 파일

선택한 `.obsidian` 설정 파일은 나중에 vault별 `.obsidian` allowlist로 동기화할 수 있습니다. 자세한 규칙은 `.obsidian` 설정 동기화 가이드를 참고하세요.

## 마이그레이션 후

다른 장치에서 새 PKV vault를 열고 노트와 첨부 파일이 올바르게 보이는지 확인합니다. 확인이 끝날 때까지 수동 백업을 보관하세요.

Obsidian Sync와 PKV Sync를 같은 폴더에서 계속 실행한다면 변경 작업을 신중하게 하세요. 두 동기화 시스템이 같은 파일에서 충돌할 수 있으며, PKV Sync는 마이그레이션 commit 이후 받은 변경만 기록합니다.
