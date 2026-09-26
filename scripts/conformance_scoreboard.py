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
import shlex
import subprocess
import sys
from datetime import date
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence


SOURCES = {
    "ansi-test": {
        "url": "https://gitlab.common-lisp.net/ansi-test/ansi-test.git",
        "commit": "ca06bd919661af162c67407c9d994e881870bdb3",
    },
    "cl-bench": {
        "url": "https://gitlab.common-lisp.net/ansi-test/cl-bench.git",
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


def checkout_source(name: str, cache_dir: Path, *, timeout: float = 60.0) -> Path:
    """Clone a pinned source once, then verify the cached checkout's commit."""
    try:
        source = SOURCES[name]
    except KeyError as error:
        raise ValueError(f"unknown source: {name}") from error
    destination = cache_dir / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    commit = source["commit"]
    if destination.exists():
        cached = subprocess.run(
            ["git", "-C", str(destination), "rev-parse", "HEAD"],
            check=False, capture_output=True, text=True,
        ).stdout.strip()
        if cached == commit:
            return destination
    else:
        subprocess.run(
            ["git", "clone", source["url"], str(destination)],
            check=True, capture_output=True, text=True, timeout=timeout,
        )
    subprocess.run(
        ["git", "-C", str(destination), "fetch", "--quiet", "origin", commit],
        check=True, capture_output=True, text=True, timeout=timeout,
    )
    subprocess.run(
        ["git", "-C", str(destination), "checkout", "--quiet", "--detach", commit],
        check=True, capture_output=True, text=True, timeout=timeout,
    )
    actual = subprocess.run(
        ["git", "-C", str(destination), "rev-parse", "HEAD"],
        check=True, capture_output=True, text=True, timeout=timeout,
    ).stdout.strip()
    if actual != commit:
        raise RuntimeError(f"{name}: expected {commit}, got {actual}")
    return destination


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
    parser.add_argument("--timeout", type=float, default=300.0)
    parser.add_argument("--format", choices=("json", "markdown"), default="json")
    parser.add_argument("--output", type=Path, help="write the report to this path")
    args = parser.parse_args(argv)
    ansi_command = _command(args.ansi_command)
    bench_command = _command(args.cl_bench_command)
    if ansi_command is None or bench_command is None:
        parser.error("--ansi-command and --cl-bench-command are required")
    try:
        ansi_source = checkout_source("ansi-test", args.cache_dir, timeout=args.timeout)
        bench_source = checkout_source("cl-bench", args.cache_dir, timeout=args.timeout)
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
