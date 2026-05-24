# PKV Sync 사용자 설명서

[English](./user-manual.md) | [简体中文](./user-manual.zh-CN.md) | [繁體中文](./user-manual.zh-Hant.md) | [日本語](./user-manual.ja.md) | 한국어

이 문서는 기계 번역으로 만든 초기 버전입니다. 공개 전에 원어민 검토를 권장합니다.

이 설명서는 기존 PKV Sync 서버에 연결하는 Obsidian 사용자를 위한 것입니다. 시작하기 전에 서버 관리자에게 서버 공유 URL과 계정 또는 초대 코드를 요청하세요.

## 플러그인 수동 설치

1. 해당 GitHub release에서 `pkv-sync-plugin.zip`을 다운로드합니다.
2. 아카이브를 vault에 풉니다.

   ```text
   <vault>/.obsidian/plugins/pkv-sync/
   ```

3. Obsidian에서 community plugins를 활성화합니다.
4. **PKV Sync**를 활성화합니다.

압축을 푼 디렉터리에는 `main.js`, `manifest.json`, `styles.css`가 있어야 합니다.

## 플러그인 업데이트

PKV Sync 설정 페이지에는 **Updates** 섹션이 있습니다. 기본적으로 플러그인은 연결된 PKV Sync 서버의 bundled plugin version을 확인합니다. 서버를 업그레이드하면 대응하는 plugin assets도 함께 제공되므로, self-hosted deployment에서는 이 경로가 권장됩니다. `public_host`가 설정되어 있으면 plugin asset URLs는 해당 외부 host로 고정됩니다. 필요하면 update source를 GitHub releases로 바꿀 수 있습니다.

업데이트가 있으면 **Update now**가 `main.js`, `manifest.json`, 그리고 존재할 경우 `styles.css`를 download하고 SHA-256을 verify한 뒤 plugin files에 써 넣고 Obsidian reload를 요청합니다. command palette에도 **PKV Sync: Check for PKV Sync plugin updates**가 있습니다.

## 서버에 연결

서버 공유 URL은 보통 다음과 같습니다.

```text
https://sync.example.com/k_xxx/
```

**Settings -> PKV Sync**를 열고 공유 URL을 붙여 넣은 다음 **Connect**를 클릭합니다. 배포 키가 URL에 포함되어 있으면 플러그인이 자동으로 채웁니다.

잘못된 서버를 입력했거나 다른 자체 호스팅 서버로 이동해야 하는 경우, 로그인 화면의 **Change server**를 사용해 플러그인을 다시 설치하지 않고 서버 설정으로 돌아갈 수 있습니다.

## 로그인 또는 등록

등록 동작은 서버 런타임 설정에 따라 달라집니다.

- **Disabled**: 관리자가 계정을 만들어야 합니다.
- **Invite only**: 관리자가 제공한 초대 코드를 입력합니다.
- **Open**: 계정을 직접 만들 수 있습니다.

로그인 후 기존 원격 vault를 선택하거나 새 원격 vault를 만듭니다. 선택한 원격 vault와 이미 완전히 동일한 로컬 vault를 연결하면, PKV Sync는 전체 vault에 대한 conflict file을 만들지 않고 일치하는 파일을 로컬 동기화 인덱스에 채택합니다.

## 동기화 동작

PKV Sync는 Obsidian 안에서 현재 vault를 동기화합니다.

- 로컬 파일 변경은 짧은 debounce 간격 후 push됩니다.
- 원격 변경은 주기적으로 poll됩니다.
- 설정 페이지와 command palette에서 수동 동기화를 사용할 수 있습니다.
- 관련 파일 생성/수정/삭제 이벤트가 동기화를 예약합니다.
- 창 blur가 동기화를 트리거할 수 있습니다.
- 시작 시 vault 내용과 로컬 동기화 인덱스에서 동기화되지 않은 로컬 변경을 감지합니다.

큰 첨부 파일을 업로드하는 동안 Obsidian을 열어 두세요. 플러그인은 연결 후 서버 설정을 읽고 서버가 제공한 텍스트 확장자 목록과 최대 파일 크기 규칙을 사용합니다.

## 선택적 `.obsidian` 동기화

PKV Sync는 vault별 allowlist를 통해 선택된 Obsidian 설정 파일을 동기화할 수 있습니다. 새 원격 vault는 테마, CSS snippets, 단축키, 앱 환경설정, 모양 환경설정, 활성화된 플러그인 목록 규칙으로 시작합니다.

기존 원격 vault는 사용자가 opt-in할 때까지 빈 allowlist를 유지합니다. **Settings -> PKV Sync**에서 현재 vault를 선택하고 **.obsidian sync rules**를 편집한 다음 저장합니다. 권장 starter list 버튼은 새 vault에 사용되는 것과 같은 starter rules를 채웁니다.

플러그인 코드와 플러그인 설정은 기본적으로 동기화되지 않습니다. `.obsidian/plugins/**` 또는 플러그인 `data.json` 파일 같은 고급 규칙을 추가하기 전에 [`dot-obsidian-sync-howto.ko.md`](./dot-obsidian-sync-howto.ko.md)를 읽어 주세요.

## 마지막 동기화 시간

설정 페이지는 마지막 성공 동기화를 상대 시간으로 표시합니다. 옆의 작은 펼침 컨트롤을 사용하면 다음 형식의 정확한 타임스탬프를 볼 수 있습니다.

```text
YYYY/MM/DD HH:MM:SS
```

플러그인은 선택한 IANA 시간대를 사용하며 기본값은 `Asia/Shanghai`입니다.

## 기록, Diff, 복원

서버가 기록 지원을 보고하고 플러그인 설정에서 **Enable history and diff UI**가 켜져 있으면 다음 위치에서 파일 기록을 볼 수 있습니다.

- **PKV Sync: Show file history**
- 파일 오른쪽 클릭 메뉴: **PKV Sync: File history**
- 파일 오른쪽 클릭 메뉴: **PKV Sync: Diff with previous**

기록 모달은 현재 파일의 commit을 시간, 장치, commit id, 변경 유형과 함께 나열합니다. 텍스트 파일은 unified diff를 표시할 수 있습니다. 바이너리 파일은 기록에 표시하고 복원할 수 있지만 PKV Sync는 바이너리 diff를 렌더링하지 않습니다.

버전을 복원하면 플러그인이 선택한 과거 콘텐츠를 서버에서 읽고 로컬 Obsidian vault에 다시 쓴 뒤 일반 동기화 엔진이 그 쓰기를 새 commit으로 push하게 합니다. 현재 로컬 파일이 마지막 동기화 hash와 다르면 확인 대화상자가 동기화되지 않은 로컬 변경이 덮어써진다고 경고합니다.

PKV Sync는 플러그인에 전체 오프라인 기록 캐시를 유지하지 않습니다. 기록과 diff 보기는 서버에 연결할 수 있어야 합니다.

## Conflict Files

두 장치가 같은 파일을 오프라인에서 편집하면 PKV Sync는 두 버전을 모두 보존합니다. 원격 또는 로컬의 대체 버전은 생성된 conflict file로 저장됩니다.

```text
note.md
note.conflict-2026-04-25-143022-Desktop.md
```

생성된 conflict files는 이후 동기화에서 제외됩니다. Obsidian에서 두 파일을 검토하고 보존할 내용을 수동으로 병합한 뒤 conflict file을 삭제하세요.

생성된 conflict files는 다음에서 관리할 수 있습니다.

- **Settings -> PKV Sync -> Conflict files**
- **PKV Sync: List conflict files**
- **PKV Sync: Delete conflict files**

삭제 작업은 PKV Sync가 생성한 conflict filename만 대상으로 합니다. `my.conflict-resolution-notes.md` 같은 일반 파일은 계속 동기화 대상입니다.

## 장치 Token

로그인하면 bearer device token이 발급됩니다. 인증된 사용은 token을 갱신하므로 활성 장치는 로그인 상태를 유지하고 90일 동안 유휴 상태인 장치는 만료됩니다. 플러그인은 안정적인 device ID를 유지하므로 같은 장치에서 다시 로그인하면 중복을 누적하지 않고 그 장치의 이전 활성 token을 대체합니다.

Obsidian 플러그인은 활성 token과 deployment key를 `<vault>/.obsidian/plugins/pkv-sync/data.json`에 저장합니다. 이 파일을 민감한 파일로 다루세요. 평문 백업과 클라우드 동기화 대상을 보호하고 공유하지 마세요. 파일이 유출되었을 수 있으면 로그아웃하거나 관리자에게 장치 token 철회를 요청한 뒤 다시 연결하세요.

- 플러그인 설정에서 현재 장치를 로그아웃할 수 있습니다.
- 분실한 장치의 token은 관리자에게 철회를 요청하세요.
- 비밀번호를 변경하면 현재 장치는 로그인 상태를 유지하고 다른 장치 token은 철회됩니다.

## MCP 접근

관리자가 `pkvsyncd mcp` 명령을 활성화하면 AI 도구가 bearer device token을 사용해 MCP로 vault에 접근할 수 있습니다. MCP는 vault 목록, 파일 목록, HEAD 또는 commit의 파일 읽기, 간단한 텍스트 검색, 그리고 optimistic concurrency가 적용된 명시적 write / delete tools를 제공합니다. stdio와 Streamable HTTP 설정 예시는 [`mcp-howto.ko.md`](./mcp-howto.ko.md)를 참고하세요.

## 명령

PKV Sync는 다음 command palette actions를 추가합니다.

- Show sync status
- Refresh account info
- Manual sync now
- View sync status details
- List conflict files
- Delete conflict files

## 개인정보 알림

PKV Sync는 종단 간 암호화를 제공하지 않습니다. 서버 관리자와 서버 파일 시스템 접근 권한이 있는 사람은 동기화된 vault 내용과 첨부 파일을 읽을 수 있습니다. 신뢰하는 서버와 관리자에게만 사용하세요.
