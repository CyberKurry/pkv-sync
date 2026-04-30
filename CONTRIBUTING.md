# Contributing to PKV Sync

PKV Sync is early-stage. Please open an issue before large changes.

## Development

```bash
cargo test -p pkv-sync-server
npm --prefix plugin test
npm --prefix plugin run typecheck
```

## Commits

Use clear conventional-style commit messages, for example:

- `feat(server): add token auth extractor`
- `fix(plugin): preserve local conflict file`
- `docs: update nginx deployment guide`

## License

By contributing, you agree that your contribution is licensed under AGPL-3.0-only.
