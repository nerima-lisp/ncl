#!/usr/bin/env python3
"""Measure representative NCL and SBCL execution micro workloads."""

from __future__ import annotations

import argparse
import getpass
import json
import math
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import date
from pathlib import Path
from typing import Any, Mapping, Sequence

import conformance_scoreboard as scoreboard
from benchmark_constants import REPO_ROOT, SBCL_VERSION

if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from bench.timing import MINIMUM_SAMPLE_SECONDS, measurement_metadata
from micro_bench_cases import (
    ALLOCATION_COUNTERS, CASE_DIR, DEFAULT_ITERATIONS, DEFAULT_RUNS, PROCESS_TIMEOUT,
    MICRO_CASES, MICRO_CASE_BY_NAME, RESULT_PREFIX, MicroCase, _driver_source,
    _paired_count, _positive_count, _select_cases,
)

from bench.statistics import paired_statistics, sample_statistics


def _diagnostic(result: scoreboard.ProcessResult) -> str:
    text = result.stderr or result.stdout or "process returned no diagnostic"
    text, truncated = scoreboard._stderr_tail(text)
    for username in {getpass.getuser(), Path.home().name}:
        if username:
            text = re.sub(
                rf"(?<![\w.-]){re.escape(username)}(?![\w.-])",
                "<user>",
                text,
                flags=re.IGNORECASE,
            )
    if truncated:
        return f"<diagnostic truncated>\n{text}"
    return text


def _parse_result(
    stdout: str,
    case: MicroCase,
    engine: str,
    *,
    iterations: int = DEFAULT_ITERATIONS,
    expected_repetitions: int | None = None,
    require_minimum_duration: bool = True,
) -> dict[str, Any]:
    records = []
    repetition_counts = []
    for line in stdout.splitlines():
        if line.startswith("MICRO_REPETITIONS "):
            try:
                repetition_counts.append(int(line.removeprefix("MICRO_REPETITIONS ")))
            except ValueError as error:
                raise ValueError("driver repetition count must be an integer") from error
            continue
        if not line.startswith(RESULT_PREFIX):
            continue
        try:
            record = json.loads(line[len(RESULT_PREFIX) :])
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict):
            records.append(record)
    if len(records) != 1:
        raise ValueError("driver output must contain exactly one MICRO_RESULT record")
    if len(repetition_counts) != 1 or repetition_counts[0] < 1:
        raise ValueError("driver output must contain one positive MICRO_REPETITIONS count")
    repetitions = repetition_counts[0]
    if expected_repetitions is not None and repetitions != expected_repetitions:
        raise ValueError("driver repetition count does not match the requested count")
    record = records[0]
    expected_keys = {
        "case",
        "engine",
        "result",
        "ticks",
        "units_per_second",
        "allocated_bytes",
        "allocation_counter",
        "allocation_reason",
        "gc_count_before",
        "gc_count_after",
        "disassemble_available",
    }
    if set(record) != expected_keys:
        raise ValueError("driver result has an unexpected shape")
    if record["case"] != case.name or record["engine"] != engine:
        raise ValueError("driver result does not match the requested case and engine")
    for field in ("result", "ticks", "units_per_second"):
        value = record[field]
        if isinstance(value, bool) or not isinstance(value, int):
            raise ValueError(f"driver result {field} must be an integer")
    if record["ticks"] < 0 or record["units_per_second"] <= 0:
        raise ValueError("driver time must be positive and above the clock resolution")
    if require_minimum_duration and record["ticks"] == 0:
        raise ValueError("driver time must be positive and above the clock resolution")
    elapsed_seconds = record["ticks"] / record["units_per_second"]
    if require_minimum_duration and elapsed_seconds < MINIMUM_SAMPLE_SECONDS:
        raise ValueError("driver sample is shorter than the minimum measurement time")
    allocation = record["allocated_bytes"]
    if allocation is not None and (
        isinstance(allocation, bool) or not isinstance(allocation, int) or allocation < 0
    ):
        raise ValueError("driver allocation bytes must be null or a non-negative integer")
    if not isinstance(record["allocation_counter"], str):
        raise ValueError("driver allocation counter name must be a string")
    if not isinstance(record["allocation_reason"], str):
        raise ValueError("driver allocation reason must be a string")
    for field in ("gc_count_before", "gc_count_after"):
        value = record[field]
        if value is not None and (
            isinstance(value, bool) or not isinstance(value, int) or value < 0
        ):
            raise ValueError(f"driver {field} must be null or a non-negative integer")
    if allocation is not None and record["allocation_counter"] == "unavailable":
        raise ValueError("driver reported allocated bytes without an allocation counter")
    if allocation is not None and record["allocation_reason"]:
        raise ValueError("driver reported allocated bytes with an allocation failure reason")
    if allocation is None and not record["allocation_reason"]:
        raise ValueError("driver omitted allocation bytes without an allocation reason")
    if engine == "ncl" and allocation is not None and (
        record["gc_count_before"] is None
        or record["gc_count_after"] is None
        or record["gc_count_before"] != record["gc_count_after"]
    ):
        raise ValueError("NCL allocation bytes require stable GC count snapshots")
    if engine == "sbcl" and any(
        record[field] is not None for field in ("gc_count_before", "gc_count_after")
    ):
        raise ValueError("SBCL allocation record must not include NCL GC counts")
    if not isinstance(record["disassemble_available"], bool):
        raise ValueError("driver disassembler status must be boolean")
    expected = case.expected_result(iterations)
    if record["result"] != expected:
        raise ValueError(f"workload result mismatch: expected {expected}, got {record['result']}")
    record["repetitions"] = repetitions
    record["iteration_count"] = iterations * repetitions
    record["elapsed_seconds"] = elapsed_seconds
    record["seconds"] = elapsed_seconds / repetitions
    if allocation is not None:
        record["allocated_bytes_per_batch"] = allocation / repetitions
    return record


def _run_command(
    command: Sequence[str], *, cwd: Path, timeout: float
) -> scoreboard.ProcessResult:
    return scoreboard.run_command(command, cwd=cwd, timeout=timeout)


def _probe_runtime(binary: str, engine: str) -> dict[str, Any]:
    result = _run_command([binary, "--version"], cwd=REPO_ROOT, timeout=30.0)
    version = result.stdout.strip().splitlines()[0] if result.stdout.strip() else None
    record: dict[str, Any] = {
        "version": version,
        "process_status": result.status,
        "return_code": result.returncode,
    }
    if result.status != "passed" or result.returncode != 0:
        record["error"] = _diagnostic(result)
        raise RuntimeError(f"{engine} version probe failed: {record['error']}")
    if engine == "sbcl" and not re.search(
        rf"\bSBCL\s+{re.escape(SBCL_VERSION)}\b", version or "", re.IGNORECASE
    ):
        raise RuntimeError(f"SBCL runtime must report version {SBCL_VERSION}")
    return record


def _sample(
    *,
    binary: str,
    engine: str,
    case: MicroCase,
    pair: int,
    iterations: int,
    driver_path: Path,
) -> dict[str, Any]:
    repetitions = 1
    total_wall_seconds = 0.0
    while True:
        driver_path.write_text(
            _driver_source(case, engine, iterations, repetitions), encoding="utf-8"
        )
        started = time.perf_counter()
        result = _run_command(
            [binary, "--script", str(driver_path)],
            cwd=REPO_ROOT,
            timeout=PROCESS_TIMEOUT,
        )
        total_wall_seconds += max(0.0, time.perf_counter() - started)
        sample: dict[str, Any] = {
            "pair": pair,
            "engine": engine,
            "process_status": result.status,
            "return_code": result.returncode,
            "wall_seconds": total_wall_seconds,
        }
        if result.status != "passed" or result.returncode != 0:
            sample.update({"status": "failed", "reason": _diagnostic(result)})
            return sample
        try:
            parsed = _parse_result(
                result.stdout,
                case,
                engine,
                iterations=iterations,
                expected_repetitions=repetitions,
                require_minimum_duration=False,
            )
        except (TypeError, ValueError) as error:
            sample.update({"status": "failed", "reason": str(error)})
            return sample
        if parsed["elapsed_seconds"] < MINIMUM_SAMPLE_SECONDS:
            repetitions *= 2
            continue
        sample.update(parsed)
        sample["status"] = "passed"
        return sample


def _summarize_engine(samples: Sequence[Mapping[str, Any]], runs: int) -> dict[str, Any]:
    passed = [sample for sample in samples if sample.get("status") == "passed"]
    seconds = [float(sample["seconds"]) for sample in passed]
    timing_statistics = sample_statistics(seconds)
    allocation_values = [
        sample.get("allocated_bytes_per_batch", sample.get("allocated_bytes"))
        for sample in passed
    ]
    all_allocations = len(passed) == runs and all(
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
        and value >= 0
        for value in allocation_values
    )
    allocation_names = list(
        dict.fromkeys(
            str(sample["allocation_counter"])
            for sample in passed
            if sample.get("allocation_counter") != "unavailable"
        )
    )
    if all_allocations:
        allocation_status = "measured"
        allocation_reason = None
        median_allocated_bytes = statistics.median(allocation_values)
        minimum_allocated_bytes = min(allocation_values)
    else:
        allocation_status = "unavailable"
        median_allocated_bytes = None
        minimum_allocated_bytes = None
        allocation_reasons = list(dict.fromkeys(
            str(sample["allocation_reason"])
            for sample in passed
            if sample.get("allocation_reason")
        ))
        if allocation_reasons:
            allocation_reason = "; ".join(allocation_reasons)
        elif not passed or not allocation_names:
            expected = ALLOCATION_COUNTERS[passed[0]["engine"]] if passed else None
            allocation_reason = (
                f"{expected} was unavailable in the runtime process."
                if expected
                else "No successful sample exposed an allocation counter."
            )
        else:
            allocation_reason = "The allocation counter did not return a valid value for every sample."
    return {
        "status": "passed" if len(passed) == runs else "partial",
        "samples": list(samples),
        **timing_statistics,
        "allocation_status": allocation_status,
        "allocation_counter": allocation_names or None,
        "allocation_reason": allocation_reason,
        "median_allocated_bytes": median_allocated_bytes,
        "minimum_allocated_bytes": minimum_allocated_bytes,
    }


def _result_parity(engines: Mapping[str, Mapping[str, Any]]) -> dict[str, Any]:
    results = {
        engine: {
            sample["result"]
            for sample in data["samples"]
            if sample.get("status") == "passed"
        }
        for engine, data in engines.items()
    }
    if not results["ncl"] or not results["sbcl"]:
        return {"status": "unverified", "results": {key: sorted(value) for key, value in results.items()}}
    matched = results["ncl"] == results["sbcl"] and len(results["ncl"]) == 1
    return {
        "status": "passed" if matched else "failed",
        "results": {key: sorted(value) for key, value in results.items()},
    }


def run_micro_suite(
    *,
    ncl: str,
    sbcl: str = "sbcl",
    runs: int = DEFAULT_RUNS,
    iterations: int = DEFAULT_ITERATIONS,
    cases: Sequence[str] | None = None,
) -> dict[str, Any]:
    if runs < 5:
        raise ValueError("at least 5 paired runs are required")
    if iterations < 1:
        raise ValueError("iterations must be a positive integer")
    selected = _select_cases(cases)
    ncl_binary = str(Path(ncl).expanduser().resolve())
    sbcl_binary = str(Path(sbcl).expanduser())
    runtimes = {
        "ncl": _probe_runtime(ncl_binary, "ncl"),
        "sbcl": _probe_runtime(sbcl_binary, "sbcl"),
    }
    metadata = measurement_metadata(SBCL_VERSION)
    samples: dict[str, dict[str, list[dict[str, Any]]]] = {
        case.name: {"ncl": [], "sbcl": []} for case in selected
    }
    pair_order = [
        ["ncl", "sbcl"] if pair % 2 else ["sbcl", "ncl"]
        for pair in range(1, runs + 1)
    ]
    with tempfile.TemporaryDirectory(prefix="ncl-micro-bench-", dir=REPO_ROOT) as temporary:
        temporary_path = Path(temporary)
        for case in selected:
            source = CASE_DIR / case.source
            if not source.is_file() or not source.read_text(encoding="utf-8").strip():
                raise RuntimeError(f"micro benchmark source is missing or empty: {case.source}")
            for pair, order in enumerate(pair_order, start=1):
                for engine in order:
                    binary = ncl_binary if engine == "ncl" else sbcl_binary
                    driver_path = temporary_path / f"{case.name}-{pair}-{engine}.lisp"
                    samples[case.name][engine].append(
                        _sample(
                            binary=binary,
                            engine=engine,
                            case=case,
                            pair=pair,
                            iterations=iterations,
                            driver_path=driver_path,
                        )
                    )

    case_records: dict[str, Any] = {}
    for case in selected:
        engine_records = {
            engine: _summarize_engine(samples[case.name][engine], runs)
            for engine in ("ncl", "sbcl")
        }
        ncl_disassemble = [
            sample["disassemble_available"]
            for sample in engine_records["ncl"]["samples"]
            if sample.get("status") == "passed"
        ]
        disassemble_available = None if not ncl_disassemble else all(ncl_disassemble)
        codegen = {
            "status": "unsupported" if disassemble_available is False else "unverified",
            "disassembler_available": disassemble_available,
            "reason": (
                "COMMON-LISP:DISASSEMBLE is not fbound in NCL."
                if disassemble_available is False
                else "NCL has no successful sample to inspect."
                if disassemble_available is None
                else "The NCL disassembler is bound, but this suite has no instruction-sequence assertions."
            ),
        }
        case_records[case.name] = {
            "iterations": iterations,
            "minimum_sample_seconds": MINIMUM_SAMPLE_SECONDS,
            "engines": engine_records,
            "paired_comparison": paired_statistics(
                engine_records["ncl"]["samples"],
                engine_records["sbcl"]["samples"],
                expected_sample_count=runs,
            ),
            "output_parity": _result_parity(engine_records),
            "codegen_inspection": codegen,
        }
    complete = all(
        data["engines"][engine]["status"] == "passed"
        and data["engines"][engine]["allocation_status"] == "measured"
        for data in case_records.values()
        for engine in ("ncl", "sbcl")
    ) and all(
        data["output_parity"]["status"] == "passed"
        and data["codegen_inspection"]["status"] == "passed"
        for data in case_records.values()
    )
    return {
        "schema_version": 1,
        "status": "passed" if complete else "partial",
        "measured_on": date.today().isoformat(),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "architecture": platform.machine(),
        },
        "measurement": metadata,
        "runtimes": runtimes,
        "protocol": {
            "runs": runs,
            "iterations": iterations,
            "minimum_sample_seconds": MINIMUM_SAMPLE_SECONDS,
            "iteration_count_definition": "iterations per workload call multiplied by repetitions",
            "in_process_warmup_calls": 1,
            "pair_order": pair_order,
            "timer": "GET-INTERNAL-REAL-TIME / INTERNAL-TIME-UNITS-PER-SECOND",
            "allocation_counters": ALLOCATION_COUNTERS,
            "selected_cases": [case.name for case in selected],
            "required_cases": [case.name for case in MICRO_CASES],
        },
        "noise_width_definition": (
            "per-engine timing range is max minus min; paired noise width is the "
            "range of matched NCL-minus-SBCL pair deltas"
        ),
        "cases": case_records,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ncl", required=True, help="NCL binary")
    parser.add_argument("--sbcl", default="sbcl", help=f"SBCL {SBCL_VERSION} executable")
    parser.add_argument("--runs", type=_paired_count, default=DEFAULT_RUNS)
    parser.add_argument("--iterations", type=_positive_count, default=DEFAULT_ITERATIONS)
    parser.add_argument("--case", action="append", dest="cases", help="select a micro case; may be repeated")
    parser.add_argument("--output", type=Path, help="write JSON report to this path")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    try:
        report = run_micro_suite(
            ncl=args.ncl,
            sbcl=args.sbcl,
            runs=args.runs,
            iterations=args.iterations,
            cases=args.cases,
        )
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"micro-bench: {error}\n")
    rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0


if __name__ == "__main__":
    sys.exit(main())
