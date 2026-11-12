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
        return "noise 以下"
    if value is False:
        return "range 超"
    return "n/a"


def render_cl_bench_table(report: Mapping[str, Any]) -> str:
    scores = report.get("scores", {})
    stats_by_runtime = scores.get("benchmark_statistics_seconds", {})
    ratios = scores.get("benchmark_ncl_over_sbcl_ratio", {})
    comparisons = scores.get("benchmark_comparisons", {})
    names = list(stats_by_runtime.get("ncl_off", {}))
    lines = [
        "| Case | Implementation | Samples | Median (s) | Range (s) | NCL/SBCL ratio | Noise |",
        "|---|---|---:|---:|---:|---:|---|",
    ]
    implementations = (
        ("ncl_off", "NCL off", "off_over_sbcl"),
        ("ncl_default", "NCL default", "default_over_sbcl"),
        ("sbcl", "SBCL", None),
    )
    for name in names:
        for runtime, label, mode in implementations:
            stats = stats_by_runtime.get(runtime, {}).get(name, {})
            comparison = comparisons.get(name, {}).get(mode, {}) if mode else {}
            ratio = ratios.get(mode, {}).get(name) if mode else None
            lines.append(
                f"| {_cell(name)} | {label} | {_cell(stats.get('sample_count'))} "
                f"| {_cell(stats.get('median_seconds'))} | {_cell(stats.get('range_seconds'))} "
                f"| {_cell(ratio)} | {_noise_label(comparison.get('noise_below')) if mode else 'n/a'} |"
            )
    return "\n".join(lines)


def render_micro_table(report: Mapping[str, Any]) -> str:
    cases = report.get("cases", {})
    lines = [
        "| Case | Implementation | Samples | Median (s) | Range (s) | Matched pairs | Pair median Δ NCL−SBCL (s) | Wins NCL/SBCL/tie | Noise |",
        "|---|---|---:|---:|---:|---:|---:|---:|---|",
    ]
    for name, case in cases.items():
        pair = case.get("paired_comparison", {})
        pair_count = f"{pair.get('sample_count', 'n/a')}/{pair.get('expected_sample_count', 'n/a')}"
        wins = f"{pair.get('ncl_wins', 'n/a')}/{pair.get('sbcl_wins', 'n/a')}/{pair.get('ties', 'n/a')}"
        for engine in ("ncl", "sbcl"):
            stats = case.get("engines", {}).get(engine, {})
            lines.append(
                f"| {_cell(name)} | {engine.upper()} | {_cell(stats.get('sample_count'))} "
                f"| {_cell(stats.get('median_seconds'))} | {_cell(stats.get('range_seconds'))} "
                f"| {pair_count} | {_cell(pair.get('median_delta_seconds'))} | {wins} "
                f"| {_noise_label(pair.get('noise_below'))} |"
            )
    return "\n".join(lines)


def render_compile_table(report: Mapping[str, Any]) -> str:
    runtimes = report.get("runtimes", {})
    lines = [
        "| Runtime | Compile samples | Total median (s) | Total minimum (s) | Total range (s) | NCL/SBCL ratio | Noise | Startup samples | Startup median (s) | Startup range (s) | Compiled files | Lines/s | Peak RSS (bytes) | RSS samples | Binary size (bytes) |",
        "|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|",
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
            f"| {_cell(metrics.get('binary_size_bytes'))} |"
        )
    lines.extend([
        "",
        "| Case | Implementation | Samples | Median (s) | Range (s) | NCL/SBCL ratio | Noise |",
        "|---|---|---:|---:|---:|---:|---|",
    ])
    comparisons = report.get("case_comparisons", {})
    for runtime, metrics in runtimes.items():
        for row in metrics.get("files", []):
            label = f"{row.get('suite')}/{row.get('file')}"
            comparison = comparisons.get(label, {})
            ratio = comparison.get("ratio") if runtime == "ncl" else None
            lines.append(
                f"| {_cell(label)} | {runtime.upper()} | {_cell(row.get('sample_count'))} "
                f"| {_cell(row.get('median_seconds'))} | {_cell(row.get('range_seconds'))} "
                f"| {_cell(ratio)} | {_noise_label(comparison.get('noise_below')) if runtime == 'ncl' else 'n/a'} |"
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
