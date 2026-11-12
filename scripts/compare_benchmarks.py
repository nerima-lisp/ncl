#!/usr/bin/env python3
"""Compare two NCL binaries with interleaved compile-file measurements."""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import statistics
import sys
from typing import Any, Callable, Sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from bench.corpus.compile import BenchmarkError, run_benchmark, _safe_diagnostic
from bench.corpus.fetch import CorpusError
from bench.statistics import compare_statistics, sample_statistics
from performance_summary import render_ab_comparison


class ComparisonError(RuntimeError):
    """A comparison did not produce matching, complete benchmark samples."""


def _file_times(report: dict[str, Any]) -> dict[str, float]:
    if report.get("status") != "complete":
        raise ComparisonError("compile benchmark report is incomplete")
    runtimes = report.get("runtimes")
    ncl = runtimes.get("ncl") if isinstance(runtimes, dict) else None
    files = ncl.get("files") if isinstance(ncl, dict) else None
    if not isinstance(files, list):
        raise ComparisonError("compile benchmark report has no NCL file results")
    corpus_count = report.get("corpus_file_count")
    if (
        not isinstance(corpus_count, int)
        or isinstance(corpus_count, bool)
        or corpus_count != len(files)
        or ncl.get("file_count") != len(files)
    ):
        raise ComparisonError("compile benchmark report has an incomplete source denominator")
    if ncl.get("failed_file_count") != 0 or ncl.get("compiled_file_count") != len(files):
        raise ComparisonError("compile benchmark contains a failed source")
    values: dict[str, float] = {}
    for row in files:
        if not isinstance(row, dict):
            raise ComparisonError("compile benchmark file result is invalid")
        label = f"{row.get('suite')}/{row.get('file')}"
        seconds = row.get("compile_seconds")
        if row.get("status") != "passed":
            raise ComparisonError("compile benchmark contains a failed source")
        if not isinstance(seconds, (int, float)) or isinstance(seconds, bool):
            raise ComparisonError("compile benchmark contains an invalid duration")
        if not math.isfinite(float(seconds)) or seconds < 0:
            raise ComparisonError("compile benchmark contains an invalid duration")
        if label in values:
            raise ComparisonError("compile benchmark contains a duplicate source")
        values[label] = float(seconds)
    if not values:
        raise ComparisonError("no source compiled successfully in the comparison sample")
    return values


def _sum_common(values: dict[str, float], expected: set[str], side: str) -> float:
    if set(values) != expected:
        raise ComparisonError(f"{side} compiled a different corpus file set")
    return sum(values.values())


def run_comparison(
    baseline_ncl: str | Path,
    candidate_ncl: str | Path,
    sbcl: str | Path,
    *,
    mode: str = "light",
    runs: int = 5,
    threshold: float = 0.10,
    benchmark: Callable[..., dict[str, Any]] = run_benchmark,
    cache_dir: Path | None = None,
    timeout: float = 60.0,
) -> dict[str, Any]:
    if runs < 5:
        raise ValueError("A/B comparison requires at least five runs per binary")
    if mode not in {"light", "full"}:
        raise ValueError("mode must be light or full")
    if not math.isfinite(threshold) or threshold < 0:
        raise ValueError("threshold must be a finite nonnegative number")
    identities = {"baseline": str(baseline_ncl), "candidate": str(candidate_ncl)}
    measurements: dict[str, list[float]] = {"baseline": [], "candidate": []}
    file_measurements: dict[str, dict[str, list[float]]] = {"baseline": {}, "candidate": {}}
    corpus: set[str] | None = None
    order: list[str] = []
    for index in range(runs):
        sides = ("baseline", "candidate") if index % 2 == 0 else ("candidate", "baseline")
        for side in sides:
            report = benchmark(
                identities[side],
                sbcl,
                mode,
                sample_runs=1,
                cache_dir=cache_dir,
                timeout=timeout,
            )
            values = _file_times(report)
            if corpus is None:
                corpus = set(values)
            measurements[side].append(_sum_common(values, corpus, side))
            for label, seconds in values.items():
                file_measurements[side].setdefault(label, []).append(seconds)
            order.append(side)
    if corpus is None or not corpus:
        raise ComparisonError("comparison produced no measurements")
    baseline = measurements["baseline"]
    candidate = measurements["candidate"]
    baseline_median = statistics.median(baseline)
    candidate_median = statistics.median(candidate)
    baseline_minimum = min(baseline)
    candidate_minimum = min(candidate)
    baseline_statistics = sample_statistics(baseline)
    candidate_statistics = sample_statistics(candidate)
    comparison = compare_statistics(candidate_statistics, baseline_statistics)
    file_comparisons = {}
    for label in sorted(corpus):
        baseline_file = sample_statistics(file_measurements["baseline"][label])
        candidate_file = sample_statistics(file_measurements["candidate"][label])
        file_comparisons[label] = {
            "baseline": {
                **baseline_file,
                "samples_seconds": file_measurements["baseline"][label],
            },
            "candidate": {
                **candidate_file,
                "samples_seconds": file_measurements["candidate"][label],
            },
            "comparison": compare_statistics(candidate_file, baseline_file),
        }
    median_regression = candidate_median >= baseline_median * (1 + threshold)
    minimum_regression = candidate_minimum >= baseline_minimum * (1 + threshold)
    regression = median_regression and minimum_regression
    return {
        "schema_version": 1,
        "status": "regression" if regression else "no-regression",
        "mode": mode,
        "runs_per_binary": runs,
        "sample_runs_per_compile_trial": 1,
        "warmup_runs_per_compile_trial": 0,
        "process_topology": "fresh process per source and trial",
        "regression_threshold_ratio": 1 + threshold,
        "baseline": {
            "samples_seconds": baseline,
            **baseline_statistics,
        },
        "candidate": {
            "samples_seconds": candidate,
            **candidate_statistics,
        },
        "comparison": comparison,
        "file_comparisons": file_comparisons,
        "noise_width_definition": (
            "absolute candidate-minus-baseline median difference compared with "
            "the larger of the two max-minus-min sample ranges"
        ),
        "checks": {
            "median_at_least_threshold_slower": median_regression,
            "minimum_at_least_threshold_slower": minimum_regression,
            "regression": regression,
        },
        "interleaving_order": order,
        "compared_file_count": len(corpus),
    }


def _positive_int(value: str) -> int:
    try:
        parsed = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-ncl", required=True, help="baseline NCL executable")
    parser.add_argument("--candidate-ncl", required=True, help="candidate NCL executable")
    parser.add_argument("--sbcl", default="sbcl", help="SBCL 2.6.10 executable")
    parser.add_argument("--mode", choices=("light", "full"), default="light")
    parser.add_argument("--runs", type=_positive_int, default=5, help="runs for each NCL binary (minimum 5)")
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    parser.add_argument("--output", type=Path, help="write the result to this path")
    parser.add_argument("--cache-dir", type=Path, default=REPO_ROOT / "target/bench-corpus-cache")
    parser.add_argument("--timeout", type=float, default=60.0, help="seconds allowed per Lisp process")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    if args.runs < 5:
        parser.error("--runs must be at least five")
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be a finite positive number")
    try:
        report = run_comparison(
            args.baseline_ncl,
            args.candidate_ncl,
            args.sbcl,
            mode=args.mode,
            runs=args.runs,
            cache_dir=args.cache_dir,
            timeout=args.timeout,
        )
    except (BenchmarkError, CorpusError, ComparisonError, OSError, ValueError) as error:
        report = {"schema_version": 1, "status": "error", "error": _safe_diagnostic(str(error))}
        exit_status = 2
    else:
        exit_status = 1 if report["status"] == "regression" else 0
    rendered = (
        render_ab_comparison(report) + "\n"
        if args.format == "markdown"
        else json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    )
    if args.output is None:
        sys.stdout.write(rendered)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8", newline="\n")
    return exit_status


if __name__ == "__main__":
    raise SystemExit(main())
