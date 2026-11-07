"""Aggregate compile samples and pass-profiler diagnostics."""

from __future__ import annotations

import re
import statistics
from pathlib import Path
from typing import Any, Callable, Sequence


_PASS_NAME = re.compile(r'\bpass="((?:[^"\\]|\\.)*)"')
_PASS_TIME = re.compile(r"\belapsed_ns=([0-9]+)\b")
_PASS_ALLOC = re.compile(r"\ballocated_bytes=([0-9]+)\b")


def aggregate_file_samples(samples: list[dict[str, Any]]) -> dict[str, Any]:
    first = samples[0]
    seconds = [
        float(row["compile_seconds"])
        for row in samples
        if row["status"] == "passed" and isinstance(row["compile_seconds"], (int, float))
    ]
    complete = len(seconds) == len(samples)
    median = statistics.median(seconds) if complete else None
    reasons = [row["reason"] for row in samples if row["reason"] is not None]
    peaks = [
        row["peak_rss_bytes"]
        for row in samples
        if isinstance(row["peak_rss_bytes"], int) and row["peak_rss_bytes"] >= 0
    ]
    return {
        "suite": first["suite"],
        "version": first["version"],
        "file": first["file"],
        "line_count": first["line_count"],
        "status": "passed" if complete else "failed",
        "sample_count": len(samples),
        "compile_seconds_samples": [
            row["compile_seconds"] if row["status"] == "passed" else None for row in samples
        ],
        "compile_seconds": median,
        "minimum_compile_seconds": min(seconds) if complete else None,
        "lines_per_second": first["line_count"] / median if complete and median and median > 0 else None,
        "process_wall_seconds_samples": [row["process_wall_seconds"] for row in samples],
        "peak_rss_bytes": max(peaks) if peaks else None,
        "peak_rss_sample_count": len(peaks),
        "failure_reasons": reasons,
    }


def pass_profile(
    output: str,
    paths: Sequence[Path],
    safe_diagnostic: Callable[[str, Sequence[Path]], str],
) -> dict[str, Any]:
    passes: dict[str, dict[str, int]] = {}
    malformed_record = False
    for line in output.splitlines():
        if not line.startswith("ncl-opt "):
            continue
        name_match = _PASS_NAME.search(line)
        time_match = _PASS_TIME.search(line)
        allocation_match = _PASS_ALLOC.search(line)
        if name_match is None or time_match is None or allocation_match is None:
            malformed_record = True
            continue
        name = name_match.group(1).replace('\\"', '"').replace("\\\\", "\\")
        row = passes.setdefault(name, {"invocations": 0, "elapsed_ns": 0, "allocated_bytes": 0, "allocation_samples": 0})
        row["invocations"] += 1
        row["elapsed_ns"] += int(time_match.group(1))
        row["allocated_bytes"] += int(allocation_match.group(1))
        row["allocation_samples"] += 1
    if malformed_record:
        return {
            "status": "unavailable",
            "passes": [],
            "top_10": [],
            "allocation_bytes_available": False,
            "reason": "profiler emitted a malformed pass record",
        }
    rows = []
    for name, metrics in passes.items():
        rows.append({
            "pass": name,
            "invocations": metrics["invocations"],
            "elapsed_ns": metrics["elapsed_ns"],
            "allocated_bytes": metrics["allocated_bytes"] if metrics["allocation_samples"] == metrics["invocations"] else None,
        })
    rows.sort(key=lambda row: (-row["elapsed_ns"], row["pass"]))
    return {
        "status": "measured" if rows else "unavailable",
        "passes": rows,
        "top_10": rows[:10],
        "allocation_bytes_available": bool(rows) and all(row["allocated_bytes"] is not None for row in rows),
        "reason": None if rows else safe_diagnostic(output, paths),
    }


def runtime_summary(
    version: str,
    startup: dict[str, Any],
    files: list[dict[str, Any]],
    binary_size: int | None,
) -> dict[str, Any]:
    successful = [row for row in files if row["status"] == "passed" and isinstance(row["compile_seconds"], (int, float))]
    all_successful = len(successful) == len(files)
    sample_count = files[0]["sample_count"]
    if any(len(row["compile_seconds_samples"]) != sample_count for row in files):
        raise ValueError("compile source samples have inconsistent counts")
    total_samples = (
        [sum(float(row["compile_seconds_samples"][index]) for row in files) for index in range(sample_count)]
        if all_successful
        else [None] * sample_count
    )
    total_seconds = statistics.median(total_samples) if all_successful else None
    minimum_total_seconds = min(total_samples) if all_successful else None
    line_total = sum(int(row["line_count"]) for row in successful)
    expected_line_total = sum(int(row["line_count"]) for row in files)
    peaks = [
        row["peak_rss_bytes"]
        for row in files
        if isinstance(row["peak_rss_bytes"], int) and row["peak_rss_bytes"] >= 0
    ]
    peak_samples = sum(row["peak_rss_sample_count"] for row in files)
    expected_peak_samples = sum(row["sample_count"] for row in files)
    return {
        "version": version,
        "file_count": len(files),
        "compiled_file_count": len(successful),
        "failed_file_count": len(files) - len(successful),
        "sample_runs": sample_count,
        "total_compile_seconds_samples": total_samples,
        "total_compile_seconds": total_seconds,
        "minimum_total_compile_seconds": minimum_total_seconds,
        "line_count": line_total,
        "expected_line_count": expected_line_total,
        "lines_per_second": line_total / total_seconds if total_seconds is not None and total_seconds > 0 else None,
        "peak_rss_bytes": max(peaks) if peaks else None,
        "peak_rss_sample_count": peak_samples,
        "peak_rss_expected_sample_count": expected_peak_samples,
        "peak_rss_available": peak_samples == expected_peak_samples,
        "startup": startup,
        "binary_size_bytes": binary_size,
        "files": [{key: value for key, value in row.items() if key != "_stderr"} for row in files],
    }
