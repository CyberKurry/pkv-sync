# Contributing to PKV Sync

PKV Sync is early-stage. Please open an issue before large changes.

## Development

```bash
cargo test -p pkv-sync-server
(cd plugin && npm test)
(cd plugin && npm run typecheck)
```

## Git hooks

The repo ships a `pre-commit` hook in `.githooks/`. Enable it once per clone:

```bash
git config core.hooksPath .githooks
```

It runs the same checks as CI's "Plugin checks" job, but only when your staged
change actually touches what they cover:

| Staged paths | Check |
| --- | --- |
| `README*.md`, `SECURITY*.md`, `CHANGELOG.md`, `public-docs/` | `python3 scripts/docs_check.py` |
| `plugin/src/i18n/` | `python3 scripts/i18n_check.py` |

Anything else skips the hook entirely. Bypass once with `git commit --no-verify`.

### Skipping CI on docs-only commits

A docs-only commit does not need the full matrix (Rust on two platforms, Docker
build, coverage gate). Put `[skip ci]` in the commit subject to skip it:

```bash
git commit -m "docs: fix zh-Hant terminology [skip ci]"
```

The hook prints a reminder when it sees a docs-only change.

**One exception:** never put `[skip ci]` on a release doc-alignment commit (the
one that bumps `Document version` in the READMEs and `public-docs/`). GitHub
attaches the skip to the tagged commit, so `git tag` would skip the Release
workflow too — no binaries, no plugin zip, no Docker image. The hook warns when
the staged change looks like that commit.

## Commits

Use clear conventional-style commit messages, for example:

- `feat(server): add token auth extractor`
- `fix(plugin): preserve local conflict file`
- `docs: update nginx deployment guide`

## License

By contributing, you agree that your contribution is licensed under AGPL-3.0-only.
