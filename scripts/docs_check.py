#!/usr/bin/env python3
"""Guard the public docs against encoding corruption and version drift.

The 1.4.4 release bump silently double-encoded every language-switcher row and
em dash in the English `public-docs/*.md` files (UTF-8 bytes re-read as GBK and
written back), which shipped unreadable text for three releases. This check
fails CI instead of relying on a reviewer noticing.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS_DIR = ROOT / "public-docs"

LANGUAGES = {
    "en": None,
    "zh-CN": "简体中文",
    "zh-Hant": "繁體中文",
    "ja": "日本語",
    "ko": "한국어",
}

# Mojibake witnesses: `鈥` is the UTF-8 bytes of an em dash read as GBK; the
# others are the four language labels from the switcher row.
MOJIBAKE = ("鈥", "绠€", "绻侀", "鏃ユ", "頃滉", "\ufffd")

VERSION_LINE = re.compile(r"v(\d+\.\d+\.\d+)")
LINK = re.compile(r"\]\(([^)]+)\)")


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not match:
        raise SystemExit("docs check: could not read workspace version from Cargo.toml")
    return match.group(1)


def doc_bases() -> list[str]:
    bases = set()
    for path in DOCS_DIR.glob("*.md"):
        stem = path.name[: -len(".md")]
        for suffix in LANGUAGES:
            if suffix != "en" and stem.endswith("." + suffix):
                stem = stem[: -len(suffix) - 1]
                break
        bases.add(stem)
    return sorted(bases)


def english_docs() -> list[Path]:
    """Docs that carry a `Document version` header (SECURITY*.md do not)."""
    return sorted([ROOT / "README.md"] + [DOCS_DIR / f"{base}.md" for base in doc_bases()])


def translated_docs() -> list[Path]:
    """Translated docs that mirror an English `Document version` header."""
    docs = [ROOT / f"README.{suffix}.md" for suffix in LANGUAGES if suffix != "en"]
    for base in doc_bases():
        docs += [DOCS_DIR / f"{base}.{suffix}.md" for suffix in LANGUAGES if suffix != "en"]
    return sorted(path for path in docs if path.exists())


def all_docs() -> list[Path]:
    return sorted(
        list(ROOT.glob("README*.md"))
        + list(ROOT.glob("SECURITY*.md"))
        + list(DOCS_DIR.glob("*.md"))
        + [DOCS_DIR / "openapi.yaml"]
    )


def check_encoding(failures: list[str]) -> None:
    for path in all_docs():
        raw = path.read_bytes()
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError as error:
            failures.append(f"{path.relative_to(ROOT)}: not valid UTF-8 ({error})")
            continue
        for witness in MOJIBAKE:
            if witness in text:
                failures.append(
                    f"{path.relative_to(ROOT)}: mojibake {witness!r} present "
                    "(file was double-encoded or truncated)"
                )


def check_switcher_rows(failures: list[str]) -> None:
    for path in english_docs():
        text = path.read_text(encoding="utf-8")
        base = path.stem
        for suffix, label in LANGUAGES.items():
            if suffix == "en":
                continue
            target = f"{base}.{suffix}.md" if base != "README" and base != "SECURITY" else f"{base}.{suffix}.md"
            if not (path.parent / target).exists():
                continue
            if f"[{label}](./{target})" not in text:
                failures.append(
                    f"{path.relative_to(ROOT)}: language switcher row is missing "
                    f"[{label}](./{target})"
                )


def check_language_coverage(failures: list[str]) -> None:
    for base in doc_bases():
        for suffix in LANGUAGES:
            name = f"{base}.md" if suffix == "en" else f"{base}.{suffix}.md"
            if not (DOCS_DIR / name).exists():
                failures.append(f"public-docs/{name}: missing translation")


def check_versions(failures: list[str], version: str) -> None:
    for path in english_docs():
        head = "\n".join(path.read_text(encoding="utf-8").splitlines()[:12])
        found = VERSION_LINE.search(head)
        if not found:
            failures.append(f"{path.relative_to(ROOT)}: no Document version header")
        elif found.group(1) != version:
            failures.append(
                f"{path.relative_to(ROOT)}: Document version v{found.group(1)} "
                f"!= workspace {version}"
            )

    for path in translated_docs():
        head = "\n".join(path.read_text(encoding="utf-8").splitlines()[:16])
        found = VERSION_LINE.search(head)
        if not found:
            failures.append(f"{path.relative_to(ROOT)}: no Document version header")
        elif found.group(1) != version:
            failures.append(
                f"{path.relative_to(ROOT)}: Document version v{found.group(1)} "
                f"!= workspace {version}"
            )

    openapi = (DOCS_DIR / "openapi.yaml").read_text(encoding="utf-8")
    match = re.search(r"^  version:\s*(\S+)\s*$", openapi, re.MULTILINE)
    if not match:
        failures.append("public-docs/openapi.yaml: no info.version")
    elif match.group(1) != version:
        failures.append(
            f"public-docs/openapi.yaml: info.version {match.group(1)} "
            f"!= workspace {version}"
        )


def check_links(failures: list[str]) -> None:
    for path in all_docs():
        if path.suffix != ".md":
            continue
        text = path.read_text(encoding="utf-8")
        for match in LINK.finditer(text):
            target = match.group(1)
            if target.startswith(("http", "mailto:", "#")):
                continue
            if not target.split("#")[0]:
                continue
            resolved = (path.parent / target.split("#")[0]).resolve()
            if not resolved.exists():
                failures.append(
                    f"{path.relative_to(ROOT)}: broken link {target}"
                )


def main() -> int:
    version = workspace_version()
    failures: list[str] = []

    check_encoding(failures)
    check_switcher_rows(failures)
    check_language_coverage(failures)
    check_versions(failures, version)
    check_links(failures)

    if failures:
        print("Public doc check failed:", file=sys.stderr)
        for failure in failures:
            print(f"  {failure}", file=sys.stderr)
        return 1

    print(
        f"public doc check passed: {len(doc_bases())} doc sets, "
        f"5 languages, all aligned to v{version}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
