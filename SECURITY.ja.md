# セキュリティポリシー

[English](./SECURITY.md) | [简体中文](./SECURITY.zh-CN.md) | [繁體中文](./SECURITY.zh-Hant.md) | 日本語 | [한국어](./SECURITY.ko.md)

## サポート対象バージョン

PKV Sync は v1.0.0 から semantic versioning に従います。セキュリティ修正は、
現在の 1.x minor line と直前の 1.x minor line を対象に維持します。

| Version | Status | セキュリティサポート終了 |
| --- | --- | --- |
| 最新の 1.x minor | Active | TBD |
| 直前の 1.x minor | Security-fix only | 次の 1.x minor が出るまで |
| 0.x | Not supported | v1.0.0 release |

## 脆弱性の報告

セキュリティ脆弱性を公開 GitHub issue に投稿しないでください。

推奨窓口：`cyberkurry/pkv-sync` の GitHub Security Advisories から private report を開いてください。

報告には次を含めてください。

- 影響を受ける PKV Sync version。
- 最小限の再現手順。
- 影響範囲の評価。
- 修正案があればその内容。

## 対応目標

- 初回 acknowledgement：5 business days。
- severity triage：10 business days。
- 修正と coordinated disclosure：critical/high は 90 days、medium/low は 180 days。
- CVE assignment：該当する場合は GitHub Security Advisories 経由。

## Scope

対象：

- `pkvsyncd` server binary。
- Obsidian plugin。
- Admin Web UI。
- MCP stdio transport と Streamable HTTP transport。
- 安全でない deployment を推奨している公開ドキュメント。

対象外：

- PKV Sync の外側にある host、reverse proxy、TLS、Docker、systemd、OS hardening。
- third-party Obsidian plugin。
- PKV Sync の使い方では exploit できない dependency vulnerability。
- すでに侵害された host の administrator 権限を必要とする報告。

## 既知の非問題

- PKV Sync 1.0 は通常の vault contents を server-side plaintext として保存します。
  これは design choice です。vault 単位の native E2EE は 1.x roadmap で計画されています。
  今すぐ client-side encryption が必要な場合は
  [`git-crypt`](./public-docs/git-crypt-howto.ja.md) を使えます。trade-off は README に記載しています。
- `/metrics` は default で disabled です。有効化しても production server stack では
  deployment key middleware、PKV Sync User-Agent guard、admin bearer token が必要です。
- MCP HTTP は deployment key と bearer token authentication の両方を要求します。
  public に公開する場合も、authenticated admin-adjacent surface として保護してください。

## Disclosure

PKV Sync は coordinated disclosure に従います。匿名を希望しない限り、reporter は advisory と
`CHANGELOG.md` で credit されます。
