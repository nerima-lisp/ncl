#!/usr/bin/env python3
"""Measure paired NCL optimization modes and a pinned SBCL baseline."""

from __future__ import annotations

import argparse
import json
import platform
import re
import subprocess
import sys
import time
from datetime import date
from pathlib import Path
from typing import Any, Sequence

import conformance_scoreboard as scoreboard
from optimization_baseline_constants import (
    BENCHMARKS, DISABLED_BENCHMARK, DISABLED_BENCHMARK_REASONS,
    DRIVER_SUPERVISOR, INFO_TIMEOUT, MAX_BENCHMARK_TIMES, MAX_ERROR_CHARS,
    MAX_FLAT_TIMES, MAX_RUNNER_OUTPUT_CHARS, NCL_DRIVER, REPO_ROOT,
    RUNNER_STATUSES, SBCL_DRIVER, SBCL_VERSION,
)
from optimization_baseline_records import (
    _benchmark_record, _driver_data, _failed_benchmark, _finite_number,
    _normalize_times, _parse_json_object, _redact, _safe_error,
)
from optimization_baseline_scoring import (
    _median_for_benchmark, _report_status, _run_is_clean, _score_runs,
)


def _positive_count(value: str) -> int:
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive integer") from error
    if count < 1:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return count


def _paired_count(value: str) -> int:
    count = _positive_count(value)
    if count < 5:
        raise argparse.ArgumentTypeError("at least 5 samples are required")
    return count


def _revision_sha(value: str) -> str:
    if not re.fullmatch(r"[0-9a-f]{40,64}", value):
        raise ValueError("must be a 40- to 64-character lowercase commit SHA")
    return value


def _round_schedule(ncl_pairs: int, sbcl_samples: int) -> list[list[dict[str, Any]]]:
    rotations = (
        (("ncl", "off"), ("sbcl", None), ("ncl", "default")),
        (("sbcl", None), ("ncl", "default"), ("ncl", "off")),
        (("ncl", "default"), ("ncl", "off"), ("sbcl", None)),
    )
    rounds = []
    for number in range(1, max(ncl_pairs, sbcl_samples) + 1):
        current = []
        for engine, mode in rotations[(number - 1) % len(rotations)]:
            if engine == "ncl" and number > ncl_pairs:
                continue
            if engine == "sbcl" and number > sbcl_samples:
                continue
            sample = {"engine": engine}
            if mode is not None:
                sample["mode"] = mode
                sample["pair"] = number
            else:
                sample["sample"] = number
            current.append(sample)
        rounds.append(current)
    return rounds


def _select_benchmarks(names: Sequence[str] | None) -> tuple[str, ...]:
    if names is None:
        return BENCHMARKS
    by_casefold = {name.casefold(): name for name in BENCHMARKS}
    selected: list[str] = []
    seen: set[str] = set()
    for name in names:
        if not isinstance(name, str) or name.casefold() not in by_casefold:
            raise ValueError(f"unknown benchmark: {name}")
        canonical = by_casefold[name.casefold()]
        if canonical in seen:
            raise ValueError(f"duplicate benchmark: {canonical}")
        selected.append(canonical)
        seen.add(canonical)
    if not selected:
        raise ValueError("at least one benchmark must be selected")
    return tuple(selected)


def _cl_bench_working_directory(source_root: Path) -> Path:
    for candidate in (source_root, source_root / "cl-bench"):
        if (candidate / "tests.lisp").is_file() and (candidate / "files").is_dir():
            return candidate
    raise RuntimeError("pinned cl-bench checkout has no tests.lisp and files/ directory")


def _process_record(
    result: scoreboard.ProcessResult,
    wall_seconds: float,
) -> dict[str, Any]:
    stdout, stdout_truncated = _redact(result.stdout)
    stderr, stderr_truncated = _redact(result.stderr)
    return {
        "status": result.status,
        "return_code": result.returncode,
        "wall_seconds": wall_seconds,
        "stdout": stdout,
        "stdout_truncated": stdout_truncated,
        "stderr": stderr,
        "stderr_truncated": stderr_truncated,
    }


def _timed_command(
    command: Sequence[str],
    *,
    cwd: Path,
    timeout: float,
) -> tuple[scoreboard.ProcessResult, float]:
    started = time.perf_counter()
    result = scoreboard.run_command(command, cwd=cwd, timeout=timeout)
    return result, max(0.0, time.perf_counter() - started)


def _revision_record(ncl_revision: str | None = None) -> dict[str, Any]:
    if ncl_revision is not None:
        return {
            "commit": _revision_sha(ncl_revision),
            "status": "passed",
            "source": "explicit NCL executable build revision",
            "wall_seconds": 0.0,
        }
    result, wall_seconds = _timed_command(
        ["jj", "log", "--ignore-working-copy", "--no-graph", "-r", "@-", "-T", "commit_id"],
        cwd=REPO_ROOT,
        timeout=INFO_TIMEOUT,
    )
    revision = result.stdout.strip()
    if result.status == "passed" and re.fullmatch(r"[0-9a-f]{40,64}", revision):
        return {
            "commit": revision,
            "status": result.status,
            "source": "committed source at @-",
            "wall_seconds": wall_seconds,
        }
    error, _ = _redact(result.stderr or result.stdout or "unable to read the working-copy revision")
    raise RuntimeError(f"unable to identify committed build revision at @-: {error}")


def _runtime_probe(command: Sequence[str], *, cwd: Path) -> dict[str, Any]:
    result, wall_seconds = _timed_command(command, cwd=cwd, timeout=INFO_TIMEOUT)
    process = _process_record(result, wall_seconds)
    version = result.stdout.strip().splitlines()[0] if result.stdout.strip() else None
    version, _ = _redact(version or "")
    return {"version": version or None, "process": process}


def _run_benchmark_process(
    *,
    phase: str,
    command: Sequence[str],
    cwd: Path,
    timeout: float,
    mode: str | None = None,
    pair: int | None = None,
    sample: int | None = None,
    benchmark_names: Sequence[str] = BENCHMARKS,
) -> dict[str, Any]:
    process_result, wall_seconds = _timed_command(command, cwd=cwd, timeout=timeout)
    driver_status, flat_times, benchmarks, driver_error, unexpected = _driver_data(
        process_result, benchmark_names
    )
    process = _process_record(process_result, wall_seconds)
    record: dict[str, Any] = {
        "phase": phase,
        "process": process,
        "driver_status": driver_status,
        "driver_times": flat_times,
        "benchmarks": benchmarks,
    }
    if mode is not None:
        record["mode"] = mode
    if pair is not None:
        record["pair"] = pair
    if sample is not None:
        record["sample"] = sample
    if driver_error:
        record["driver_error"] = driver_error
    if unexpected:
        record["unexpected_benchmarks"] = unexpected
    return record


def _ncl_command(
    binary: Path,
    mode: str,
    benchmark_names: Sequence[str] = BENCHMARKS,
) -> list[str]:
    command = [str(NCL_DRIVER), str(binary), mode]
    if tuple(benchmark_names) != BENCHMARKS:
        command.extend(benchmark_names)
    return command


def _sbcl_command(
    binary: str,
    benchmark_names: Sequence[str] = BENCHMARKS,
    *,
    cwd: Path,
) -> list[str]:
    return [
        sys.executable,
        str(DRIVER_SUPERVISOR),
        "--engine",
        "sbcl",
        "--runner",
        binary,
        "--mode",
        "default",
        "--driver",
        str(SBCL_DRIVER),
        "--cwd",
        str(cwd),
        *benchmark_names,
    ]


def run_baseline(
    *,
    ncl: str,
    sbcl: str,
    cache_dir: Path,
    warmups: int = 1,
    pairs: int = 5,
    sbcl_samples: int = 5,
    benchmarks: Sequence[str] | None = None,
    ncl_revision: str | None = None,
) -> dict[str, Any]:
    if pairs < 5 or sbcl_samples < 5:
        raise ValueError("at least 5 samples are required for each runtime comparison")
    ncl_binary = Path(ncl).expanduser().resolve()
    benchmark_names = _select_benchmarks(benchmarks)
    revision = _revision_record(ncl_revision)
    ncl_runtime = _runtime_probe([str(ncl_binary), "--version"], cwd=REPO_ROOT)
    sbcl_runtime = _runtime_probe([sbcl, "--version"], cwd=REPO_ROOT)
    sbcl_version = sbcl_runtime["version"] or ""
    if sbcl_runtime["process"]["status"] != "passed" or not re.search(
        rf"\bSBCL\s+{re.escape(SBCL_VERSION)}\b", sbcl_version, re.IGNORECASE
    ):
        raise RuntimeError(f"SBCL runtime must report version {SBCL_VERSION}")
    if ncl_runtime["process"]["status"] != "passed":
        raise RuntimeError("NCL version probe failed")

    source_root = scoreboard.checkout_source("cl-bench", cache_dir).resolve()
    source = _cl_bench_working_directory(source_root)
    runs: list[dict[str, Any]] = []
    sbcl_command = _sbcl_command(sbcl, benchmark_names, cwd=source)
    warmup_rounds = _round_schedule(warmups, warmups)
    measurement_rounds = _round_schedule(pairs, sbcl_samples)
    for phase, rounds in (("warmup", warmup_rounds), ("measurement", measurement_rounds)):
        for round_items in rounds:
            for item in round_items:
                if item["engine"] == "ncl":
                    runs.append(_run_benchmark_process(
                        phase=phase,
                        command=_ncl_command(ncl_binary, item["mode"], benchmark_names),
                        cwd=source,
                        timeout=scoreboard.DEFAULT_COMMAND_TIMEOUT,
                        mode=item["mode"],
                        pair=item.get("pair"),
                        sample=item.get("sample"),
                        benchmark_names=benchmark_names,
                    ))
                else:
                    runs.append(_run_benchmark_process(
                        phase=phase,
                        command=sbcl_command,
                        cwd=source,
                        timeout=scoreboard.DEFAULT_COMMAND_TIMEOUT,
                        sample=item["sample"],
                        benchmark_names=benchmark_names,
                    ))

    scores = _score_runs(
        runs,
        pairs=pairs,
        sbcl_samples=sbcl_samples,
        benchmark_names=benchmark_names,
    )
    runtimes = {"ncl": ncl_runtime, "sbcl": sbcl_runtime}
    report = {
        "schema_version": 1,
        "status": "pending",
        "measured_on": date.today().isoformat(),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "architecture": platform.machine(),
        },
        "build_revision": revision,
        "source": {
            "name": "cl-bench",
            "commit": scoreboard.SOURCES["cl-bench"]["commit"],
        },
        "runtimes": runtimes,
        "protocol": {
            "ncl_warmups_per_mode": warmups,
            "ncl_pairs": pairs,
            "ncl_pair_order": [
                [item["mode"] for item in round_items if item["engine"] == "ncl"]
                for round_items in measurement_rounds
                if any(item["engine"] == "ncl" for item in round_items)
            ],
            "measurement_round_order": measurement_rounds,
            "sbcl_warmups": warmups,
            "sbcl_samples": sbcl_samples,
            "command_timeout_seconds": scoreboard.DEFAULT_COMMAND_TIMEOUT,
            "benchmarks": list(benchmark_names),
        },
        "runs": runs,
        "scores": scores,
    }
    report["status"] = _report_status(runs, revision, runtimes, scores, benchmark_names)
    return report


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ncl", required=True, help="release NCL binary")
    parser.add_argument(
        "--ncl-revision",
        type=_revision_sha,
        help="commit SHA used to build --ncl; defaults to the current workspace revision",
    )
    parser.add_argument("--sbcl", default="sbcl", help=f"SBCL {SBCL_VERSION} executable")
    parser.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
    parser.add_argument("--warmups", type=_positive_count, default=1)
    parser.add_argument("--pairs", type=_paired_count, default=5)
    parser.add_argument("--sbcl-samples", type=_paired_count, default=5)
    parser.add_argument(
        "--benchmark",
        action="append",
        dest="benchmarks",
        help="measure one selected benchmark; may be repeated",
    )
    parser.add_argument("--output", type=Path, help="write the JSON report to this path")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    try:
        report = run_baseline(
            ncl=args.ncl,
            sbcl=args.sbcl,
            cache_dir=args.cache_dir,
            warmups=args.warmups,
            pairs=args.pairs,
            sbcl_samples=args.sbcl_samples,
            benchmarks=args.benchmarks,
            ncl_revision=args.ncl_revision,
        )
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"optimization-baseline: {error}\n")
    rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    sys.exit(main())
