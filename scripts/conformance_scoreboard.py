#!/usr/bin/env python3
"""Run the small, reproducible M2 conformance scoreboard.

The benchmark commands are deliberately supplied by the caller.  This keeps
the source acquisition deterministic while allowing the test suite to replace
the process runner without downloading or executing Lisp implementations.
"""

from __future__ import annotations

import argparse
from collections import Counter
import dataclasses
import json
import math
import os
import re
import shlex
import signal
import shutil
import socket
import subprocess
import sys
import tempfile
from datetime import date
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence


DEFAULT_CONNECT_TIMEOUT = 120.0
DEFAULT_SOURCE_TIMEOUT = 300.0
DEFAULT_COMMAND_TIMEOUT = 900.0
DEFAULT_SOURCE_RETRIES = 2
MAX_STDERR_TAIL_CHARS = 4000
CL_BENCH_STAGES = frozenset({"load", "register", "execute", "finish"})
CL_BENCH_STATUSES = frozenset({"passed", "failed", "disabled", "timeout"})
CL_BENCH_CHECKPOINT_ENV = "NCL_CL_BENCH_CHECKPOINT"
ANSI_POLICIES = ("safety-3", "speed-3-safety-1")
SBCL_BASELINE_VERSION = "SBCL 2.6.10"

_QUOTED_PATH_PATTERNS = tuple(
    re.compile(
        re.escape(quote) + r"/[^" + re.escape(quote) + r"\r\n]*" + re.escape(quote)
    )
    for quote in ("'", '"', "`")
)
_FILE_URI_PATTERN = re.compile(r"(?i)(?<![\w.-])file:[^\r\n]*")
_UNQUOTED_ABSOLUTE_PATH_LINE_PATTERN = re.compile(r"(?<![\w:/])/(?!/)[^\r\n]*")


SOURCES = {
    "ansi-test": {
        "url": "https://gitlab.common-lisp.net/ansi-test/ansi-test.git",
        "mirrors": ("https://github.com/clasp-developers/ansi-test.git",),
        "commit": "ca06bd919661af162c67407c9d994e881870bdb3",
    },
    "cl-bench": {
        "url": "https://gitlab.common-lisp.net/ansi-test/cl-bench.git",
        "mirrors": ("https://github.com/clasp-developers/cl-bench.git",),
        "commit": "553fbcdf88d2ca4e340a0cdf02679055f3279c8f",
    },
}


@dataclasses.dataclass(frozen=True)
class ProcessResult:
    status: str
    returncode: int | None
    stdout: str = ""
    stderr: str = ""


def classify_process(returncode: int | None, timed_out: bool = False) -> str:
    """Classify a child process without conflating timeout, crash, and failure."""
    if timed_out:
        return "timeout"
    if returncode is None:
        return "crash"
    if returncode < 0 or returncode >= 128:
        return "crash"
    return "passed" if returncode == 0 else "failed"


def run_command(
    command: Sequence[str],
    *,
    cwd: Path,
    timeout: float,
    runner: Callable[..., Any] = subprocess.run,
    env: Mapping[str, str] | None = None,
) -> ProcessResult:
    """Run a command and preserve enough output to diagnose its classification."""
    try:
        options = {
            "cwd": cwd,
            "capture_output": True,
            "text": True,
            "timeout": timeout,
            "check": False,
        }
        if env is not None:
            options["env"] = dict(env)
        completed = runner(list(command), **options)
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout or ""
        stderr = error.stderr or ""
        if isinstance(stdout, bytes):
            stdout = stdout.decode(errors="replace")
        if isinstance(stderr, bytes):
            stderr = stderr.decode(errors="replace")
        return ProcessResult("timeout", None, stdout, stderr)
    except (OSError, ValueError) as error:
        return ProcessResult("crash", None, "", str(error))
    return ProcessResult(
        classify_process(completed.returncode), completed.returncode,
        completed.stdout or "", completed.stderr or "",
    )


def geometric_mean(values: Sequence[float]) -> float:
    """Return the geometric mean of positive measurements."""
    if not values:
        raise ValueError("geometric mean requires at least one value")
    if any(value <= 0 or not math.isfinite(value) for value in values):
        raise ValueError("geometric mean requires finite positive values")
    return math.exp(sum(math.log(value) for value in values) / len(values))


_RETRYABLE_GIT_ERRORS = (
    OSError,
    subprocess.CalledProcessError,
    subprocess.TimeoutExpired,
)


def _git_environment(connect_timeout: float) -> dict[str, str]:
    """Configure Git's HTTP connection timeout without changing global config."""
    environment = os.environ.copy()
    config = f"'http.connectTimeout'='{int(connect_timeout)}'"
    previous = environment.get("GIT_CONFIG_PARAMETERS")
    environment["GIT_CONFIG_PARAMETERS"] = f"{previous} {config}".strip() if previous else config
    environment["GIT_TERMINAL_PROMPT"] = "0"
    return environment


def _run_git(
    command: Sequence[str],
    *,
    timeout: float,
    connect_timeout: float,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(command),
        check=True,
        capture_output=True,
        text=True,
        timeout=timeout,
        env=_git_environment(connect_timeout),
    )


def _run_git_with_retries(
    command: Sequence[str],
    *,
    timeout: float,
    connect_timeout: float,
    retries: int,
) -> subprocess.CompletedProcess[str]:
    attempts = max(1, retries + 1)
    last_error: Exception | None = None
    for _ in range(attempts):
        try:
            return _run_git(
                command, timeout=timeout, connect_timeout=connect_timeout,
            )
        except _RETRYABLE_GIT_ERRORS as error:
            last_error = error
    if last_error is None:
        raise RuntimeError("git command failed without an error")
    raise last_error


def _remove_failed_clone(destination: Path) -> None:
    """Remove only the cache entry created by a failed clone attempt."""
    if destination.is_symlink() or destination.is_file():
        destination.unlink()
    elif destination.is_dir():
        shutil.rmtree(destination)


def _clone_with_retries(
    url: str,
    destination: Path,
    *,
    timeout: float,
    connect_timeout: float,
    retries: int,
) -> None:
    attempts = max(1, retries + 1)
    last_error: Exception | None = None
    for _ in range(attempts):
        try:
            _run_git(
                ["git", "clone", url, str(destination)],
                timeout=timeout,
                connect_timeout=connect_timeout,
            )
            return
        except _RETRYABLE_GIT_ERRORS as error:
            last_error = error
            if destination.exists():
                _remove_failed_clone(destination)
    if last_error is None:
        raise RuntimeError("git clone failed without an error")
    raise last_error


def _checkout_from_url(
    url: str,
    destination: Path,
    commit: str,
    *,
    timeout: float,
    connect_timeout: float,
    retries: int,
) -> None:
    cloned = not destination.exists()
    try:
        if cloned:
            _clone_with_retries(
                url,
                destination,
                timeout=timeout,
                connect_timeout=connect_timeout,
                retries=retries,
            )
            fetch_source = "origin"
        else:
            fetch_source = url
        _run_git_with_retries(
            ["git", "-C", str(destination), "fetch", "--quiet", fetch_source, commit],
            timeout=timeout,
            connect_timeout=connect_timeout,
            retries=retries,
        )
        _run_git_with_retries(
            ["git", "-C", str(destination), "checkout", "--quiet", "--detach", commit],
            timeout=timeout,
            connect_timeout=connect_timeout,
            retries=retries,
        )
        actual = _run_git_with_retries(
            ["git", "-C", str(destination), "rev-parse", "HEAD"],
            timeout=timeout,
            connect_timeout=connect_timeout,
            retries=retries,
        ).stdout.strip()
        if actual != commit:
            raise RuntimeError(f"expected {commit}, got {actual}")
    except Exception:
        if cloned and destination.exists():
            _remove_failed_clone(destination)
        raise


def checkout_source(
    name: str,
    cache_dir: Path,
    *,
    timeout: float = DEFAULT_SOURCE_TIMEOUT,
    connect_timeout: float = DEFAULT_CONNECT_TIMEOUT,
    retries: int = DEFAULT_SOURCE_RETRIES,
) -> Path:
    """Acquire a pinned source with retries, mirror fallback, and hash verification."""
    try:
        source = SOURCES[name]
    except KeyError as error:
        raise ValueError(f"unknown source: {name}") from error
    destination = cache_dir / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    commit = source["commit"]
    if destination.exists():
        try:
            cached = _run_git_with_retries(
                ["git", "-C", str(destination), "rev-parse", "HEAD"],
                timeout=timeout,
                connect_timeout=connect_timeout,
                retries=0,
            ).stdout.strip()
        except _RETRYABLE_GIT_ERRORS:
            cached = ""
        if cached == commit:
            return destination
    errors: list[str] = []
    for url in (source["url"], *source.get("mirrors", ())):
        try:
            _checkout_from_url(
                url,
                destination,
                commit,
                timeout=timeout,
                connect_timeout=connect_timeout,
                retries=retries,
            )
            return destination
        except (OSError, subprocess.SubprocessError, RuntimeError) as error:
            errors.append(f"{url}: {error}")
    details = "; ".join(errors)
    raise RuntimeError(f"{name}: all source URLs failed: {details}")


def _json_output(result: ProcessResult) -> Mapping[str, Any]:
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ValueError("runner stdout must be a JSON object") from error
    if not isinstance(value, dict):
        raise ValueError("runner stdout must be a JSON object")
    return value


def _driver_times(
    value: Any,
    *,
    field: str,
    require_positive: bool = True,
) -> list[float]:
    if not isinstance(value, list):
        raise ValueError(f"cl-bench {field} must be an array")
    times: list[float] = []
    for sample in value:
        if isinstance(sample, bool) or not isinstance(sample, (int, float)):
            raise ValueError(f"cl-bench {field} must contain positive finite numbers")
        try:
            normalized = float(sample)
        except OverflowError as error:
            raise ValueError(
                f"cl-bench {field} must contain positive finite numbers"
            ) from error
        if (
            not math.isfinite(normalized)
            or normalized < 0
            or (require_positive and normalized == 0)
        ):
            raise ValueError(f"cl-bench {field} must contain positive finite numbers")
        times.append(normalized)
    return times


def _driver_wall_time(value: Any, *, field: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"cl-bench {field} must be a finite non-negative number")
    try:
        normalized = float(value)
    except OverflowError as error:
        raise ValueError(
            f"cl-bench {field} must be a finite non-negative number"
        ) from error
    if not math.isfinite(normalized) or normalized < 0:
        raise ValueError(f"cl-bench {field} must be a finite non-negative number")
    return normalized


def _validate_cl_bench_benchmark(
    name: str, record: Any
) -> tuple[dict[str, Any], list[float]]:
    if not isinstance(name, str) or not name:
        raise ValueError("cl-bench benchmark names must be non-empty strings")
    if not isinstance(record, dict) or set(record) != {
        "status", "stage", "times", "condition", "wall_time_seconds",
    }:
        raise ValueError(f"cl-bench benchmark {name} has an unexpected shape")
    row_status = record.get("status")
    if not isinstance(row_status, str) or row_status not in CL_BENCH_STATUSES:
        raise ValueError(f"cl-bench benchmark {name} has an invalid status")
    stage = record.get("stage")
    if not isinstance(stage, str) or stage not in CL_BENCH_STAGES:
        raise ValueError(f"cl-bench benchmark {name} has an invalid stage")
    condition = record.get("condition")
    if condition is not None and not isinstance(condition, str):
        raise ValueError(f"cl-bench benchmark {name} condition must be a string or null")
    row_times = _driver_times(
        record.get("times"),
        field=f"{name} times",
        require_positive=row_status not in ("failed", "timeout"),
    )
    wall_time_seconds = _driver_wall_time(
        record.get("wall_time_seconds"), field=f"{name} wall_time_seconds"
    )

    if row_status == "passed":
        if stage != "finish" or condition is not None or not row_times:
            raise ValueError(f"cl-bench passed benchmark {name} has invalid result data")
        passed_times = row_times
    elif row_status in ("failed", "timeout"):
        if not condition:
            raise ValueError(f"cl-bench {row_status} benchmark {name} has no condition")
        if row_status == "timeout" and row_times:
            raise ValueError(f"cl-bench timed out benchmark {name} has sample times")
        passed_times = []
    else:
        if row_times:
            raise ValueError(f"cl-bench disabled benchmark {name} has sample times")
        passed_times = []

    return {
        "status": row_status,
        "stage": stage,
        "times": row_times,
        "condition": condition,
        "wall_time_seconds": wall_time_seconds,
    }, passed_times


def validate_cl_bench_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate and normalize the shared cl-bench driver JSON contract."""
    if set(value) != {"status", "times", "benchmarks"}:
        raise ValueError("cl-bench report has an unexpected shape")
    status = value.get("status")
    if not isinstance(status, str) or status not in {"passed", "failed"}:
        raise ValueError("cl-bench report status must be passed or failed")
    flat_times = _driver_times(value.get("times"), field="times")
    reported = value.get("benchmarks")
    if not isinstance(reported, dict):
        raise ValueError("cl-bench benchmarks must be an object")

    benchmarks: dict[str, dict[str, Any]] = {}
    passed_times: list[float] = []
    has_failed = False
    for name, record in reported.items():
        benchmark, row_passed_times = _validate_cl_bench_benchmark(name, record)
        benchmarks[name] = benchmark
        passed_times.extend(row_passed_times)
        has_failed = has_failed or benchmark["status"] in ("failed", "timeout")

    if status == "passed" and (not benchmarks or not passed_times or has_failed):
        raise ValueError("cl-bench report passed with no benchmarks or failed rows")
    if Counter(flat_times) != Counter(passed_times):
        raise ValueError("cl-bench flat times differ from passed benchmark times")
    return {"status": status, "times": flat_times, "benchmarks": benchmarks}


def read_cl_bench_checkpoint(path: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    """Read complete JSONL checkpoint records and ignore a torn final line."""
    try:
        data = path.read_bytes()
    except OSError as error:
        raise ValueError("cl-bench checkpoint is unavailable") from error
    lines = data.splitlines(keepends=True)
    if lines and not lines[-1].endswith(b"\n"):
        lines.pop()
    if not lines:
        raise ValueError("cl-bench checkpoint has no complete header")

    records: list[Any] = []
    for line in lines:
        try:
            records.append(json.loads(line.decode("utf-8").strip()))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ValueError("cl-bench checkpoint contains an invalid complete record") from error

    header = records[0]
    if (
        not isinstance(header, dict)
        or set(header) != {"type", "version", "expected"}
        or header.get("type") != "header"
        or header.get("version") != 1
        or isinstance(header.get("version"), bool)
    ):
        raise ValueError("cl-bench checkpoint header is invalid")
    expected = header.get("expected")
    if (
        not isinstance(expected, list)
        or not all(isinstance(name, str) and name for name in expected)
        or len(expected) != len(set(expected))
    ):
        raise ValueError("cl-bench checkpoint expected IDs are invalid")

    benchmarks: dict[str, dict[str, Any]] = {}
    for record in records[1:]:
        if (
            not isinstance(record, dict)
            or set(record) != {"type", "name", "benchmark"}
            or record.get("type") != "row"
        ):
            raise ValueError("cl-bench checkpoint row record is invalid")
        name = record.get("name")
        if not isinstance(name, str) or name not in expected:
            raise ValueError("cl-bench checkpoint contains an unexpected benchmark")
        if name in benchmarks:
            raise ValueError("cl-bench checkpoint contains a duplicate benchmark")
        benchmark, _ = _validate_cl_bench_benchmark(name, record.get("benchmark"))
        benchmarks[name] = benchmark

    return (
        {"status": "partial", "times": [], "benchmarks": benchmarks},
        {
            "complete": set(benchmarks) == set(expected),
            "expected": list(expected),
            "completed": len(benchmarks),
        },
    )


def _stderr_tail(stderr: str) -> tuple[str, bool]:
    """Redact machine-specific details and keep only a bounded stderr tail."""
    hostname = socket.gethostname()
    if hostname:
        stderr = re.sub(
            rf"(?<![\w.-]){re.escape(hostname)}(?![\w.-])",
            "<hostname>",
            stderr,
            flags=re.IGNORECASE,
        )
    for pattern in _QUOTED_PATH_PATTERNS:
        stderr = pattern.sub(
            lambda match: match.group(0)[0] + "<absolute-path>" + match.group(0)[0],
            stderr,
        )
    stderr = _FILE_URI_PATTERN.sub("<absolute-path>", stderr)
    stderr = _UNQUOTED_ABSOLUTE_PATH_LINE_PATTERN.sub("<absolute-path>", stderr)
    truncated = len(stderr) > MAX_STDERR_TAIL_CHARS
    return stderr[-MAX_STDERR_TAIL_CHARS:], truncated


def _test_map(rows: Any) -> dict[str, Mapping[str, Any]]:
    if not isinstance(rows, list):
        return {}
    tests: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("name"), str):
            raise ValueError("ANSI report contains an invalid test row")
        name = row["name"]
        if name in tests:
            raise ValueError(f"ANSI report contains a duplicate test: {name}")
        tests[name] = row
    return tests


def _policy_deltas(
    policy: str,
    current: Mapping[str, Any],
    previous: Mapping[str, Any] | None,
) -> dict[str, Any]:
    unavailable = {"available": False, "reason": "previous scoreboard report was not available"}
    if previous is None:
        return unavailable
    previous_ansi = previous.get("ansi-test")
    if not isinstance(previous_ansi, dict) or not isinstance(previous_ansi.get("policies"), dict):
        return {
            "available": False,
            "reason": "previous scoreboard has no policy-specific ANSI reports (legacy artifact)",
        }
    previous_policy = previous_ansi["policies"].get(policy)
    if not isinstance(previous_policy, dict):
        return {"available": False, "reason": f"previous scoreboard has no {policy} result"}
    current_identity = current["identity"]
    previous_identity = previous_policy.get("identity")
    if not isinstance(previous_identity, dict):
        return {"available": False, "reason": "previous policy result is missing its run identity"}
    if previous_identity.get("policy") != policy:
        return {"available": False, "reason": "previous policy identity is incompatible"}
    for field, label in (
        ("platform", "platform"),
        ("source_commit", "ansi-test source"),
        ("baseline_runtime_version", "SBCL baseline runtime"),
    ):
        if previous_identity.get(field) != current_identity.get(field):
            return {"available": False, "reason": f"previous {label} is incompatible"}
    old_tests = _test_map(previous_policy.get("tests"))
    new_tests = _test_map(current.get("tests"))
    if not old_tests or not new_tests:
        return {"available": False, "reason": "previous or current report has no test-level results"}
    if old_tests.keys() != new_tests.keys():
        return {"available": False, "reason": "previous and current ANSI test sets differ"}
    new_pass = []
    new_fail = []
    for name in sorted(new_tests):
        old_row, new_row = old_tests[name], new_tests[name]
        old_status, new_status = old_row.get("status"), new_row.get("status")
        if old_status != "passed" and new_status == "passed":
            delta = {"name": name, "chapter": new_row.get("chapter"), "previous_status": old_status}
            new_pass.append(delta)
        elif old_status == "passed" and new_status != "passed":
            delta = {
                "name": name,
                "chapter": new_row.get("chapter"),
                "previous_status": old_status,
                "status": new_status,
            }
            for field in ("failure_class", "condition_type", "error_kind", "message", "reason"):
                if field in new_row:
                    delta[field] = new_row[field]
            new_fail.append(delta)
    return {
        "available": True,
        "previous_commit_sha": previous.get("run", {}).get("commit_sha"),
        "new_pass_count": len(new_pass),
        "new_pass": new_pass,
        "new_fail_count": len(new_fail),
        "new_fail": new_fail,
    }


def _ansi_scoreboard(
    reports: Mapping[str, Mapping[str, Any]],
    previous: Mapping[str, Any] | None,
) -> dict[str, Any]:
    if set(reports) != set(ANSI_POLICIES):
        raise ValueError(f"ANSI reports must include exactly: {', '.join(ANSI_POLICIES)}")
    shared_fields = ("measured_on", "platform", "source_commit", "runtime_version", "baseline_runtime_version")
    shared_identity: dict[str, Any] | None = None
    policies: dict[str, Any] = {}
    for policy in ANSI_POLICIES:
        report = reports[policy]
        if report.get("status") != "complete" or report.get("implementation") != "ncl":
            raise ValueError(f"{policy} ANSI report is incomplete or is not an NCL report")
        if report.get("policy") != policy:
            raise ValueError(f"{policy} ANSI report has a different policy identity")
        comparison = report.get("comparison")
        if not isinstance(comparison, dict) or not isinstance(comparison.get("t_ansi"), list):
            raise ValueError(f"{policy} ANSI report has no test comparison")
        if int(comparison.get("t_ansi_count", -1)) != len(comparison["t_ansi"]):
            raise ValueError(f"{policy} ANSI report missing-test count does not match its test rows")
        if report.get("baseline_runtime_version") != SBCL_BASELINE_VERSION:
            raise ValueError(f"{policy} ANSI report does not use {SBCL_BASELINE_VERSION}")
        identity = {field: report.get(field) for field in shared_fields}
        if not all(identity.get(field) for field in shared_fields):
            raise ValueError(f"{policy} ANSI report is missing run identity metadata")
        if shared_identity is None:
            shared_identity = identity
        elif identity != shared_identity:
            raise ValueError("policy reports differ in date, platform, source, or runtime version")
        tests = report.get("tests")
        if not isinstance(tests, list):
            raise ValueError(f"{policy} ANSI report has no per-test results")
        test_by_name = _test_map(tests)
        if len(test_by_name) != int(comparison.get("expected", -1)):
            raise ValueError(f"{policy} ANSI report test count does not match its comparison")
        sbcl_pass_ncl_fail = [dict(row) for row in comparison["t_ansi"]]
        policy_result: dict[str, Any] = {
            "status": "failed" if comparison.get("t_ansi_count", len(sbcl_pass_ncl_fail)) else "passed",
            "passed": int(report.get("passed", comparison.get("passed", 0))),
            "failed": int(report.get("failed", 0)),
            "error": int(report.get("error", 0)),
            "timeout": int(report.get("timeout", 0)),
            "harness_error": int(report.get("harness_error", 0)),
            "unexecuted": int(report.get("unexecuted", 0)),
            "failure_reasons": report.get("failure_summary", comparison.get("failure_summary", [])),
            "sbcl_pass_ncl_fail": sbcl_pass_ncl_fail,
            "tests": tests,
            "comparison": comparison,
            "identity": {**identity, "policy": policy},
        }
        policy_result["delta"] = _policy_deltas(policy, policy_result, previous)
        policies[policy] = policy_result
    if shared_identity is None:
        raise ValueError("no ANSI policy reports were supplied")
    return {
        "status": "failed" if any(item["status"] == "failed" for item in policies.values()) else "passed",
        "platform": shared_identity["platform"],
        "source_commit": shared_identity["source_commit"],
        "sbcl_version": shared_identity["baseline_runtime_version"],
        "policies": policies,
    }


def make_scoreboard(
    ansi: ProcessResult,
    ansi_counts: Mapping[str, int] | None,
    bench: ProcessResult,
    bench_times: Sequence[float] | None,
    ansi_reports: Mapping[str, Mapping[str, Any]] | None = None,
    bench_report: Mapping[str, Any] | None = None,
    bench_driver_error: str | None = None,
    bench_progress: Mapping[str, Any] | None = None,
    legacy_ansi_report: Mapping[str, Any] | None = None,
    previous_report: Mapping[str, Any] | None = None,
    commit_sha: str = "unknown",
    commit_date: str | None = None,
) -> dict[str, Any]:
    """Build a JSON-serializable scoreboard from ANSI policies and cl-bench."""
    ansi_data: dict[str, Any] = {"status": ansi.status}
    if ansi_reports is not None:
        ansi_data = _ansi_scoreboard(ansi_reports, previous_report)
    else:
        if legacy_ansi_report is not None and ansi.status == "passed" and legacy_ansi_report.get("status") == "unmeasured":
            ansi_data["status"] = "unmeasured"
            if legacy_ansi_report.get("reason"):
                ansi_data["reason"] = str(legacy_ansi_report["reason"])
        if ansi_counts is not None:
            ansi_data.update({key: int(ansi_counts[key]) for key in ("passed", "failed", "unexecuted")})
        if legacy_ansi_report is not None:
            for key in ("error", "timeout", "harness_error"):
                if key in legacy_ansi_report:
                    ansi_data[key] = int(legacy_ansi_report[key])
            if isinstance(legacy_ansi_report.get("tests"), list):
                ansi_data["tests"] = legacy_ansi_report["tests"]
            if isinstance(legacy_ansi_report.get("comparison"), dict):
                ansi_data["comparison"] = legacy_ansi_report["comparison"]
    bench_status = bench.status
    if bench_status == "passed" and (
        bench_driver_error is not None
        or (bench_report is not None and bench_report["status"] == "failed")
    ):
        bench_status = "failed"
    bench_data: dict[str, Any] = {"status": bench_status}
    if bench_report is not None:
        bench_data["driver_status"] = bench_report["status"]
        bench_data["benchmarks"] = {
            name: {
                "status": row["status"],
                "stage": row["stage"],
                "times": list(row["times"]),
                "condition": (
                    _stderr_tail(row["condition"])[0]
                    if row["condition"] is not None
                    else None
                ),
                "wall_time_seconds": row["wall_time_seconds"],
            }
            for name, row in bench_report["benchmarks"].items()
        }
    if bench_progress is not None:
        bench_data["progress"] = dict(bench_progress)
    if bench_driver_error is not None:
        bench_data["driver_error"], _ = _stderr_tail(bench_driver_error)
    if bench_times is not None:
        bench_data["samples"] = len(bench_times)
        if bench_times:
            bench_data["geometric_mean"] = geometric_mean(bench_times)
    bench_data["return_code"] = bench.returncode
    signal_number = None
    if bench.returncode is not None and bench.returncode < 0:
        signal_number = -bench.returncode
    elif (
        bench.returncode == 128 + signal.SIGSEGV
        and re.search(r"\bsegmentation fault\b", bench.stderr, re.IGNORECASE)
    ):
        signal_number = signal.SIGSEGV
    if signal_number is not None:
        bench_data["signal_number"] = signal_number
    bench_data["stderr_tail"], bench_data["stderr_truncated"] = _stderr_tail(bench.stderr)
    measured_on = date.today().isoformat()
    return {
        "schema_version": 3,
        "run": {
            "commit_sha": commit_sha,
            "commit_date": commit_date or measured_on,
            "measured_on": measured_on,
        },
        "measured_on": measured_on,
        "sources": SOURCES,
        "ansi-test": ansi_data,
        "cl-bench": bench_data,
    }


def render_markdown(scoreboard: Mapping[str, Any]) -> str:
    ansi = scoreboard["ansi-test"]
    bench = scoreboard["cl-bench"]
    lines = ["# Conformance scoreboard", "", "| suite | status | result |", "| --- | --- | --- |"]
    if isinstance(ansi.get("policies"), dict):
        counts = "; ".join(
            f"{policy}: {result['passed']} passed, {len(result['sbcl_pass_ncl_fail'])} failed"
            for policy, result in ansi["policies"].items()
        )
    elif ansi["status"] == "unmeasured":
        counts = str(ansi.get("reason", "no reason supplied"))
    elif "passed" in ansi:
        counts = f"{ansi['passed']} passed, {ansi['failed']} failed, {ansi['unexecuted']} unexecuted"
        details = []
        for key, label in (("error", "error"), ("timeout", "timeout"), ("harness_error", "harness error")):
            value = int(ansi.get(key, 0))
            if value:
                details.append(f"{value} {label}{'' if value == 1 else 's'}")
        if details:
            counts += f" ({', '.join(details)})"
    lines.append(f"| ansi-test | {ansi['status']} | {counts} |")
    mean = str(bench.get("geometric_mean", ""))
    lines.append(f"| cl-bench | {bench['status']} | geometric mean: {mean} |")
    lines.extend(["", "| suite | commit |", "| --- | --- |"])
    for name in ("ansi-test", "cl-bench"):
        lines.append(f"| {name} | `{scoreboard['sources'][name]['commit']}` |")
    for policy, policy_result in ansi.get("policies", {}).items():
        lines.extend(["", f"## ANSI tests: {policy}", "", "| Test | Chapter | Status | Reason |", "| --- | --- | --- | --- |"])
        test_rows = policy_result["sbcl_pass_ncl_fail"]
        if test_rows:
            for row in test_rows:
                reason = str(row.get("reason", row.get("message", ""))).replace("|", "\\|").replace("\n", " ")
                lines.append(f"| `{row['name']}` | {row.get('chapter', '')} | {row['status']} | {reason} |")
        else:
            lines.append("| none | - | passed | - |")
        delta = policy_result["delta"]
        if delta["available"]:
            lines.extend([
                "",
                f"Previous-run delta: {delta['new_pass_count']} new passes, {delta['new_fail_count']} new failures.",
            ])
        else:
            lines.extend(["", f"Previous-run delta unavailable: {delta['reason']}."])
    if not ansi.get("policies"):
        test_rows = [row for row in ansi.get("tests", []) if row["status"] != "passed"]
        if test_rows:
            lines.extend(["", "## ANSI test outcomes", "", "| Test | Status | Reason |", "| --- | --- | --- |"])
            for row in test_rows:
                reason = str(row.get("reason", "")).replace("|", "\\|").replace("\n", " ")
                lines.append(f"| `{row['name']}` | {row['status']} | {reason} |")
    return "\n".join(lines) + "\n"


def _command(value: str | None) -> list[str] | None:
    return shlex.split(value) if value else None


def _read_ansi_reports(specs: Sequence[str]) -> dict[str, Mapping[str, Any]]:
    reports: dict[str, Mapping[str, Any]] = {}
    for spec in specs:
        policy, separator, path_text = spec.partition("=")
        if not separator or policy not in ANSI_POLICIES or not path_text:
            raise ValueError("--ansi-report must use POLICY=PATH for a supported policy")
        if policy in reports:
            raise ValueError(f"duplicate ANSI report for policy {policy}")
        value = json.loads(Path(path_text).read_text(encoding="utf-8"))
        if not isinstance(value, dict):
            raise ValueError(f"ANSI report for {policy} must be a JSON object")
        reports[policy] = value
    if set(reports) != set(ANSI_POLICIES):
        raise ValueError(f"supply one --ansi-report for each policy: {', '.join(ANSI_POLICIES)}")
    return reports


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
    parser.add_argument("--ansi-command", help="command emitting {passed,failed,unexecuted} JSON")
    parser.add_argument(
        "--ansi-report", action="append", default=[], metavar="POLICY=PATH",
        help="read a completed NCL report for one optimization policy (repeat once per policy)",
    )
    parser.add_argument("--previous-report", type=Path, help="previous successful scoreboard JSON for per-test deltas")
    parser.add_argument("--commit-sha", default=os.environ.get("GITHUB_SHA", "unknown"))
    parser.add_argument("--commit-date", help="date of the measured commit in YYYY-MM-DD form")
    parser.add_argument(
        "--cl-bench-command",
        help="command emitting {status, times, benchmarks} JSON",
    )
    parser.add_argument("--timeout", type=float, default=DEFAULT_COMMAND_TIMEOUT)
    parser.add_argument("--source-timeout", type=float, default=DEFAULT_SOURCE_TIMEOUT)
    parser.add_argument("--connect-timeout", type=float, default=DEFAULT_CONNECT_TIMEOUT)
    parser.add_argument("--source-retries", type=int, default=DEFAULT_SOURCE_RETRIES)
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    parser.add_argument("--output", type=Path, help="write the report to this path")
    args = parser.parse_args(argv)
    ansi_command = _command(args.ansi_command)
    bench_command = _command(args.cl_bench_command)
    if (ansi_command is None) == (not args.ansi_report) or bench_command is None:
        parser.error("supply exactly one of --ansi-command and policy-specific --ansi-report values, plus --cl-bench-command")
    if not args.commit_sha.strip():
        parser.error("--commit-sha cannot be empty")
    try:
        commit_date = date.fromisoformat(args.commit_date).isoformat() if args.commit_date else date.today().isoformat()
    except ValueError:
        parser.error("--commit-date must be an ISO date")
    ansi_reports = None
    if args.ansi_report:
        try:
            ansi_reports = _read_ansi_reports(args.ansi_report)
        except (OSError, ValueError, TypeError, json.JSONDecodeError) as error:
            parser.exit(1, f"scoreboard: ANSI reports are invalid: {error}\n")
    previous_report = None
    if args.previous_report is not None:
        try:
            previous_report = json.loads(args.previous_report.read_text(encoding="utf-8"))
            if not isinstance(previous_report, dict):
                raise ValueError("previous scoreboard must be a JSON object")
        except (OSError, ValueError, json.JSONDecodeError) as error:
            parser.exit(1, f"scoreboard: previous report is invalid: {error}\n")
    try:
        if ansi_reports is None:
            ansi_source = checkout_source(
                "ansi-test",
                args.cache_dir,
                timeout=args.source_timeout,
                connect_timeout=args.connect_timeout,
                retries=args.source_retries,
            )
        else:
            ansi_source = None
        bench_source = checkout_source(
            "cl-bench",
            args.cache_dir,
            timeout=args.source_timeout,
            connect_timeout=args.connect_timeout,
            retries=args.source_retries,
        )
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"scoreboard: source acquisition failed: {error}\n")
    legacy_ansi_report = None
    if ansi_reports is not None:
        ansi = ProcessResult("passed", 0)
        ansi_counts = None
    else:
        assert ansi_command is not None and ansi_source is not None
        ansi = run_command(ansi_command, cwd=ansi_source, timeout=args.timeout)
        ansi_counts = None
        if ansi.stdout:
            legacy_ansi_report = _json_output(ansi)
            ansi_counts = {key: int(legacy_ansi_report[key]) for key in ("passed", "failed", "unexecuted")}
    bench_times = None
    bench_report = None
    bench_driver_error = None
    bench_progress = None
    with tempfile.TemporaryDirectory(prefix="conformance-cl-bench-checkpoint-") as temporary:
        checkpoint_path = Path(temporary) / "checkpoint.jsonl"
        bench_environment = os.environ.copy()
        bench_environment[CL_BENCH_CHECKPOINT_ENV] = str(checkpoint_path)
        bench = run_command(
            bench_command,
            cwd=bench_source,
            timeout=args.timeout,
            env=bench_environment,
        )
        if bench.status == "timeout":
            try:
                bench_report, bench_progress = read_cl_bench_checkpoint(checkpoint_path)
            except ValueError as error:
                bench_driver_error = str(error)
            else:
                bench_driver_error = None
        elif bench.stdout:
            try:
                bench_report = validate_cl_bench_report(_json_output(bench))
            except ValueError as error:
                bench_driver_error = str(error)
            else:
                bench_times = bench_report["times"]
    if bench.status != "timeout" and not bench.stdout:
        bench_driver_error = "cl-bench runner stdout was empty"
    scoreboard = make_scoreboard(
        ansi,
        ansi_counts,
        bench,
        bench_times,
        ansi_reports=ansi_reports,
        bench_report=bench_report,
        bench_driver_error=bench_driver_error,
        bench_progress=bench_progress,
        legacy_ansi_report=legacy_ansi_report,
        previous_report=previous_report,
        commit_sha=args.commit_sha,
        commit_date=commit_date,
    )
    output = render_markdown(scoreboard) if args.format == "markdown" else json.dumps(scoreboard, indent=2, sort_keys=True)
    output = output if output.endswith("\n") else output + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")
    # A test failure is a measurement result, not a harness failure.  Only
    # process supervision or malformed runner output makes the harness fail.
    ansi_status = scoreboard["ansi-test"]["status"]
    complete_statuses = ("passed", "failed", "unmeasured")
    bench_status = scoreboard["cl-bench"]["status"]
    return 0 if (
        ansi_status in complete_statuses
        and bench_status in ("passed", "failed")
        and bench_driver_error is None
    ) else 1


if __name__ == "__main__":
    sys.exit(main())
