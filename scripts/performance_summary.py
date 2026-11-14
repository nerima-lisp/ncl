"""Render benchmark JSON summaries as case-level Markdown tables."""

from __future__ import annotations

from typing import Any, Mapping


def _cell(value: Any) -> str:
    if value is None:
        return "n/a"
    if isinstance(value, float):
        return f"{value:.6g}"
    return str(value).replace("|", "\\|").replace("\n", " ")


def _noise_label(value: Any) -> str:
    if value is True:
        return "below noise"
    if value is False:
        return "exceeds noise"
    return "n/a"


def _measurement_cells(report: Mapping[str, Any]) -> tuple[str, str, str, str]:
    metadata = report.get("measurement", {})
    if not isinstance(metadata, Mapping):
        metadata = {}
    load = metadata.get("load_average")
    if isinstance(load, (list, tuple)) and len(load) == 3:
        load_average = "/".join(_cell(value) for value in load)
    else:
        load_average = "n/a"
    return (
        _cell(metadata.get("ncl_sha")),
        _cell(metadata.get("sbcl_version")),
        _cell(metadata.get("host")),
        load_average,
    )


def _repetition_counts(samples: Any) -> str:
    if not isinstance(samples, (list, tuple)) or not samples:
        return "n/a"
    return ",".join(
        _cell(sample.get("repetitions"))
        if isinstance(sample, Mapping) and sample.get("status") == "passed"
        else _cell(sample)
        if isinstance(sample, int) and not isinstance(sample, bool)
        else "fail"
        for sample in samples
    )


def _sample_status(samples: Any, label: str, fallback: Any = None) -> str:
    if not isinstance(samples, (list, tuple)) or not samples:
        return _cell(fallback)
    failures = []
    for sample in samples:
        if not isinstance(sample, Mapping):
            failures.append("invalid sample record")
        elif sample.get("status") != "passed":
            status = sample.get("status", "unknown")
            reason = sample.get("reason") or sample.get("error")
            failures.append(
                str(reason)
                if status == "failed" and reason
                else f"{status}: {reason}"
                if reason
                else str(status)
            )
    if failures:
        return f"{label} fail: {'; '.join(dict.fromkeys(failures))}"
    return "passed"


def _compile_repetitions_by_file(metrics: Mapping[str, Any]) -> str:
    rows = metrics.get("files", [])
    if not isinstance(rows, (list, tuple)) or not rows:
        return "n/a"
    return "; ".join(
        f"{row.get('suite')}/{row.get('file')}: "
        f"{_repetition_counts(row.get('compile_repetitions_samples', []))}"
        for row in rows
        if isinstance(row, Mapping)
    ) or "n/a"


def _compile_status(row: Mapping[str, Any], runtime: str) -> str:
    status = row.get("status", "unknown")
    if status == "passed":
        return "passed"
    reasons = row.get("failure_reasons", [])
    if isinstance(reasons, (list, tuple)):
        reasons = list(dict.fromkeys(str(reason) for reason in reasons if reason))
    else:
        reasons = []
    if reasons:
        return f"{runtime.upper()} {status}: {'; '.join(reasons)}"
    return f"{runtime.upper()} {status}"


def _cl_bench_failure(report: Mapping[str, Any], name: str, mode: str | None, label: str) -> str:
    errors = []
    disabled = []
    for run in report.get("runs", []):
        if not isinstance(run, Mapping) or run.get("phase") != "measurement":
            continue
        if run.get("mode") != mode:
            continue
        benchmarks = run.get("benchmarks", {})
        record = benchmarks.get(name, {}) if isinstance(benchmarks, Mapping) else {}
        if not isinstance(record, Mapping):
            continue
        status = record.get("status")
        if status == "disabled":
            reason = record.get("reason") or record.get("condition")
            if reason:
                disabled.append(str(reason))
        elif status != "passed":
            reason = record.get("error") or record.get("condition") or f"status {status}"
            errors.append(str(reason))
    errors = list(dict.fromkeys(errors))
    disabled = list(dict.fromkeys(disabled))
    if errors:
        return f"{label} fail: {'; '.join(errors)}"
    if disabled:
        return f"disabled: {'; '.join(disabled)}"
    return "passed"


def _cl_bench_sample_metadata(
    report: Mapping[str, Any], name: str, mode: str | None
) -> list[Mapping[str, Any]]:
    samples = []
    for run in report.get("runs", []):
        if not isinstance(run, Mapping) or run.get("phase") != "measurement":
            continue
        if run.get("mode") != mode:
            continue
        benchmarks = run.get("benchmarks", {})
        record = benchmarks.get(name, {}) if isinstance(benchmarks, Mapping) else {}
        if not isinstance(record, Mapping) or record.get("status") != "passed":
            continue
        sample_metadata = record.get("sample_metadata", [])
        if isinstance(sample_metadata, (list, tuple)):
            samples.extend(
                sample for sample in sample_metadata if isinstance(sample, Mapping)
            )
    return samples


def _cl_bench_repetitions(samples: list[Mapping[str, Any]], fallback: Any) -> str:
    if not samples:
        return _cell(fallback)
    return ",".join(_cell(sample.get("repetitions")) for sample in samples)


def _cl_bench_floor(samples: list[Mapping[str, Any]]) -> str:
    if not samples:
        return "n/a"
    met = sum(sample.get("minimum_duration_met") is True for sample in samples)
    per_sample = ",".join(
        "met" if sample.get("minimum_duration_met") is True else "missed"
        for sample in samples
    )
    return f"{per_sample} ({met}/{len(samples)})"


def _cl_bench_load(samples: list[Mapping[str, Any]]) -> tuple[str, bool]:
    values = []
    high = False
    for sample in samples:
        start = sample.get("load_average_process_start")
        end = sample.get("load_average_process_end")
        valid_start = isinstance(start, (list, tuple)) and len(start) == 3
        valid_end = isinstance(end, (list, tuple)) and len(end) == 3
        if not valid_start and not valid_end:
            values.append("n/a")
            continue
        start_text = "/".join(_cell(value) for value in start) if valid_start else "n/a"
        end_text = "/".join(_cell(value) for value in end) if valid_end else "n/a"
        rendered = f"start {start_text}; end {end_text}"
        is_high = any(
            isinstance(load[0], (int, float)) and load[0] > 80
            for load, valid in ((start, valid_start), (end, valid_end))
            if valid
        )
        high = high or is_high
        values.append(rendered)
    return "; ".join(values) if values else "n/a", high


def _cl_bench_noise_inconclusive(
    comparisons: Mapping[str, Any], name: str, comparison_mode: str | None
) -> bool:
    benchmark = comparisons.get(name, {})
    if not isinstance(benchmark, Mapping):
        return False
    modes = (comparison_mode,) if comparison_mode else ("off_over_sbcl", "default_over_sbcl")
    return any(
        isinstance(benchmark.get(mode), Mapping)
        and benchmark[mode].get("noise_below") is True
        for mode in modes
        if mode is not None
    )


def render_cl_bench_table(report: Mapping[str, Any]) -> str:
    scores = report.get("scores", {})
    stats_by_runtime = scores.get("benchmark_statistics_seconds", {})
    ratios = scores.get("benchmark_ncl_over_sbcl_ratio", {})
    comparisons = scores.get("benchmark_comparisons", {})
    names = list(stats_by_runtime.get("ncl_off", {}))
    ncl_sha, sbcl_version, host, load_average = _measurement_cells(report)
    protocol = report.get("protocol", {})
    minimum_policy = protocol.get("minimum_sample_duration_policy", {})
    repetitions = protocol.get("inner_repetitions_per_sample", "n/a")
    target = (
        _cell(minimum_policy.get("target_seconds"))
        if isinstance(minimum_policy, Mapping)
        else "n/a"
    )
    lines = [
        f"| Case | Implementation | Status / failure reason | Samples | Median (s) | Range (s) | NCL/SBCL ratio | Noise | Calls per timing sample | {target}s floor | Process-start/end load average (1/5/15m) | NCL SHA | SBCL version | Host | Load average (1/5/15m) |",
        "|---|---|---|---:|---:|---:|---:|---|---:|---|---|---|---|---|---|",
    ]
    implementations = (
        ("ncl_off", "NCL off", "off_over_sbcl", "off"),
        ("ncl_default", "NCL default", "default_over_sbcl", "default"),
        ("sbcl", "SBCL", None, None),
    )
    for name in names:
        for runtime, label, comparison_mode, run_mode in implementations:
            stats = stats_by_runtime.get(runtime, {}).get(name, {})
            comparison = comparisons.get(name, {}).get(comparison_mode, {}) if comparison_mode else {}
            ratio = ratios.get(comparison_mode, {}).get(name) if comparison_mode else None
            result = _cl_bench_failure(report, name, run_mode, label)
            sample_metadata = _cl_bench_sample_metadata(report, name, run_mode)
            sample_load, high_load = _cl_bench_load(sample_metadata)
            if result == "passed" and high_load and _cl_bench_noise_inconclusive(
                comparisons, name, comparison_mode
            ):
                result = "load 高で未確定"
            sample_count = stats.get("sample_count")
            if result == "passed" and sample_count == 0:
                result = f"{label} fail: no positive timed samples"
            elif result == "passed" and sample_count is None:
                result = "n/a"
            lines.append(
                f"| {_cell(name)} | {label} | {_cell(result)} | {_cell(stats.get('sample_count'))} "
                f"| {_cell(stats.get('median_seconds'))} | {_cell(stats.get('range_seconds'))} "
                f"| {_cell(ratio)} | {_noise_label(comparison.get('noise_below')) if comparison_mode else 'n/a'} "
                f"| {_cl_bench_repetitions(sample_metadata, repetitions)} "
                f"| {_cl_bench_floor(sample_metadata)} | {sample_load} "
                f"| {ncl_sha} | {sbcl_version} | {host} | {load_average} |"
            )
    return "\n".join(lines)


def render_micro_table(report: Mapping[str, Any]) -> str:
    cases = report.get("cases", {})
    ncl_sha, sbcl_version, host, load_average = _measurement_cells(report)
    lines = [
        "| Case | Implementation | Status / failure reason | Samples | Median (s) | Range (s) | Matched pairs | Pair median Δ NCL−SBCL (s) | Wins NCL/SBCL/tie | Noise | NCL/SBCL repetitions per sample | NCL SHA | SBCL version | Host | Load average (1/5/15m) |",
        "|---|---|---|---:|---:|---:|---:|---:|---:|---|---|---|---|---|---|",
    ]
    for name, case in cases.items():
        pair = case.get("paired_comparison", {})
        pair_count = f"{pair.get('sample_count', 'n/a')}/{pair.get('expected_sample_count', 'n/a')}"
        wins = f"{pair.get('ncl_wins', 'n/a')}/{pair.get('sbcl_wins', 'n/a')}/{pair.get('ties', 'n/a')}"
        engines = case.get("engines", {})
        repetitions = "/".join(
            _repetition_counts(engines.get(engine, {}).get("samples", []))
            for engine in ("ncl", "sbcl")
        )
        for engine in ("ncl", "sbcl"):
            stats = engines.get(engine, {})
            status = _sample_status(stats.get("samples", []), engine.upper(), stats.get("status"))
            lines.append(
                f"| {_cell(name)} | {engine.upper()} | {_cell(status)} | {_cell(stats.get('sample_count'))} "
                f"| {_cell(stats.get('median_seconds'))} | {_cell(stats.get('range_seconds'))} "
                f"| {pair_count} | {_cell(pair.get('median_delta_seconds'))} | {wins} "
                f"| {_noise_label(pair.get('noise_below'))} | {repetitions} "
                f"| {ncl_sha} | {sbcl_version} | {host} | {load_average} |"
            )
    return "\n".join(lines)


def render_compile_table(report: Mapping[str, Any]) -> str:
    runtimes = report.get("runtimes", {})
    ncl_sha, sbcl_version, host, load_average = _measurement_cells(report)
    lines = [
        "| Runtime | Compile samples | Total median (s) | Total minimum (s) | Total range (s) | NCL/SBCL ratio | Noise | Startup samples | Startup median (s) | Startup range (s) | Compiled files | Lines/s | Peak RSS (bytes) | RSS samples | Binary size (bytes) | Repetitions by file/sample | NCL SHA | SBCL version | Host | Load average (1/5/15m) |",
        "|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|---|",
    ]
    total_comparison = report.get("total_comparison", {})
    for runtime, metrics in runtimes.items():
        startup = metrics.get("startup", {})
        ratio = total_comparison.get("ratio") if runtime == "ncl" else None
        noise = _noise_label(total_comparison.get("noise_below")) if runtime == "ncl" else "n/a"
        lines.append(
            f"| {runtime} | {_cell(metrics.get('total_compile_sample_count'))} "
            f"| {_cell(metrics.get('total_compile_seconds'))} | {_cell(metrics.get('minimum_total_compile_seconds'))} "
            f"| {_cell(metrics.get('total_compile_range_seconds'))} "
            f"| {_cell(ratio)} | {noise} | {_cell(startup.get('sample_count'))} | {_cell(startup.get('median_seconds'))} "
            f"| {_cell(startup.get('range_seconds'))} | {_cell(metrics.get('compiled_file_count'))} "
            f"| {_cell(metrics.get('lines_per_second'))} | {_cell(metrics.get('peak_rss_bytes'))} "
            f"| {_cell(metrics.get('peak_rss_sample_count'))}/{_cell(metrics.get('peak_rss_expected_sample_count'))} "
            f"| {_cell(metrics.get('binary_size_bytes'))} | {_compile_repetitions_by_file(metrics)} "
            f"| {ncl_sha} | {sbcl_version} | {host} | {load_average} |"
        )
    lines.extend([
        "",
        "| Case | Implementation | Status / failure reason | Samples | Median (s) | Range (s) | NCL/SBCL ratio | Noise | NCL/SBCL repetitions per sample | NCL SHA | SBCL version | Host | Load average (1/5/15m) |",
        "|---|---|---|---:|---:|---:|---:|---|---|---|---|---|---|",
    ])
    comparisons = report.get("case_comparisons", {})
    file_rows = {}
    for runtime, metrics in runtimes.items():
        for row in metrics.get("files", []):
            label = f"{row.get('suite')}/{row.get('file')}"
            file_rows.setdefault(label, {})[runtime] = row
    for runtime, metrics in runtimes.items():
        for row in metrics.get("files", []):
            label = f"{row.get('suite')}/{row.get('file')}"
            comparison = comparisons.get(label, {})
            ratio = comparison.get("ratio") if runtime == "ncl" else None
            pair_rows = file_rows.get(label, {})
            repetitions = "/".join(
                _repetition_counts(pair_rows.get(engine, {}).get("compile_repetitions_samples", []))
                for engine in ("ncl", "sbcl")
            )
            status = _compile_status(row, runtime)
            lines.append(
                f"| {_cell(label)} | {runtime.upper()} | {_cell(status)} | {_cell(row.get('sample_count'))} "
                f"| {_cell(row.get('median_seconds'))} | {_cell(row.get('range_seconds'))} "
                f"| {_cell(ratio)} | {_noise_label(comparison.get('noise_below')) if runtime == 'ncl' else 'n/a'} "
                f"| {repetitions} | {ncl_sha} | {sbcl_version} | {host} | {load_average} |"
            )
    return "\n".join(lines)


def render_ab_comparison(report: Mapping[str, Any]) -> str:
    comparison = report.get("comparison", {})
    lines = [
        "| Comparison | Samples | Median total (s) | Range (s) | Candidate−baseline median (s) | Noise width (s) | Noise |",
        "|---|---:|---:|---:|---:|---:|---|",
    ]
    for name in ("baseline", "candidate"):
        stats = report.get(name, {})
        lines.append(
            f"| {name} | {_cell(stats.get('sample_count'))} | {_cell(stats.get('median_seconds'))} "
            f"| {_cell(stats.get('range_seconds'))} | n/a | n/a | n/a |"
        )
    lines.append(
        f"| A/B difference | {_cell(report.get('runs_per_binary'))} | n/a | n/a "
        f"| {_cell(comparison.get('median_difference_seconds'))} "
        f"| {_cell(comparison.get('absolute_noise_width_seconds'))} "
        f"| {_noise_label(comparison.get('noise_below'))} |"
    )
    lines.extend([
        "",
        "| Case | Baseline samples | Baseline median (s) | Baseline range (s) | Candidate samples | Candidate median (s) | Candidate range (s) | Candidate−baseline median (s) | Noise |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---|",
    ])
    for name, data in report.get("file_comparisons", {}).items():
        baseline = data.get("baseline", {})
        candidate = data.get("candidate", {})
        case_comparison = data.get("comparison", {})
        lines.append(
            f"| {_cell(name)} | {_cell(baseline.get('sample_count'))} "
            f"| {_cell(baseline.get('median_seconds'))} | {_cell(baseline.get('range_seconds'))} "
            f"| {_cell(candidate.get('sample_count'))} | {_cell(candidate.get('median_seconds'))} "
            f"| {_cell(candidate.get('range_seconds'))} "
            f"| {_cell(case_comparison.get('median_difference_seconds'))} "
            f"| {_noise_label(case_comparison.get('noise_below'))} |"
        )
    return "\n".join(lines)
