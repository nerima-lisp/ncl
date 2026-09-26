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
import shlex
import shutil
import subprocess
import sys
from datetime import date
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence


DEFAULT_CONNECT_TIMEOUT = 120.0
DEFAULT_SOURCE_TIMEOUT = 300.0
DEFAULT_COMMAND_TIMEOUT = 900.0
DEFAULT_SOURCE_RETRIES = 2


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


def make_scoreboard(
    ansi: ProcessResult,
    ansi_counts: Mapping[str, int] | None,
    bench: ProcessResult,
    bench_times: Sequence[float] | None,
) -> dict[str, Any]:
    """Build a JSON-serializable scoreboard from two process results."""
    ansi_data: dict[str, Any] = {"status": ansi.status}
    if ansi_counts is not None:
        ansi_data.update({key: int(ansi_counts[key]) for key in ("passed", "failed", "unexecuted")})
    bench_data: dict[str, Any] = {"status": bench.status}
    if bench_times is not None:
        bench_data["samples"] = len(bench_times)
        bench_data["geometric_mean"] = geometric_mean(bench_times)
    return {
        "schema_version": 1,
        "measured_on": date.today().isoformat(),
        "sources": SOURCES,
        "ansi-test": ansi_data,
        "cl-bench": bench_data,
    }


def render_markdown(scoreboard: Mapping[str, Any]) -> str:
    ansi = scoreboard["ansi-test"]
    bench = scoreboard["cl-bench"]
    lines = ["# Conformance scoreboard", "", "| suite | status | result |", "| --- | --- | --- |"]
    counts = ""
    if "passed" in ansi:
        counts = f"{ansi['passed']} passed, {ansi['failed']} failed, {ansi['unexecuted']} unexecuted"
    lines.append(f"| ansi-test | {ansi['status']} | {counts} |")
    mean = str(bench.get("geometric_mean", ""))
    lines.append(f"| cl-bench | {bench['status']} | geometric mean: {mean} |")
    lines.extend(["", "| suite | commit |", "| --- | --- |"])
    for name in ("ansi-test", "cl-bench"):
        lines.append(f"| {name} | `{scoreboard['sources'][name]['commit']}` |")
    return "\n".join(lines) + "\n"


def _command(value: str | None) -> list[str] | None:
    return shlex.split(value) if value else None


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
    parser.add_argument("--ansi-command", help="command emitting {passed,failed,unexecuted} JSON")
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
    if ansi_command is None or bench_command is None:
        parser.error("--ansi-command and --cl-bench-command are required")
    try:
        ansi_source = checkout_source(
            "ansi-test",
            args.cache_dir,
            timeout=args.source_timeout,
            connect_timeout=args.connect_timeout,
            retries=args.source_retries,
        )
        bench_source = checkout_source(
            "cl-bench",
            args.cache_dir,
            timeout=args.source_timeout,
            connect_timeout=args.connect_timeout,
            retries=args.source_retries,
        )
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"scoreboard: source acquisition failed: {error}\n")
    ansi = run_command(ansi_command, cwd=ansi_source, timeout=args.timeout)
    bench = run_command(bench_command, cwd=bench_source, timeout=args.timeout)
    ansi_counts = None
    bench_times = None
    if ansi.stdout:
        data = _json_output(ansi)
        ansi_counts = {key: int(data[key]) for key in ("passed", "failed", "unexecuted")}
    if bench.stdout:
        data = _json_output(bench)
        bench_times = [float(value) for value in data["times"]]
    scoreboard = make_scoreboard(ansi, ansi_counts, bench, bench_times)
    output = render_markdown(scoreboard) if args.format == "markdown" else json.dumps(scoreboard, indent=2, sort_keys=True)
    output = output if output.endswith("\n") else output + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8")
    else:
        print(output, end="")
    # A test failure is a measurement result, not a harness failure.  Only
    # process supervision or malformed runner output makes the harness fail.
    return 0 if ansi.status in ("passed", "failed") and bench.status in ("passed", "failed") else 1


if __name__ == "__main__":
    sys.exit(main())
