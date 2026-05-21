#!/usr/bin/env python3
"""Gate Rust and plugin coverage against a tracked markdown baseline."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_MAX_DROP = 5.0
PERCENT_RE = re.compile(r"(-?\d+(?:\.\d+)?)\s*%?")


@dataclass(frozen=True)
class CoverageMetric:
    name: str
    pct: float


def parse_percent(value: str) -> float:
    match = PERCENT_RE.search(value.replace("**", ""))
    if not match:
        raise ValueError(f"could not parse percentage from {value!r}")
    return float(match.group(1))


def normalize_name(value: str) -> str:
    return re.sub(r"\s+", " ", value.strip().lower())


def read_baseline(path: Path) -> dict[str, float]:
    metrics: dict[str, float] = {}
    in_table = False

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line.startswith("|") or not line.endswith("|"):
            if in_table and metrics:
                break
            continue

        cells = [cell.strip() for cell in line.strip("|").split("|")]
        lowered = [normalize_name(cell) for cell in cells]
        if any("baseline" in cell for cell in lowered) and any("area" in cell or "component" in cell for cell in lowered):
            in_table = True
            continue
        if not in_table or all(set(cell) <= {"-", ":"} for cell in cells):
            continue
        if len(cells) < 2:
            continue

        metrics[normalize_name(cells[0])] = parse_percent(cells[-1])

    if not metrics:
        raise ValueError(f"no coverage baseline table found in {path}")
    return metrics


def numeric_values(data: Any) -> list[float]:
    if isinstance(data, dict):
        values: list[float] = []
        for value in data.values():
            values.extend(numeric_values(value))
        return values
    if isinstance(data, list):
        values = []
        for value in data:
            values.extend(numeric_values(value))
        return values
    if isinstance(data, (int, float)) and not isinstance(data, bool):
        return [float(data)]
    return []


def ratio_to_percent(covered: float, total: float) -> float | None:
    if total <= 0:
        return None
    return covered / total * 100.0


def read_tarpaulin(path: Path) -> float:
    data = json.loads(path.read_text(encoding="utf-8-sig"))

    for key in ("line_coverage", "coverage"):
        if isinstance(data, dict) and isinstance(data.get(key), (int, float)):
            value = float(data[key])
            return value * 100.0 if value <= 1.0 else value

    if isinstance(data, dict):
        if "covered" in data and "coverable" in data:
            pct = ratio_to_percent(float(data["covered"]), float(data["coverable"]))
            if pct is not None:
                return pct

        totals = data.get("totals")
        if isinstance(totals, dict):
            for covered_key, total_key in (
                ("covered_lines", "coverable_lines"),
                ("covered", "coverable"),
                ("hits", "lines"),
            ):
                if covered_key in totals and total_key in totals:
                    pct = ratio_to_percent(float(totals[covered_key]), float(totals[total_key]))
                    if pct is not None:
                        return pct

        files = data.get("files")
        if isinstance(files, dict):
            covered = 0.0
            coverable = 0.0
            for file_data in files.values():
                if not isinstance(file_data, dict):
                    continue
                if "covered_lines" in file_data and "coverable_lines" in file_data:
                    covered += float(file_data["covered_lines"])
                    coverable += float(file_data["coverable_lines"])
                elif "covered" in file_data and "coverable" in file_data:
                    covered += float(file_data["covered"])
                    coverable += float(file_data["coverable"])
            pct = ratio_to_percent(covered, coverable)
            if pct is not None:
                return pct

    raise ValueError(f"could not derive Rust line coverage from {path}")


def read_vitest_summary(path: Path) -> float:
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(data, dict):
        raise ValueError(f"Vitest coverage summary must be a JSON object: {path}")

    total = data.get("total")
    if isinstance(total, dict):
        lines = total.get("lines")
        if isinstance(lines, dict):
            if isinstance(lines.get("pct"), (int, float)):
                return float(lines["pct"])
            if "covered" in lines and "total" in lines:
                pct = ratio_to_percent(float(lines["covered"]), float(lines["total"]))
                if pct is not None:
                    return pct

    raise ValueError(f"could not derive plugin line coverage from {path}")


def compare(actual: list[CoverageMetric], baseline: dict[str, float], max_drop: float) -> list[str]:
    failures: list[str] = []
    for metric in actual:
        key = normalize_name(metric.name)
        if key not in baseline:
            failures.append(f"{metric.name}: missing from baseline")
            continue
        allowed = baseline[key] - max_drop
        if metric.pct + 1e-9 < allowed:
            failures.append(
                f"{metric.name}: {metric.pct:.2f}% is below {baseline[key]:.2f}% baseline "
                f"by more than {max_drop:.2f} points"
            )
    return failures


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Fail when Rust tarpaulin or plugin Vitest coverage drops below the markdown baseline."
    )
    parser.add_argument("--baseline", required=True, type=Path, help="Markdown file containing the baseline table.")
    parser.add_argument("--tarpaulin-json", required=True, type=Path, help="Rust tarpaulin JSON report.")
    parser.add_argument(
        "--plugin-summary",
        required=True,
        type=Path,
        help="Vitest coverage-summary.json report.",
    )
    parser.add_argument(
        "--max-drop",
        default=DEFAULT_MAX_DROP,
        type=float,
        help="Allowed coverage drop in percentage points per component.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    baseline = read_baseline(args.baseline)
    actual = [
        CoverageMetric("Rust server", read_tarpaulin(args.tarpaulin_json)),
        CoverageMetric("Obsidian plugin", read_vitest_summary(args.plugin_summary)),
    ]
    failures = compare(actual, baseline, args.max_drop)

    for metric in actual:
        print(f"{metric.name}: {metric.pct:.2f}% (baseline {baseline.get(normalize_name(metric.name), 0):.2f}%)")

    if failures:
        print("Coverage gate failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(f"Coverage gate passed with max drop {args.max_drop:.2f} percentage points.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
