#!/usr/bin/env python3
"""Compare deterministic Lisp programs under NCL policies and pinned SBCL."""

from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable, Sequence

from v3_programs import Form, ast_size, generate_expression, render_program, render_form, structural_replacements


POLICIES = ("off", "default", "speed3")
SBCL_VERSION = "2.6.10"
MAX_MINIMIZE_ATTEMPTS = 96
MAX_DIAGNOSTIC_CHARS = 4000


@dataclass(frozen=True)
class Execution:
    status: str
    output: str
    return_code: int | None
    stderr: str


@dataclass(frozen=True)
class Mismatch:
    seed: int
    policy: str
    source_path: str
    metadata_path: str
    minimize_attempts: int


def _positive_count(value: str) -> int:
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive integer") from error
    if count < 1:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return count


def _positive_timeout(value: str) -> float:
    try:
        timeout = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive number") from error
    if timeout <= 0:
        raise argparse.ArgumentTypeError("must be a positive number")
    return timeout


def _resolve_binary(value: str) -> str:
    candidate = Path(value).expanduser()
    if candidate.is_file():
        return str(candidate.resolve())
    binary = shutil.which(value)
    if binary is None:
        raise ValueError(f"executable not found: {value}")
    return binary


def _command(engine: str, binary: str, policy: str, program_path: str) -> list[str]:
    if engine == "sbcl":
        return [binary, "--noinform", "--disable-debugger", "--script", program_path]
    if engine != "ncl":
        raise ValueError(f"unsupported engine: {engine}")
    command = [binary]
    if policy == "off":
        command.append("--optimization=off")
    elif policy not in {"default", "speed3"}:
        raise ValueError(f"unsupported policy: {policy}")
    command.extend(("--script", program_path))
    return command


def _normalize_output(output: str) -> str:
    return output.replace("\r\n", "\n").rstrip()


def execute_program(
    engine: str,
    binary: str,
    policy: str,
    form: Form,
    timeout: float,
) -> Execution:
    source = render_program(form, policy)
    try:
        with tempfile.TemporaryDirectory(prefix="ncl-difftest-") as directory:
            program = Path(directory) / "program.lisp"
            program.write_text(source, encoding="utf-8")
            command = _command(engine, binary, policy, str(program))
            try:
                completed = subprocess.run(
                    command,
                    cwd=directory,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="replace",
                    timeout=timeout,
                    check=False,
                    env=os.environ.copy(),
                )
            except subprocess.TimeoutExpired as error:
                stdout = error.stdout or ""
                stderr = error.stderr or ""
                if isinstance(stdout, bytes):
                    stdout = stdout.decode("utf-8", errors="replace")
                if isinstance(stderr, bytes):
                    stderr = stderr.decode("utf-8", errors="replace")
                return Execution("timeout", _normalize_output(stdout), None, _safe_diagnostic(stderr, directory))
            except OSError as error:
                return Execution("launch_error", "", None, _safe_diagnostic(str(error), directory))
    except OSError as error:
        return Execution("harness_error", "", None, str(error)[:MAX_DIAGNOSTIC_CHARS])

    status = "passed" if completed.returncode == 0 else "failed"
    if completed.returncode < 0 or completed.returncode >= 128:
        status = "crash"
    return Execution(
        status,
        _normalize_output(completed.stdout),
        completed.returncode,
        _safe_diagnostic(completed.stderr, directory),
    )


def _safe_diagnostic(value: str, temporary_directory: str) -> str:
    redacted = value.replace(temporary_directory, "<temporary-directory>")
    redacted = re.sub(r"(?:/Users|/home|/private|/tmp)/[^\s:]+", "<path>", redacted)
    redacted = re.sub(r"[A-Za-z]:\\(?:[^\s\\]+\\)*[^\s\\]*", "<path>", redacted)
    return redacted[-MAX_DIAGNOSTIC_CHARS:]


def runtime_version(binary: str, engine: str) -> str:
    try:
        completed = subprocess.run(
            [binary, "--version"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=20,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise RuntimeError(f"{engine} version probe failed: {error}") from error
    if completed.returncode != 0:
        raise RuntimeError(f"{engine} version probe exited {completed.returncode}")
    output = (completed.stdout or completed.stderr).strip()
    if not output:
        raise RuntimeError(f"{engine} version probe produced no output")
    return output.splitlines()[0]


def _require_sbcl_version(value: str) -> None:
    if re.fullmatch(rf"SBCL {re.escape(SBCL_VERSION)}(?:\.roswell)?", value) is None:
        raise RuntimeError(f"SBCL {SBCL_VERSION} is required, got {value}")


def _is_mismatch(oracle: Execution, candidate: Execution) -> bool:
    if oracle.status != "passed":
        raise RuntimeError(
            f"SBCL oracle failed: status={oracle.status}, "
            f"exit={oracle.return_code}, stderr={oracle.stderr}"
        )
    return candidate.status != "passed" or candidate.output != oracle.output


def _pair(
    form: Form,
    policy: str,
    ncl: str,
    sbcl: str,
    timeout: float,
    execute: Callable[[str, str, str, Form, float], Execution],
) -> tuple[Execution, Execution]:
    oracle = execute("sbcl", sbcl, policy, form, timeout)
    if oracle.status != "passed":
        raise RuntimeError(
            f"SBCL failed for generated seed program under {policy}: "
            f"status={oracle.status}, exit={oracle.return_code}, stderr={oracle.stderr}"
        )
    candidate = execute("ncl", ncl, policy, form, timeout)
    if candidate.status in {"harness_error", "launch_error"}:
        raise RuntimeError(
            f"NCL runner failed under {policy}: status={candidate.status}, "
            f"stderr={candidate.stderr}"
        )
    return oracle, candidate


def minimize_expression(
    form: Form,
    policy: str,
    ncl: str,
    sbcl: str,
    timeout: float,
    *,
    execute: Callable[[str, str, str, Form, float], Execution] = execute_program,
    max_attempts: int = MAX_MINIMIZE_ATTEMPTS,
) -> tuple[Form, int]:
    current = form
    attempts = 0
    while attempts < max_attempts:
        reduced = False
        for candidate_form in structural_replacements(current):
            if attempts >= max_attempts:
                break
            attempts += 1
            oracle, candidate = _pair(candidate_form, policy, ncl, sbcl, timeout, execute)
            if _is_mismatch(oracle, candidate):
                current = candidate_form
                reduced = True
                break
        if not reduced:
            break
    return current, attempts


def _repro_paths(directory: Path, seed: int, policy: str) -> tuple[Path, Path]:
    stem = f"seed-{seed:010d}-{policy}"
    return directory / f"{stem}.lisp", directory / f"{stem}.json"


def _save_mismatch(
    directory: Path,
    seed: int,
    policy: str,
    form: Form,
    oracle: Execution,
    candidate: Execution,
    attempts: int,
) -> Mismatch:
    directory.mkdir(parents=True, exist_ok=True)
    source_path, metadata_path = _repro_paths(directory, seed, policy)
    source_path.write_text(render_program(form, policy), encoding="utf-8")
    record = {
        "schema_version": 1,
        "seed": seed,
        "policy": policy,
        "expression": render_form(form),
        "ast_nodes": ast_size(form),
        "oracle": asdict(oracle),
        "ncl": asdict(candidate),
        "minimize_attempts": attempts,
    }
    metadata_path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return Mismatch(seed, policy, str(source_path), str(metadata_path), attempts)


def run_suite(
    *,
    seed_count: int,
    seed_start: int,
    ncl: str,
    sbcl: str,
    repro_dir: Path,
    timeout: float,
    execute: Callable[[str, str, str, Form, float], Execution] = execute_program,
    version: Callable[[str, str], str] = runtime_version,
) -> dict[str, object]:
    ncl_binary = _resolve_binary(ncl)
    sbcl_binary = _resolve_binary(sbcl)
    sbcl_runtime = version(sbcl_binary, "sbcl")
    _require_sbcl_version(sbcl_runtime)
    ncl_runtime = version(ncl_binary, "ncl")

    mismatches: list[Mismatch] = []
    comparisons = 0
    for seed in range(seed_start, seed_start + seed_count):
        form = generate_expression(seed)
        for policy in POLICIES:
            oracle, candidate = _pair(form, policy, ncl_binary, sbcl_binary, timeout, execute)
            comparisons += 1
            if not _is_mismatch(oracle, candidate):
                continue
            minimized, attempts = minimize_expression(
                form,
                policy,
                ncl_binary,
                sbcl_binary,
                timeout,
                execute=execute,
            )
            final_oracle, final_candidate = _pair(
                minimized, policy, ncl_binary, sbcl_binary, timeout, execute,
            )
            mismatch = _save_mismatch(
                repro_dir,
                seed,
                policy,
                minimized,
                final_oracle,
                final_candidate,
                attempts,
            )
            mismatches.append(mismatch)

    return {
        "schema_version": 1,
        "status": "failed" if mismatches else "passed",
        "seed_start": seed_start,
        "seed_count": seed_count,
        "programs": seed_count,
        "comparisons": comparisons,
        "ncl_version": ncl_runtime,
        "sbcl_version": sbcl_runtime,
        "policies": list(POLICIES),
        "mismatches": [asdict(mismatch) for mismatch in mismatches],
        "repro_directory": str(repro_dir),
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed-count", type=_positive_count, required=True)
    parser.add_argument("--seed-start", type=int, default=0)
    parser.add_argument("--ncl", required=True, help="path to the NCL executable")
    parser.add_argument("--sbcl", required=True, help="path to SBCL 2.6.10")
    parser.add_argument("--repro-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=_positive_timeout, default=10.0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    try:
        report = run_suite(
            seed_count=args.seed_count,
            seed_start=args.seed_start,
            ncl=args.ncl,
            sbcl=args.sbcl,
            repro_dir=args.repro_dir,
            timeout=args.timeout,
        )
    except (OSError, RuntimeError, ValueError) as error:
        parser.exit(1, f"NCL differential test: {error}\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if report["mismatches"] else 0


if __name__ == "__main__":
    sys.exit(main())
