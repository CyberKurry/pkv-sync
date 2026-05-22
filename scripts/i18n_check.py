#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
I18N_DIR = ROOT / "plugin" / "src" / "i18n"
LANG_FILES = {
    "en": I18N_DIR / "en.ts",
    "zh": I18N_DIR / "zh.ts",
    "zhHant": I18N_DIR / "zh-Hant.ts",
    "ja": I18N_DIR / "ja.ts",
    "ko": I18N_DIR / "ko.ts",
}
KEY_RE = re.compile(r"^\s{2}([A-Za-z][A-Za-z0-9_]*):", re.MULTILINE)
SPREAD_RE = re.compile(r"^\s{2}\.\.\.([A-Za-z][A-Za-z0-9_]*),", re.MULTILINE)


def raw_parts(path: Path) -> tuple[set[str], list[str]]:
    content = path.read_text(encoding="utf-8")
    return set(KEY_RE.findall(content)), SPREAD_RE.findall(content)


def resolve_keys(
    name: str,
    raw: dict[str, tuple[set[str], list[str]]],
    resolving: set[str] | None = None,
) -> set[str]:
    resolving = resolving or set()
    if name in resolving:
        raise RuntimeError(f"circular i18n spread involving {name}")
    keys, spreads = raw[name]
    resolved = set(keys)
    for spread in spreads:
        if spread not in raw:
            raise RuntimeError(f"{name} spreads unknown bundle {spread}")
        resolved.update(resolve_keys(spread, raw, resolving | {name}))
    return resolved


def main() -> int:
    missing_files = [str(path) for path in LANG_FILES.values() if not path.exists()]
    if missing_files:
        print("Missing i18n files:", file=sys.stderr)
        for path in missing_files:
            print(f"  {path}", file=sys.stderr)
        return 1

    raw = {name: raw_parts(path) for name, path in LANG_FILES.items()}
    key_sets = {name: resolve_keys(name, raw) for name in LANG_FILES}
    expected = key_sets["en"]
    ok = True

    for name, keys in key_sets.items():
        missing = sorted(expected - keys)
        extra = sorted(keys - expected)
        if missing:
            ok = False
            print(f"{name} is missing keys:", file=sys.stderr)
            for key in missing:
                print(f"  {key}", file=sys.stderr)
        if extra:
            print(f"{name} has extra keys:", file=sys.stderr)
            for key in extra:
                print(f"  {key}", file=sys.stderr)

    if not ok:
        return 1
    print(f"i18n check passed for {len(key_sets)} plugin languages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
