"""Normalize cl-bench process output into stable report records."""

from __future__ import annotations

from collections import Counter
import getpass
import json
import math
import re
from pathlib import Path
from typing import Any, Mapping, Sequence

import conformance_scoreboard as scoreboard
from optimization_baseline_constants import (
    BENCHMARKS, DISABLED_BENCHMARK, DISABLED_BENCHMARK_REASONS,
    MAX_BENCHMARK_TIMES, MAX_ERROR_CHARS, MAX_FLAT_TIMES,
    MAX_RUNNER_OUTPUT_CHARS, RUNNER_STATUSES,
)


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
    expected_keys = {"status", "stage", "times", "condition"}
    keys_with_wall_time = expected_keys | {"wall_time_seconds"}
    if set(value) not in (expected_keys, keys_with_wall_time):
        return _failed_benchmark(
            "benchmark record has an unexpected shape",
            stage=stage,
            condition=condition,
        )
    if "wall_time_seconds" in value and (
        not _finite_number(value["wall_time_seconds"])
        or value["wall_time_seconds"] < 0
    ):
        return _failed_benchmark(
            "benchmark wall time must be a finite nonnegative number",
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
