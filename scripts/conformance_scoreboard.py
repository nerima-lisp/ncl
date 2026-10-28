#!/usr/bin/env python3
"""Run the small, reproducible M2 conformance scoreboard.

The benchmark commands are deliberately supplied by the caller.  This keeps
the source acquisition deterministic while allowing the test suite to replace
the process runner without downloading or executing Lisp implementations.
"""

from __future__ import annotations

import argparse
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
from datetime import date
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence


DEFAULT_CONNECT_TIMEOUT = 120.0
DEFAULT_SOURCE_TIMEOUT = 300.0
DEFAULT_COMMAND_TIMEOUT = 900.0
DEFAULT_SOURCE_RETRIES = 2
MAX_STDERR_TAIL_CHARS = 4000

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
) -> ProcessResult:
    """Run a command and preserve enough output to diagnose its classification."""
    try:
        completed = runner(
            list(command), cwd=cwd, capture_output=True, text=True,
            timeout=timeout, check=False,
        )
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


def make_scoreboard(
    ansi: ProcessResult,
    ansi_counts: Mapping[str, int] | None,
    bench: ProcessResult,
    bench_times: Sequence[float] | None,
    ansi_report: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a JSON-serializable scoreboard from two process results."""
    ansi_data: dict[str, Any] = {"status": ansi.status}
    if (
        ansi.status == "passed"
        and ansi_report is not None
        and ansi_report.get("status") == "unmeasured"
    ):
        ansi_data["status"] = "unmeasured"
        if ansi_report.get("reason"):
            ansi_data["reason"] = str(ansi_report["reason"])
    if ansi_counts is not None:
        ansi_data.update({key: int(ansi_counts[key]) for key in ("passed", "failed", "unexecuted")})
    if ansi_report is not None:
        for key in ("error", "timeout", "harness_error"):
            if key in ansi_report:
                ansi_data[key] = int(ansi_report[key])
        if isinstance(ansi_report.get("tests"), list):
            ansi_data["tests"] = ansi_report["tests"]
        if isinstance(ansi_report.get("comparison"), dict):
            ansi_data["comparison"] = ansi_report["comparison"]
    bench_data: dict[str, Any] = {"status": bench.status}
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
    return {
        "schema_version": 1,
        "measured_on": (
            str(ansi_report.get("measured_on"))
            if ansi_report is not None and ansi_report.get("measured_on")
            else date.today().isoformat()
        ),
        "sources": SOURCES,
        "ansi-test": ansi_data,
        "cl-bench": bench_data,
    }


def render_markdown(scoreboard: Mapping[str, Any]) -> str:
    ansi = scoreboard["ansi-test"]
    bench = scoreboard["cl-bench"]
    lines = ["# Conformance scoreboard", "", "| suite | status | result |", "| --- | --- | --- |"]
    counts = ""
    if ansi["status"] == "unmeasured":
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
    test_rows = [row for row in ansi.get("tests", []) if row["status"] != "passed"]
    if test_rows:
        lines.extend(["", "## ANSI test outcomes", "", "| Test | Status | Reason |", "| --- | --- | --- |"])
        for row in test_rows:
            reason = str(row.get("reason", "")).replace("|", "\\|").replace("\n", " ")
            lines.append(f"| `{row['name']}` | {row['status']} | {reason} |")
    return "\n".join(lines) + "\n"


def _command(value: str | None) -> list[str] | None:
    return shlex.split(value) if value else None


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
    parser.add_argument("--ansi-command", help="command emitting {passed,failed,unexecuted} JSON")
    parser.add_argument("--ansi-report", type=Path, help="read a completed ANSI report JSON directly")
    parser.add_argument("--cl-bench-command", help="command emitting {times: [...]} JSON")
    parser.add_argument("--timeout", type=float, default=DEFAULT_COMMAND_TIMEOUT)
    parser.add_argument("--source-timeout", type=float, default=DEFAULT_SOURCE_TIMEOUT)
    parser.add_argument("--connect-timeout", type=float, default=DEFAULT_CONNECT_TIMEOUT)
    parser.add_argument("--source-retries", type=int, default=DEFAULT_SOURCE_RETRIES)
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    parser.add_argument("--output", type=Path, help="write the report to this path")
    args = parser.parse_args(argv)
    ansi_command = _command(args.ansi_command)
    bench_command = _command(args.cl_bench_command)
    if (ansi_command is None) == (args.ansi_report is None) or bench_command is None:
        parser.error("supply exactly one of --ansi-command and --ansi-report, plus --cl-bench-command")
    try:
        if args.ansi_report is None:
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
    ansi_report = None
    if args.ansi_report is None:
        assert ansi_command is not None and ansi_source is not None
        ansi = run_command(ansi_command, cwd=ansi_source, timeout=args.timeout)
    else:
        try:
            ansi_report = _json_output(ProcessResult(
                "passed", 0, args.ansi_report.read_text(encoding="utf-8"),
            ))
            comparison = ansi_report.get("comparison")
            if ansi_report.get("status") != "complete" or not isinstance(comparison, dict):
                raise ValueError("ANSI report must contain a completed comparison")
            missing_count = int(comparison.get("t_ansi_count", -1))
            if missing_count < 0:
                raise ValueError("ANSI report comparison has an invalid missing-test count")
            ansi_status = "failed" if missing_count else "passed"
            ansi_returncode = 1 if ansi_status == "failed" else 0
            ansi = ProcessResult(
                ansi_status, ansi_returncode, json.dumps(ansi_report), "",
            )
        except (OSError, ValueError, TypeError) as error:
            parser.exit(1, f"scoreboard: ANSI report is invalid: {error}\n")
    bench = run_command(bench_command, cwd=bench_source, timeout=args.timeout)
    ansi_counts = None
    if args.ansi_report is not None:
        ansi_counts = {key: int(ansi_report[key]) for key in ("passed", "failed", "unexecuted")}
    bench_times = None
    if ansi.stdout and args.ansi_report is None:
        ansi_report = _json_output(ansi)
        ansi_counts = {key: int(ansi_report[key]) for key in ("passed", "failed", "unexecuted")}
    if bench.stdout:
        data = _json_output(bench)
        bench_times = [float(value) for value in data["times"]]
    scoreboard = make_scoreboard(ansi, ansi_counts, bench, bench_times, ansi_report)
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
    return 0 if ansi_status in complete_statuses and bench.status in ("passed", "failed") else 1


if __name__ == "__main__":
    sys.exit(main())
