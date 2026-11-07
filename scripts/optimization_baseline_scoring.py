"""Score paired NCL modes and SBCL cl-bench observations."""

from __future__ import annotations

import math
import statistics
from typing import Any, Mapping, Sequence

import conformance_scoreboard as scoreboard
from optimization_baseline_constants import (
    BENCHMARKS, DISABLED_BENCHMARK, DISABLED_BENCHMARK_REASONS,
)


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
    if "cl-bench warning:" in run["process"].get("stderr", ""):
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
