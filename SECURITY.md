# Security Policy

English | [简体中文](./SECURITY.zh-CN.md) | [繁體中文](./SECURITY.zh-Hant.md) | [日本語](./SECURITY.ja.md) | [한국어](./SECURITY.ko.md)

## Supported Versions

PKV Sync follows semantic versioning starting at v1.0.0. Security fixes are
maintained for the current and previous minor release lines.

| Version | Status | End of security support |
| --- | --- | --- |
| Latest 1.x minor | Active | TBD |
| Previous 1.x minor | Security-fix only | When the next 1.x minor ships |
| 0.x | Not supported | v1.0.0 release |

## Reporting a Vulnerability

Do not open a public GitHub issue for security vulnerabilities.

Preferred channel: open a private report through GitHub Security Advisories for
`cyberkurry/pkv-sync`.

Include:

- Affected PKV Sync version.
- Minimal reproduction steps.
- Impact assessment.
- Suggested fix, if you have one.

## Response Targets

- Initial acknowledgement: 5 business days.
- Severity triage: 10 business days.
- Fix and coordinated disclosure: 90 days for critical/high issues, 180 days
  for medium/low issues.
- CVE assignment: through GitHub Security Advisories when applicable.

## Scope

In scope:

- `pkvsyncd` server binary.
- Obsidian plugin.
- Admin Web UI.
- MCP stdio and Streamable HTTP transports.
- Public documentation when it recommends insecure deployment.

Out of scope:

- Host, reverse proxy, TLS, Docker, systemd, and OS hardening outside PKV Sync.
- Third-party Obsidian plugins.
- Dependency vulnerabilities that are not exploitable through PKV Sync's use of
  the dependency.
- Reports that require administrator access to an already compromised host.

## Known Non-Issues

- PKV Sync 1.0 stores normal vault contents in plaintext on the server by
  design. Native per-vault E2EE is planned for the 1.x roadmap. Users needing
  client-side encryption today can use
  [`git-crypt`](./public-docs/git-crypt-howto.md), with the trade-offs
  described in the README.
- `/metrics` is disabled by default. When enabled, it still requires the
  deployment key middleware, accepted PKV Sync User-Agent, and an admin bearer
  token in the production server stack.
- MCP HTTP requires both the deployment key and bearer token authentication;
  exposing it publicly is still an operator choice and should be protected like
  any other authenticated admin-adjacent surface.

## Disclosure

PKV Sync follows coordinated disclosure. Reporters are credited in the advisory
and `CHANGELOG.md` unless they prefer anonymity.
