#!/usr/bin/env python3
"""Measure paired NCL optimization modes and a pinned SBCL baseline."""

from __future__ import annotations

import argparse
from collections import Counter
import getpass
import json
import math
import platform
import re
import statistics
import subprocess
import sys
import time
from datetime import date
from pathlib import Path
from typing import Any, Mapping, Sequence

import conformance_scoreboard as scoreboard


REPO_ROOT = Path(__file__).resolve().parents[1]
NCL_DRIVER = REPO_ROOT / "conformance/cl-bench/ncl-driver.sh"
SBCL_DRIVER = REPO_ROOT / "conformance/cl-bench/sbcl-driver.lisp"
DRIVER_SUPERVISOR = REPO_ROOT / "conformance/cl-bench/driver_supervisor.py"
INFO_TIMEOUT = 30.0
SBCL_VERSION = "2.6.0"
MAX_RUNNER_OUTPUT_CHARS = 1_000_000
MAX_BENCHMARK_TIMES = 10_000
MAX_FLAT_TIMES = 650_000
MAX_ERROR_CHARS = 64_000

BENCHMARKS = (
    "1D-ARRAYS",
    "2D-ARRAYS",
    "3D-ARRAYS",
    "ACKERMANN",
    "BENCH-STRINGS",
    "BIGNUM/ELEM-100-1000",
    "BIGNUM/ELEM-1000-100",
    "BIGNUM/ELEM-10000-1",
    "BIGNUM/PARI-100-10",
    "BIGNUM/PARI-200-5",
    "BITVECTORS",
    "BOEHM-GC",
    "BOYER",
    "BROWSE",
    "CLOS/complex-methods",
    "CLOS/defclass",
    "CLOS/defmethod",
    "CLOS/instantiate",
    "CLOS/method+after",
    "CLOS/methodcalls",
    "CLOS/simple-instantiate",
    "COMPILER",
    "CRC40",
    "CTAK",
    "DDERIV",
    "DEFLATE-FILE",
    "DERIV",
    "DESTRUCTIVE",
    "DIV2-TEST-1",
    "DIV2-TEST-2",
    "EQL-SPECIALIZED-FIB",
    "FACTORIAL",
    "FFT",
    "FIB",
    "FIB-RATIO",
    "FPRINT/PRETTY",
    "FPRINT/UGLY",
    "FRPOLY/BIGNUM",
    "FRPOLY/FIXNUM",
    "FRPOLY/FLOAT",
    "HASH-INTEGERS",
    "HASH-STRINGS",
    "LOAD-FASL",
    "MANDELBROT/COMPLEX",
    "MANDELBROT/DFLOAT",
    "MRG32K3A",
    "PI-ATAN",
    "PI-DECIMAL/BIG",
    "PI-DECIMAL/SMALL",
    "PI-RATIOS",
    "PUZZLE",
    "RICHARDS",
    "SEARCH-SEQUENCE",
    "SLURP-LINES",
    "STAK",
    "STRING-CONCAT",
    "SUM-PERMUTATIONS",
    "TAK",
    "TAKL",
    "TRAVERSE",
    "TRIANGLE",
    "TRTAK",
    "WALK-LIST/MESS",
    "WALK-LIST/SEQ",
    "fill-strings/adjustable",
)
DISABLED_BENCHMARK = "WALK-LIST/MESS"
DISABLED_BENCHMARK_REASONS = {
    DISABLED_BENCHMARK: (
        "The pinned cl-bench definition lists SBCL under disabled-for for "
        "WALK-LIST/MESS."
    ),
}
RUNNER_STATUSES = {"passed", "disabled", "failed"}


def _positive_count(value: str) -> int:
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive integer") from error
    if count < 1:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return count


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


def _redact(text: str) -> tuple[str, bool]:
    if len(text) > MAX_RUNNER_OUTPUT_CHARS:
        return "<diagnostic omitted: size limit exceeded>", True
    redacted, truncated = scoreboard._stderr_tail(text)
    for username in {getpass.getuser(), Path.home().name}:
        if username:
            redacted = re.sub(
                rf"(?<![\w.-]){re.escape(username)}(?![\w.-])",
                "<user>",
                redacted,
                flags=re.IGNORECASE,
            )
    redacted = re.sub(r"(?<![\w.-])~[/\\][^\s\r\n]*", "<home-path>", redacted)
    return redacted, truncated


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


def _revision_record() -> dict[str, Any]:
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


def _finite_number(value: Any) -> bool:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return False
    try:
        return math.isfinite(float(value))
    except (OverflowError, ValueError):
        return False


def _parse_json_object(stdout: str) -> Mapping[str, Any] | None:
    if len(stdout) > MAX_RUNNER_OUTPUT_CHARS:
        return None
    decoder = json.JSONDecoder()
    candidate = None
    for index, match in enumerate(re.finditer(r'"benchmarks"\s*:', stdout)):
        if index >= 256:
            return None
        start = stdout.rfind("{", 0, match.start())
        if start < 0:
            continue
        try:
            value, _ = decoder.raw_decode(stdout, start)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and set(value) == {"status", "times", "benchmarks"}:
            candidate = value
    return candidate


def _safe_error(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value)
    if len(text) > MAX_ERROR_CHARS:
        return "driver error exceeded the diagnostic limit"
    text, _ = _redact(text)
    return text


def _normalize_times(
    value: Any,
    *,
    limit: int = MAX_BENCHMARK_TIMES,
) -> tuple[list[Any], str | None]:
    if not isinstance(value, list):
        return [], "times must be an array"
    if len(value) > limit:
        return [], "times exceed the sample limit"
    times: list[Any] = []
    for item in value:
        if isinstance(item, bool) or not isinstance(item, (int, float)):
            times.append(None)
        else:
            times.append(item if _finite_number(item) else None)
    if any(item is None for item in times):
        return times, "times contain a non-finite or non-numeric value"
    return times, None


def _failed_benchmark(
    error: str,
    *,
    stage: str | None = None,
    condition: str | None = None,
    times: list[Any] | None = None,
) -> dict[str, Any]:
    diagnostic = _safe_error(error) or "benchmark runner reported failure"
    return {
        "status": "failed",
        "stage": stage,
        "times": times or [],
        "condition": _safe_error(condition) or diagnostic,
        "error": diagnostic,
        "reason": None,
    }


def _benchmark_record(value: Any, *, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        return _failed_benchmark("benchmark record must be an object")
    raw_stage = value.get("stage")
    stage = raw_stage if isinstance(raw_stage, str) and raw_stage in scoreboard.CL_BENCH_STAGES else None
    raw_condition = value.get("condition")
    condition = _safe_error(raw_condition) if isinstance(raw_condition, str) else None
    if set(value) != {"status", "stage", "times", "condition"}:
        return _failed_benchmark(
            "benchmark record has an unexpected shape",
            stage=stage,
            condition=condition,
        )
    status = value.get("status")
    times, time_error = _normalize_times(value.get("times"))
    if stage is None:
        return _failed_benchmark(
            "benchmark stage is missing or invalid",
            condition=condition,
            times=times,
        )
    if raw_condition is not None and not isinstance(raw_condition, str):
        return _failed_benchmark(
            "benchmark condition must be a string or null",
            stage=stage,
            times=times,
        )
    if not isinstance(status, str) or status not in RUNNER_STATUSES:
        return _failed_benchmark(
            "benchmark status is missing or invalid",
            stage=stage,
            condition=condition,
            times=times,
        )
    if time_error:
        return _failed_benchmark(
            time_error,
            stage=stage,
            condition=condition,
            times=times,
        )
    if status == "passed":
        if stage != "finish":
            return _failed_benchmark(
                "passed benchmark did not reach finish",
                stage=stage,
                condition=condition,
                times=times,
            )
        if condition is not None:
            return _failed_benchmark(
                "passed benchmark has a condition",
                stage=stage,
                condition=condition,
                times=times,
            )
        if not times or any(item is None or item <= 0 for item in times):
            return _failed_benchmark(
                "passed benchmark has no positive times",
                stage=stage,
                times=times,
            )
    if status == "failed" and not condition:
        condition = "benchmark runner reported failure"
    error = condition if status == "failed" else None
    reason = None
    if status == "disabled":
        if times:
            return _failed_benchmark(
                "disabled benchmark has sample times",
                stage=stage,
                condition=condition,
                times=times,
            )
        else:
            reason = condition or DISABLED_BENCHMARK_REASONS.get(name)
            error = None
    return {
        "status": status,
        "stage": stage,
        "times": times,
        "condition": condition,
        "error": error,
        "reason": reason,
    }


def _driver_data(
    process: scoreboard.ProcessResult,
    benchmark_names: Sequence[str] = BENCHMARKS,
) -> tuple[str | None, list[Any], dict[str, Any], str | None, list[str]]:
    report = _parse_json_object(process.stdout)
    if report is None:
        error = "driver stdout did not contain a valid benchmark JSON object"
        return None, [], {name: _failed_benchmark(error) for name in benchmark_names}, error, []

    driver_status = report.get("status")
    if not isinstance(driver_status, str) or driver_status not in {"passed", "failed"}:
        driver_error = "driver status is missing or invalid"
        driver_status = None
    else:
        driver_error = None
    flat_times, flat_time_error = _normalize_times(report.get("times"), limit=MAX_FLAT_TIMES)
    if flat_time_error:
        driver_error = driver_error or flat_time_error

    reported = report.get("benchmarks")
    if not isinstance(reported, dict):
        error = driver_error or "driver benchmarks must be an object"
        return driver_status, flat_times, {
            name: _failed_benchmark(error) for name in benchmark_names
        }, error, []

    benchmarks = {
        name: _benchmark_record(reported.get(name), name=name)
        if name in reported
        else _failed_benchmark("benchmark missing from driver output")
        for name in benchmark_names
    }
    if driver_status == "passed" and any(
        benchmark["status"] == "failed" for benchmark in benchmarks.values()
    ):
        driver_status = "failed"
        driver_error = driver_error or "driver reported passed status with failed benchmarks"
    unexpected = sorted(
        _safe_error(name) or "" for name in reported.keys() if name not in benchmark_names
    )
    if unexpected and driver_error is None:
        driver_error = "driver reported unexpected benchmark names"
    flattened = [
        time_value
        for record in benchmarks.values()
        if record["status"] == "passed"
        for time_value in record["times"]
    ]
    numeric_values = flat_times + flattened
    if all(_finite_number(value) for value in numeric_values) and Counter(
        float(value) for value in flat_times
    ) != Counter(
        float(value) for value in flattened
    ) and driver_error is None:
        driver_error = "flat times differ from benchmark times"
    return driver_status, flat_times, benchmarks, driver_error, unexpected


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


def _median_for_benchmark(runs: Sequence[Mapping[str, Any]], name: str) -> float | None:
    observations: list[float] = []
    for run in runs:
        benchmark = run["benchmarks"][name]
        if benchmark["status"] != "passed" or not benchmark["times"]:
            return None
        times = benchmark["times"]
        if any(
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not math.isfinite(value)
            or value <= 0
            for value in times
        ):
            return None
        observations.extend(float(value) for value in times)
    return statistics.median(observations) if observations else None


def _score_runs(
    runs: Sequence[Mapping[str, Any]],
    *,
    pairs: int,
    sbcl_samples: int,
    benchmark_names: Sequence[str] = BENCHMARKS,
) -> dict[str, Any]:
    measured = [run for run in runs if run["phase"] == "measurement"]
    by_runtime: dict[str, list[Mapping[str, Any]]] = {
        "ncl_off": [run for run in measured if run.get("mode") == "off"],
        "ncl_default": [run for run in measured if run.get("mode") == "default"],
        "sbcl": [run for run in measured if run.get("mode") is None],
    }
    expected_counts = {
        "ncl_off": pairs,
        "ncl_default": pairs,
        "sbcl": sbcl_samples,
    }
    medians = {
        runtime: {
            name: _median_for_benchmark(runtime_runs, name)
            if len(runtime_runs) == expected_counts[runtime]
            else None
            for name in benchmark_names
        }
        for runtime, runtime_runs in by_runtime.items()
    }
    common = [
        name for name in benchmark_names
        if name != DISABLED_BENCHMARK
        and all(medians[runtime][name] is not None for runtime in medians)
    ]
    missing = {}
    for name in benchmark_names:
        if name in common:
            continue
        if name == DISABLED_BENCHMARK:
            continue
        missing[name] = {
            runtime: {
                "reason": "no complete positive sample set",
                "errors": list(dict.fromkeys(
                    run["benchmarks"][name]["error"]
                    for run in by_runtime[runtime]
                    if run["benchmarks"][name]["error"] is not None
                )),
            }
            for runtime in medians
            if medians[runtime][name] is None
        }
    geometric_means = {
        runtime: scoreboard.geometric_mean([medians[runtime][name] for name in common])
        if common
        else None
        for runtime in medians
    }
    ratios = {
        "off_over_sbcl": (
            geometric_means["ncl_off"] / geometric_means["sbcl"]
            if common else None
        ),
        "default_over_sbcl": (
            geometric_means["ncl_default"] / geometric_means["sbcl"]
            if common else None
        ),
    }
    benchmark_ratios = {
        "off_over_sbcl": {
            name: medians["ncl_off"][name] / medians["sbcl"][name]
            for name in common
        },
        "default_over_sbcl": {
            name: medians["ncl_default"][name] / medians["sbcl"][name]
            for name in common
        },
    }
    slowest_ten = {
        mode: [
            {"name": name, "ncl_over_sbcl_ratio": ratio}
            for name, ratio in sorted(
                mode_ratios.items(),
                key=lambda item: (-item[1], BENCHMARKS.index(item[0])),
            )[:10]
        ]
        for mode, mode_ratios in benchmark_ratios.items()
    }
    disabled = {
        name: {
            runtime: [
                run["benchmarks"][name]
                for run in (by_runtime[runtime] or [])
            ]
            for runtime in by_runtime
        }
        for name in (DISABLED_BENCHMARK,)
        if name in benchmark_names
    }
    excluded = [
        {
            "name": name,
            "reason": DISABLED_BENCHMARK_REASONS[name],
        }
        for name in (DISABLED_BENCHMARK,)
    ]
    return {
        "common_benchmarks": common,
        "common_benchmark_count": len(common),
        "excluded_benchmark_count": len(excluded),
        "excluded_benchmarks": excluded,
        "missing_benchmarks": missing,
        "benchmark_medians_seconds": {
            runtime: {name: medians[runtime][name] for name in common}
            for runtime in medians
        },
        "geometric_mean_seconds": geometric_means,
        "ncl_over_sbcl_ratio": ratios,
        "benchmark_ncl_over_sbcl_ratio": benchmark_ratios,
        "slowest_10_by_ncl_over_sbcl_ratio": slowest_ten,
        "disabled_benchmarks": disabled,
    }


def _run_is_clean(run: Mapping[str, Any]) -> bool:
    if run["process"]["status"] != "passed" or run["driver_status"] != "passed":
        return False
    return all(
        benchmark["status"] in {"passed", "disabled"}
        for benchmark in run["benchmarks"].values()
    ) and not run.get("driver_error")


def _report_status(
    runs: Sequence[Mapping[str, Any]],
    revision: Mapping[str, Any],
    runtimes: Mapping[str, Any],
    scores: Mapping[str, Any],
    benchmark_names: Sequence[str] = BENCHMARKS,
) -> str:
    if revision["status"] != "passed":
        return "failed"
    if any(runtime["process"]["status"] != "passed" for runtime in runtimes.values()):
        return "failed"
    if any(not _run_is_clean(run) for run in runs):
        return "failed"
    sbcl_runs = [
        run for run in runs
        if run["phase"] == "measurement" and run.get("mode") is None
    ]
    if DISABLED_BENCHMARK in benchmark_names and (
        not sbcl_runs
        or any(
            run["benchmarks"][DISABLED_BENCHMARK]["status"] != "disabled"
            or not run["benchmarks"][DISABLED_BENCHMARK]["reason"]
            for run in sbcl_runs
        )
    ):
        return "failed"
    for run in runs:
        if run.get("mode") is not None:
            if any(benchmark["status"] != "passed" for benchmark in run["benchmarks"].values()):
                return "failed"
        elif any(
            benchmark["status"] != ("disabled" if name == DISABLED_BENCHMARK else "passed")
            for name, benchmark in run["benchmarks"].items()
        ):
            return "failed"
    expected_common = len(benchmark_names) - int(DISABLED_BENCHMARK in benchmark_names)
    if scores["common_benchmark_count"] != expected_common:
        return "failed"
    return "passed"


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
    pairs: int = 3,
    sbcl_samples: int = 3,
    benchmarks: Sequence[str] | None = None,
) -> dict[str, Any]:
    ncl_binary = Path(ncl).expanduser().resolve()
    benchmark_names = _select_benchmarks(benchmarks)
    revision = _revision_record()
    ncl_runtime = _runtime_probe([str(ncl_binary), "--version"], cwd=REPO_ROOT)
    sbcl_runtime = _runtime_probe([sbcl, "--version"], cwd=REPO_ROOT)
    sbcl_version = sbcl_runtime["version"] or ""
    if sbcl_runtime["process"]["status"] != "passed" or not re.search(
        rf"\bSBCL\s+{re.escape(SBCL_VERSION)}\b", sbcl_version, re.IGNORECASE
    ):
        raise RuntimeError("SBCL runtime must report version 2.6.0")
    if ncl_runtime["process"]["status"] != "passed":
        raise RuntimeError("NCL version probe failed")

    source = scoreboard.checkout_source("cl-bench", cache_dir)
    runs: list[dict[str, Any]] = []
    for mode in ("off", "default"):
        for sample in range(1, warmups + 1):
            runs.append(_run_benchmark_process(
                phase="warmup",
                command=_ncl_command(ncl_binary, mode, benchmark_names),
                cwd=source,
                timeout=scoreboard.DEFAULT_COMMAND_TIMEOUT,
                mode=mode,
                sample=sample,
                benchmark_names=benchmark_names,
            ))

    for pair in range(1, pairs + 1):
        order = ("off", "default") if pair % 2 else ("default", "off")
        for mode in order:
            runs.append(_run_benchmark_process(
                phase="measurement",
                command=_ncl_command(ncl_binary, mode, benchmark_names),
                cwd=source,
                timeout=scoreboard.DEFAULT_COMMAND_TIMEOUT,
                mode=mode,
                pair=pair,
                benchmark_names=benchmark_names,
            ))

    sbcl_command = _sbcl_command(sbcl, benchmark_names, cwd=source)
    for sample in range(1, warmups + 1):
        runs.append(_run_benchmark_process(
            phase="warmup",
            command=sbcl_command,
            cwd=source,
            timeout=scoreboard.DEFAULT_COMMAND_TIMEOUT,
            sample=sample,
            benchmark_names=benchmark_names,
        ))
    for sample in range(1, sbcl_samples + 1):
        runs.append(_run_benchmark_process(
            phase="measurement",
            command=sbcl_command,
            cwd=source,
            timeout=scoreboard.DEFAULT_COMMAND_TIMEOUT,
            sample=sample,
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
                ["off", "default"] if pair % 2 else ["default", "off"]
                for pair in range(1, pairs + 1)
            ],
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
    parser.add_argument("--sbcl", default="sbcl", help="SBCL 2.6.0 executable")
    parser.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
    parser.add_argument("--warmups", type=_positive_count, default=1)
    parser.add_argument("--pairs", type=_positive_count, default=3)
    parser.add_argument("--sbcl-samples", type=_positive_count, default=3)
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
