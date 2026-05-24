# 보안 정책

[English](./SECURITY.md) | [简体中文](./SECURITY.zh-CN.md) | [繁體中文](./SECURITY.zh-Hant.md) | [日本語](./SECURITY.ja.md) | 한국어

## 지원 버전

PKV Sync는 v1.0.0부터 semantic versioning을 따릅니다. 보안 수정은 현재 1.x minor line과
직전 1.x minor line에 대해 유지합니다.

| Version | Status | 보안 지원 종료 |
| --- | --- | --- |
| 최신 1.x minor | Active | TBD |
| 직전 1.x minor | Security-fix only | 다음 1.x minor가 출시될 때 |
| 0.x | Not supported | v1.0.0 release |

## 취약점 신고

보안 취약점을 공개 GitHub issue로 열지 마세요.

권장 채널：`cyberkurry/pkv-sync`의 GitHub Security Advisories에서 private report를 여세요.

신고에는 다음 내용을 포함해 주세요.

- 영향을 받는 PKV Sync version.
- 최소 재현 절차.
- 영향 평가.
- 제안하는 수정안이 있다면 그 내용.

## 대응 목표

- 최초 acknowledgement：5 business days.
- severity triage：10 business days.
- fix and coordinated disclosure：critical/high는 90 days, medium/low는 180 days.
- CVE assignment：해당되는 경우 GitHub Security Advisories를 통해 진행합니다.

## Scope

포함 범위：

- `pkvsyncd` server binary.
- Obsidian plugin.
- Admin Web UI.
- MCP stdio 및 Streamable HTTP transports.
- 안전하지 않은 deployment를 권장하는 공개 문서.

제외 범위：

- PKV Sync 외부의 host, reverse proxy, TLS, Docker, systemd, OS hardening.
- third-party Obsidian plugin.
- PKV Sync의 사용 방식으로 exploit할 수 없는 dependency vulnerability.
- 이미 침해된 host의 administrator access가 필요한 report.

## 알려진 비이슈

- PKV Sync 1.0은 일반 vault contents를 server-side plaintext로 저장합니다.
  이는 의도된 설계입니다. vault 단위 native E2EE는 1.x roadmap에서 계획되어 있습니다.
  지금 client-side encryption이 필요하다면
  [`git-crypt`](./public-docs/git-crypt-howto.ko.md)를 사용할 수 있습니다. trade-off는 README에
  설명되어 있습니다.
- `/metrics`는 default로 disabled입니다. 활성화해도 production server stack에서는
  deployment key middleware, PKV Sync User-Agent guard, admin bearer token이 필요합니다.
- MCP HTTP는 deployment key와 bearer token authentication을 모두 요구합니다.
  public으로 노출하더라도 authenticated admin-adjacent surface처럼 보호해야 합니다.

## Disclosure

PKV Sync는 coordinated disclosure를 따릅니다. 익명을 원하지 않는 한 reporter는 advisory와
`CHANGELOG.md`에 credit됩니다.
