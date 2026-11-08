#!/usr/bin/env python3
"""Compare LLVM region coverage for paths changed from a parent summary."""
from __future__ import annotations

import argparse
import html
import json
import math
import os
import sys
from dataclasses import dataclass
from pathlib import Path

THRESHOLD = 95.0


class CoverageInputError(ValueError):
    pass


@dataclass(frozen=True)
class RegionStats:
    count: int = 0
    covered: int = 0
    missed: int = 0

    @property
    def percent(self) -> float:
        if self.count == 0:
            return 100.0
        return self.covered * 100.0 / self.count

    def __add__(self, other: RegionStats) -> RegionStats:
        return RegionStats(
            self.count + other.count,
            self.covered + other.covered,
            self.missed + other.missed,
        )


@dataclass(frozen=True)
class CoverageSummary:
    paths: dict[str, RegionStats]
    totals: RegionStats


def _regions(value: object, context: str) -> RegionStats:
    if not isinstance(value, dict):
        raise CoverageInputError(f"{context} has no region summary")
    numbers = []
    for field in ("count", "covered", "notcovered"):
        number = value.get(field)
        if isinstance(number, bool) or not isinstance(number, int) or number < 0:
            raise CoverageInputError(f"{context} has invalid regions.{field}")
        numbers.append(number)
    percent = value.get("percent")
    if isinstance(percent, bool) or not isinstance(percent, (int, float)):
        raise CoverageInputError(f"{context} has invalid regions.percent")
    if (
        percent < 0.0
        or percent > 100.0
        or (isinstance(percent, float) and not math.isfinite(percent))
    ):
        raise CoverageInputError(f"{context} has invalid regions.percent")
    count, covered, missed = numbers
    if count != covered + missed:
        raise CoverageInputError(f"{context} has inconsistent region counts")
    return RegionStats(count, covered, missed)


def _relative_path(filename: str, root: Path) -> str | None:
    source_root = Path(os.path.abspath(root))
    candidate = Path(filename)
    if not candidate.is_absolute():
        candidate = source_root / candidate
    try:
        normalized = Path(os.path.normpath(candidate))
        return normalized.relative_to(source_root).as_posix()
    except ValueError:
        return None


def parse_summary(payload: object, root: Path, source: str = "coverage summary") -> CoverageSummary:
    if not isinstance(payload, dict) or not isinstance(payload.get("data"), list) or not payload["data"]:
        raise CoverageInputError(f"{source} has no data entries")

    paths: dict[str, RegionStats] = {}
    totals = RegionStats()
    for index, dataset in enumerate(payload["data"]):
        if not isinstance(dataset, dict) or not isinstance(dataset.get("files"), list):
            raise CoverageInputError(f"{source} data[{index}] has no file list")
        totals = totals + _regions(
            dataset.get("totals", {}).get("regions")
            if isinstance(dataset.get("totals"), dict)
            else None,
            f"{source} data[{index}].totals",
        )
        for file_index, entry in enumerate(dataset["files"]):
            if not isinstance(entry, dict) or not isinstance(entry.get("filename"), str):
                raise CoverageInputError(
                    f"{source} data[{index}].files[{file_index}] has no filename"
                )
            summary = entry.get("summary")
            regions = summary.get("regions") if isinstance(summary, dict) else None
            stats = _regions(
                regions,
                f"{source} data[{index}].files[{file_index}]",
            )
            relative = _relative_path(entry["filename"], root)
            if relative is not None:
                paths[relative] = paths.get(relative, RegionStats()) + stats
    return CoverageSummary(paths, totals)


def load_summary(path: Path, root: Path) -> CoverageSummary:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CoverageInputError(f"cannot read coverage summary {path}: {error}") from error
    return parse_summary(payload, root, f"coverage summary {path}")


def normalize_changed_paths(paths: list[str], root: Path) -> list[str]:
    normalized = []
    seen = set()
    for path in paths:
        if not path:
            continue
        relative = _relative_path(path, root)
        if relative is not None and relative not in seen:
            normalized.append(relative)
            seen.add(relative)
    return normalized


def changed_paths(path: Path, root: Path) -> list[str]:
    try:
        raw_paths = path.read_bytes().split(b"\0")
    except OSError as error:
        raise CoverageInputError(f"cannot read changed path list {path}: {error}") from error
    return normalize_changed_paths([os.fsdecode(raw) for raw in raw_paths], root)


def _sum(stats: list[RegionStats]) -> RegionStats:
    total = RegionStats()
    for item in stats:
        total = total + item
    return total


def _passes(stats: RegionStats, parent: RegionStats) -> bool:
    return stats.missed <= parent.missed or stats.percent >= THRESHOLD


def _display(stats: RegionStats) -> str:
    if stats.count == 0:
        return "0/0 regions (coverage n/a)"
    return f"{stats.missed}/{stats.count} missed ({stats.percent:.2f}% covered)"


def _display_path(path: str) -> str:
    encoded = json.dumps(path, ensure_ascii=True)
    return html.escape(encoded, quote=True).replace("`", "&#96;").replace("|", "&#124;")


def evaluate(
    candidate: CoverageSummary,
    parent: CoverageSummary,
    paths: list[str],
) -> tuple[bool, bool, RegionStats, RegionStats, list[tuple[str, RegionStats, RegionStats, bool | None]]]:
    candidate_group = _sum([candidate.paths.get(path, RegionStats()) for path in paths])
    parent_group = _sum([parent.paths.get(path, RegionStats()) for path in paths])
    aggregate_pass = _passes(candidate_group, parent_group)
    per_path = []
    for path in paths:
        if path not in parent.paths:
            per_path.append((path, candidate.paths.get(path, RegionStats()), RegionStats(), None))
            continue
        candidate_stats = candidate.paths.get(path, RegionStats())
        parent_stats = parent.paths[path]
        per_path.append((path, candidate_stats, parent_stats, _passes(candidate_stats, parent_stats)))
    path_failures = [entry for entry in per_path if entry[3] is False]
    return aggregate_pass, not path_failures, candidate_group, parent_group, per_path


def _print_report(
    candidate: CoverageSummary,
    parent: CoverageSummary,
    aggregate_pass: bool,
    per_path_pass: bool,
    candidate_group: RegionStats,
    parent_group: RegionStats,
    per_path: list[tuple[str, RegionStats, RegionStats, bool | None]],
) -> None:
    print("Changed paths (paths absent from summaries have zero regions):")
    print("| Path | Candidate | Parent | Per-path ratchet |")
    print("| --- | --- | --- | --- |")
    if not per_path:
        print("| (none) | 0/0 regions (coverage n/a) | 0/0 regions (coverage n/a) | not applicable |")
    for path, current, previous, result in per_path:
        shown_path = _display_path(path)
        if result is None:
            status = "not in parent summary; per-path rule not applicable"
        else:
            status = "PASS" if result else "FAIL"
        print(f"| {shown_path} | {_display(current)} | {_display(previous)} | {status} |")
    print(
        "(a) aggregate changed-path ratchet: "
        f"{'PASS' if aggregate_pass else 'FAIL'} "
        f"(candidate {_display(candidate_group)}; parent {_display(parent_group)})"
    )
    print(
        "(b) parent-path ratchet: "
        f"{'PASS' if per_path_pass else 'FAIL'}"
    )
    print(
        "Workspace region coverage: "
        f"{candidate.totals.covered}/{candidate.totals.count} "
        f"({candidate.totals.percent:.2f}%; 95.00% goal)"
    )
    print(
        "Parent workspace region coverage: "
        f"{parent.totals.covered}/{parent.totals.count} "
        f"({parent.totals.percent:.2f}%)"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--parent", type=Path, required=True)
    parser.add_argument("--changed-paths", type=Path, required=True, help="NUL-delimited git diff path list")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)

    try:
        candidate = load_summary(args.candidate, args.root)
        parent = load_summary(args.parent, args.root)
        paths = changed_paths(args.changed_paths, args.root)
    except CoverageInputError as error:
        print(f"coverage check error: {error}", file=sys.stderr)
        return 2

    aggregate_pass, per_path_pass, candidate_group, parent_group, per_path = evaluate(
        candidate, parent, paths
    )
    _print_report(
        candidate,
        parent,
        aggregate_pass,
        per_path_pass,
        candidate_group,
        parent_group,
        per_path,
    )
    return 0 if aggregate_pass and per_path_pass else 1


if __name__ == "__main__":
    raise SystemExit(main())
