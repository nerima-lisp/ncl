#!/usr/bin/env python3
"""Validate and merge complete SBCL and NCL ANSI-test shard reports."""

from __future__ import annotations

import argparse
from collections import Counter
from datetime import date
import json
from pathlib import Path
from typing import Any, Mapping, Sequence

import ansi_test_inclusion as inclusion


COUNTED_STATUSES = ("passed", "failed", "error", "timeout", "harness_error", "unexecuted")


def _read_report(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"report must be a JSON object: {path}")
    return value


def _check_identity(
    report: Mapping[str, Any],
    *,
    implementation: str,
    shard_index: int,
    shard_count: int,
) -> None:
    if report.get("schema_version") != 1 or report.get("suite") != "ansi-test":
        raise ValueError("shard report has an unsupported schema or suite")
    if report.get("implementation") != implementation:
        raise ValueError(f"expected {implementation} shard report")
    if report.get("status") != "complete":
        raise ValueError("shard report is incomplete")
    if report.get("shard_index") != shard_index or report.get("shard_count") != shard_count:
        raise ValueError("shard report identity does not match its artifact")
    if report.get("source_commit") != inclusion.SOURCES["ansi-test"]["commit"]:
        raise ValueError("shard report uses a different ansi-test source commit")
    if not report.get("platform") or not report.get("measured_on"):
        raise ValueError("shard report is missing platform or measurement date")
    try:
        date.fromisoformat(str(report["measured_on"]))
    except ValueError as error:
        raise ValueError("shard report has an invalid measurement date") from error


def _count_rows(report: Mapping[str, Any], rows: Sequence[Mapping[str, Any]]) -> None:
    counts = Counter(row.get("status") for row in rows)
    for status in COUNTED_STATUSES:
        if int(report.get(status, -1)) != counts[status]:
            raise ValueError(f"shard {status} count does not match its test rows")


def _manifest(report: Mapping[str, Any]) -> dict[str, str]:
    raw_manifest = report.get("manifest")
    if not isinstance(raw_manifest, list):
        raise ValueError("SBCL shard is missing the complete active-test manifest")
    manifest: dict[str, str] = {}
    for entry in raw_manifest:
        if not isinstance(entry, dict):
            raise ValueError("SBCL manifest contains a non-object entry")
        name, chapter = entry.get("name"), entry.get("chapter")
        if not isinstance(name, str) or not name or not isinstance(chapter, str) or not chapter:
            raise ValueError("SBCL manifest contains an invalid test entry")
        if name in manifest:
            raise ValueError(f"SBCL manifest contains duplicate test: {name}")
        manifest[name] = chapter
    if int(report.get("active_total", -1)) != len(manifest):
        raise ValueError("SBCL active-test count does not match its manifest")
    if int(report.get("registered", -1)) != (
        len(manifest) + int(report.get("disabled", -1)) + int(report.get("removed", -1))
    ):
        raise ValueError("SBCL registration counts do not balance")
    if int(report.get("removed", -1)) != len(inclusion.REMOVED_TEST_NAMES):
        raise ValueError("SBCL shard has an unexpected removed-test count")
    disabled_tests = report.get("disabled_tests")
    if (
        not isinstance(disabled_tests, list)
        or disabled_tests != sorted(set(disabled_tests))
        or len(disabled_tests) != int(report.get("disabled", -1))
    ):
        raise ValueError("SBCL disabled-test metadata is invalid")
    if set(manifest) & set(disabled_tests):
        raise ValueError("SBCL active and disabled test names overlap")
    if set(manifest) & set(inclusion.REMOVED_TEST_NAMES):
        raise ValueError("SBCL active manifest contains a runner-removed test")
    return manifest


def _validate_baseline_rows(
    report: Mapping[str, Any], manifest: Mapping[str, str], shard_index: int, shard_count: int,
) -> dict[str, Mapping[str, Any]]:
    rows = report.get("tests")
    if not isinstance(rows, list) or int(report.get("active", -1)) != len(rows):
        raise ValueError("SBCL shard active count does not match its test rows")
    by_name: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("SBCL test row is not an object")
        name = row.get("name")
        if not isinstance(name, str) or name not in manifest or name in by_name:
            raise ValueError("SBCL shard contains an unknown or duplicate test")
        if row.get("chapter") != manifest[name]:
            raise ValueError(f"SBCL test chapter differs from manifest: {name}")
        if row.get("status") not in inclusion.RESULT_STATUSES:
            raise ValueError(f"SBCL test has an invalid status: {name}")
        if inclusion.shard_for_test(name, shard_count) != shard_index:
            raise ValueError(f"SBCL test is assigned to the wrong shard: {name}")
        by_name[name] = row
    expected_names = {
        name for name in manifest
        if inclusion.shard_for_test(name, shard_count) == shard_index
    }
    if set(by_name) != expected_names:
        raise ValueError("SBCL shard is missing or has extra assigned test rows")
    if int(report.get("unexecuted", -1)) != 0 or any(
        row.get("status") == "unexecuted" for row in rows
    ):
        raise ValueError("SBCL shard contains unexecuted active tests")
    _count_rows(report, rows)
    if report.get("process_exit_status") != 0:
        raise ValueError("SBCL inventory process did not exit successfully")
    if not any(row.get("status") == "passed" for row in rows):
        raise ValueError("SBCL shard has no passed tests")
    return by_name


def _validate_compare_rows(
    report: Mapping[str, Any],
    expected: Mapping[str, str],
    shard_index: int,
    shard_count: int,
    artifact_dir: Path,
) -> dict[str, Mapping[str, Any]]:
    rows = report.get("tests")
    if not isinstance(rows, list) or int(report.get("active", -1)) != len(rows):
        raise ValueError("NCL shard active count does not match its test rows")
    by_name: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("NCL test row is not an object")
        name = row.get("name")
        if not isinstance(name, str) or name not in expected or name in by_name:
            raise ValueError("NCL shard contains an unexpected or duplicate test")
        if row.get("chapter") != expected[name]:
            raise ValueError(f"NCL test chapter differs from SBCL baseline: {name}")
        if row.get("status") not in inclusion.RESULT_STATUSES:
            raise ValueError(f"NCL test has an invalid status: {name}")
        if inclusion.shard_for_test(name, shard_count) != shard_index:
            raise ValueError(f"NCL test is assigned to the wrong shard: {name}")
        by_name[name] = row
    expected_names = {
        name for name in expected
        if inclusion.shard_for_test(name, shard_count) == shard_index
    }
    if set(by_name) != expected_names:
        raise ValueError("NCL shard is missing or has extra assigned test rows")
    if int(report.get("registered", -1)) != len(expected_names):
        raise ValueError("NCL shard registration count differs from SBCL passing rows")
    _count_rows(report, rows)
    actual_comparison = inclusion.compare_results(
        {name: expected[name] for name in expected_names}, by_name,
    )
    if report.get("comparison") != actual_comparison:
        raise ValueError("NCL shard comparison does not match its test rows")
    try:
        process_status = int((artifact_dir / "ncl-exit-status").read_text(encoding="utf-8").strip())
    except (OSError, ValueError) as error:
        raise ValueError("NCL shard is missing its comparison process exit status") from error
    if process_status != inclusion.comparison_exit_status(actual_comparison):
        raise ValueError("NCL comparison process exit status does not match its test rows")
    return by_name


def merge_shards(
    artifacts_dir: Path,
    shard_count: int,
    baseline_prefix: Path,
    baseline_report_path: Path,
    comparison_report_path: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    if shard_count < 1:
        raise ValueError("shard count must be positive")
    baseline_reports: list[dict[str, Any]] = []
    compare_reports: list[dict[str, Any]] = []
    for index in range(shard_count):
        artifact = artifacts_dir / f"ansi-shard-{index}"
        baseline_reports.append(_read_report(artifact / "sbcl-report.json"))
        compare_reports.append(_read_report(artifact / "ncl-report.json"))

    baseline_manifest: dict[str, str] | None = None
    baseline_rows: dict[str, Mapping[str, Any]] = {}
    disabled_tests: list[str] | None = None
    identity: tuple[str, str] | None = None
    for index, report in enumerate(baseline_reports):
        _check_identity(
            report, implementation="sbcl", shard_index=index, shard_count=shard_count,
        )
        if report.get("runtime_version") != "SBCL 2.6.0":
            raise ValueError("SBCL shard does not use SBCL 2.6.0")
        current_identity = (str(report["measured_on"]), str(report["platform"]))
        if identity is None:
            identity = current_identity
        elif current_identity != identity:
            raise ValueError("SBCL shards differ in measurement date or platform")
        current_manifest = _manifest(report)
        if baseline_manifest is None:
            baseline_manifest = current_manifest
        elif current_manifest != baseline_manifest:
            raise ValueError("SBCL shards have different active-test manifests")
        current_disabled = report["disabled_tests"]
        if disabled_tests is None:
            disabled_tests = current_disabled
        elif current_disabled != disabled_tests:
            raise ValueError("SBCL shards have different disabled-test metadata")
        current_rows = _validate_baseline_rows(report, current_manifest, index, shard_count)
        overlap = baseline_rows.keys() & current_rows.keys()
        if overlap:
            raise ValueError(f"duplicate SBCL baseline rows: {', '.join(sorted(overlap)[:3])}")
        baseline_rows.update(current_rows)
    if baseline_manifest is None or identity is None or disabled_tests is None:
        raise ValueError("no complete SBCL shard set was supplied")
    if set(baseline_rows) != set(baseline_manifest):
        raise ValueError("merged SBCL rows do not cover the complete active-test manifest")

    baseline_tests = [baseline_rows[name] for name in baseline_manifest]
    baseline_tests.sort(key=lambda row: (row["chapter"], row["name"]))
    baseline_counts = Counter(row["status"] for row in baseline_tests)
    first_baseline = baseline_reports[0]
    merged_baseline: dict[str, Any] = {
        "schema_version": 1,
        "suite": "ansi-test",
        "implementation": "sbcl",
        "runtime_version": "SBCL 2.6.0",
        "platform": identity[1],
        "measured_on": identity[0],
        "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
        "registered": first_baseline["registered"],
        "active": len(baseline_tests),
        "active_total": len(baseline_tests),
        "disabled": first_baseline["disabled"],
        "disabled_tests": disabled_tests,
        "removed": first_baseline["removed"],
        "passed": baseline_counts["passed"],
        "failed": baseline_counts["failed"],
        "error": baseline_counts["error"],
        "timeout": baseline_counts["timeout"],
        "harness_error": baseline_counts["harness_error"],
        "unexecuted": baseline_counts["unexecuted"],
        "status": "complete",
        "tests": baseline_tests,
        "manifest": [
            {"name": name, "chapter": chapter}
            for name, chapter in sorted(baseline_manifest.items(), key=lambda item: (item[1], item[0]))
        ],
        "process_exit_status": 0,
        "first_error": next(
            (row.get("reason") for row in baseline_tests if row.get("reason")), None,
        ),
        "shard_count": shard_count,
    }
    passing_baseline = {
        row["name"]: row["chapter"] for row in baseline_tests if row["status"] == "passed"
    }
    compare_identity: tuple[str, str, str] | None = None
    observed_rows: dict[str, Mapping[str, Any]] = {}
    for index, report in enumerate(compare_reports):
        _check_identity(
            report, implementation="ncl", shard_index=index, shard_count=shard_count,
        )
        current_identity = (
            str(report["measured_on"]), str(report["platform"]), str(report.get("runtime_version", "")),
        )
        if not current_identity[2]:
            raise ValueError("NCL shard is missing its runtime version")
        if compare_identity is None:
            compare_identity = current_identity
        elif current_identity != compare_identity:
            raise ValueError("NCL shards differ in date, platform, or runtime version")
        if current_identity[:2] != identity:
            raise ValueError("SBCL and NCL shards differ in measurement date or platform")
        expected_for_shard = {
            name: chapter for name, chapter in passing_baseline.items()
            if inclusion.shard_for_test(name, shard_count) == index
        }
        current_rows = _validate_compare_rows(
            report, expected_for_shard, index, shard_count,
            artifacts_dir / f"ansi-shard-{index}",
        )
        overlap = observed_rows.keys() & current_rows.keys()
        if overlap:
            raise ValueError(f"duplicate NCL comparison rows: {', '.join(sorted(overlap)[:3])}")
        observed_rows.update(current_rows)
    if compare_identity is None or set(observed_rows) != set(passing_baseline):
        raise ValueError("merged NCL rows do not cover the complete SBCL passing baseline")

    observed_tests = list(observed_rows.values())
    observed_tests.sort(key=lambda row: (row["chapter"], row["name"]))
    observed_counts = Counter(row["status"] for row in observed_tests)
    comparison = inclusion.compare_results(passing_baseline, observed_rows)
    merged_comparison: dict[str, Any] = {
        "schema_version": 1,
        "suite": "ansi-test",
        "implementation": "ncl",
        "runtime_version": compare_identity[2],
        "platform": compare_identity[1],
        "measured_on": compare_identity[0],
        "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
        "registered": len(passing_baseline),
        "active": len(observed_tests),
        "disabled": 0,
        "passed": observed_counts["passed"],
        "failed": observed_counts["failed"],
        "error": observed_counts["error"],
        "timeout": observed_counts["timeout"],
        "harness_error": observed_counts["harness_error"],
        "unexecuted": observed_counts["unexecuted"],
        "status": "complete",
        "tests": observed_tests,
        "comparison": comparison,
        "first_error": next(
            (row.get("reason") for row in observed_tests if row.get("reason")), None,
        ),
        "shard_count": shard_count,
    }
    inclusion._write_baseline_files(merged_baseline, baseline_prefix)
    baseline_report_path.parent.mkdir(parents=True, exist_ok=True)
    baseline_report_path.write_text(
        json.dumps(merged_baseline, indent=2, sort_keys=True) + "\n", encoding="utf-8",
    )
    comparison_report_path.parent.mkdir(parents=True, exist_ok=True)
    comparison_report_path.write_text(
        json.dumps(merged_comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8",
    )
    return merged_baseline, merged_comparison


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts-dir", type=Path, required=True)
    parser.add_argument("--shards", type=int, required=True)
    parser.add_argument("--baseline-prefix", type=Path, required=True)
    parser.add_argument("--baseline-report", type=Path, required=True)
    parser.add_argument("--comparison-report", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        merge_shards(
            args.artifacts_dir, args.shards, args.baseline_prefix,
            args.baseline_report, args.comparison_report,
        )
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as error:
        parser.exit(1, f"ansi-test shard merge: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
