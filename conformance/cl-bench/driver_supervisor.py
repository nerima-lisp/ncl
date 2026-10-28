#!/usr/bin/env python3
"""Isolate cl-bench source loads before running the selected suite once."""

from __future__ import annotations

import argparse
import ast
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
import math
import os
import re
import signal
import subprocess
import sys
import threading
from pathlib import Path
from typing import Any


SOURCE_FILES = (
    "arrays.lisp",
    "bignum.lisp",
    "boehm-gc.lisp",
    "clos.lisp",
    "clos-janderson.lisp",
    "crc40.lisp",
    "deflate.lisp",
    "gabriel.lisp",
    "hash.lisp",
    "math.lisp",
    "misc.lisp",
    "ratios.lisp",
    "richards.lisp",
)
PACKAGE_DECLARATION_HEAD = re.compile(r"\(\s*defpackage\b", re.IGNORECASE)

CLOS_JANDERSON_FUNCTIONS = {
    "run-defclass",
    "run-defmethod",
    "make-instances",
    "make-instances/simple",
    "methodcalls/simple",
    "methodcalls/simple+after",
    "methodcalls/complex",
}
CHECKPOINT_ENV = "NCL_CL_BENCH_CHECKPOINT"
# 13 source preflights and 65 benchmark workers fit within 78 * 20 = 1,560
# seconds, leaving 240 seconds of the workflow's 1,800-second budget for setup.
CHILD_TIMEOUT_SECONDS = 20.0
MAX_BENCHMARK_WORKERS = 4
# The pinned suite has two pairs that write shared files: compiler/load-fasl
# use the same FASL, and the FPRINT benchmarks both write /tmp/fprint.tst.
SERIALIZED_BENCHMARKS = frozenset({
    "compiler", "load-fasl", "fprint/pretty", "fprint/ugly",
})


def _lisp_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _lisp_list(values: tuple[str, ...] | list[str]) -> str:
    return "(list " + " ".join(_lisp_string(value) for value in values) + ")"


def _extract_lisp_form(source: str, start: int) -> str:
    depth = 0
    index = start
    in_string = False
    escaped = False
    line_comment = False
    block_comment_depth = 0
    while index < len(source):
        character = source[index]
        if line_comment:
            if character == "\n":
                line_comment = False
            index += 1
            continue
        if block_comment_depth:
            pair = source[index : index + 2]
            if pair == "#|":
                block_comment_depth += 1
                index += 2
            elif pair == "|#":
                block_comment_depth -= 1
                index += 2
            else:
                index += 1
            continue
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            index += 1
            continue
        if source.startswith("#|", index):
            block_comment_depth = 1
            index += 2
        elif character == ";":
            line_comment = True
            index += 1
        elif source.startswith("#\\", index):
            index += 2
            if index < len(source):
                if source[index] in "()\";":
                    index += 1
                else:
                    while index < len(source) and not source[index].isspace() and source[index] not in "()":
                        index += 1
        elif character == '"':
            in_string = True
            index += 1
        elif character == "(":
            depth += 1
            index += 1
        elif character == ")":
            depth -= 1
            index += 1
            if depth == 0:
                return source[start:index]
        else:
            index += 1
    raise ValueError("unterminated DEFPACKAGE form in cl-bench source")


def _skip_lisp_character(source: str, index: int) -> int:
    index += 2
    if index < len(source):
        if source[index] in "()\";":
            return index + 1
        while index < len(source) and not source[index].isspace() and source[index] not in "()":
            index += 1
    return index


def _package_forms_in_file(path: Path) -> tuple[str, ...]:
    source = path.read_text(encoding="utf-8")
    forms = []
    index = 0
    depth = 0
    in_string = False
    escaped = False
    line_comment = False
    block_comment_depth = 0
    while index < len(source):
        character = source[index]
        if line_comment:
            if character == "\n":
                line_comment = False
            index += 1
            continue
        if block_comment_depth:
            pair = source[index : index + 2]
            if pair == "#|":
                block_comment_depth += 1
                index += 2
            elif pair == "|#":
                block_comment_depth -= 1
                index += 2
            else:
                index += 1
            continue
        if in_string:
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                in_string = False
            index += 1
            continue
        if source.startswith("#|", index):
            block_comment_depth = 1
            index += 2
        elif character == ";":
            line_comment = True
            index += 1
        elif source.startswith("#\\", index):
            index = _skip_lisp_character(source, index)
        elif character == '"':
            in_string = True
            index += 1
        elif character == "(":
            if depth == 0 and PACKAGE_DECLARATION_HEAD.match(source, index):
                forms.append(_extract_lisp_form(source, index))
            depth += 1
            index += 1
        elif character == ")":
            depth -= 1
            index += 1
        else:
            index += 1
    return tuple(forms)


def _package_bootstrap_forms(cwd: Path) -> tuple[str, tuple[str, ...]]:
    support_forms = _package_forms_in_file(cwd / "package.lisp")
    if len(support_forms) != 1:
        raise ValueError("expected one DEFPACKAGE form in cl-bench/package.lisp")
    export_clause = re.search(r"\(:export\b", support_forms[0], re.IGNORECASE)
    if export_clause is None:
        raise ValueError("cl-bench/package.lisp DEFPACKAGE has no :EXPORT clause")
    support_package = (
        support_forms[0][: export_clause.end()]
        + " #:*misc-dir*"
        + support_forms[0][export_clause.end() :]
    )
    source_forms = tuple(
        form
        for source in SOURCE_FILES
        for form in _package_forms_in_file(cwd / "files" / source)
    )
    return support_package, source_forms


def _eval_form(
    *,
    mode: str,
    driver: Path,
    cwd: Path,
    selection: tuple[str, ...],
    skip_files: tuple[str, ...] = (),
    preflight_file: str | None = None,
) -> str:
    assignments = [
        f"common-lisp-user::*cl-bench-driver-mode* :{mode}",
        "common-lisp-user::*cl-bench-selection* " + _lisp_list(selection),
        "common-lisp-user::*cl-bench-source-files* " + _lisp_list(SOURCE_FILES),
        "common-lisp-user::*cl-bench-skip-files* " + _lisp_list(skip_files),
        "common-lisp-user::*cl-bench-preflight-file* "
        + (_lisp_string(preflight_file) if preflight_file else "nil"),
    ]
    variables = (
        "*cl-bench-driver-mode*",
        "*cl-bench-selection*",
        "*cl-bench-source-files*",
        "*cl-bench-skip-files*",
        "*cl-bench-preflight-file*",
    )
    declarations = " ".join(
        f"(defvar common-lisp-user::{name})" for name in variables
    )
    support_package, source_packages = _package_bootstrap_forms(cwd)
    package_bootstrap = (
        f"(eval '{support_package}) "
        + " ".join(f"(eval '{form})" for form in source_packages)
    )
    return (
        "(progn "
        + declarations
        + " (setf "
        + " ".join(assignments)
        + ") "
        + package_bootstrap
        + " (load "
        + _lisp_string(str(driver))
        + "))"
    )


def _command(
    *,
    engine: str,
    runner: str,
    mode: str,
    driver: Path,
    cwd: Path,
    selection: tuple[str, ...],
    skip_files: tuple[str, ...] = (),
    preflight_file: str | None = None,
) -> list[str]:
    form = _eval_form(
        mode="preflight" if preflight_file is not None else "run",
        driver=driver,
        cwd=cwd,
        selection=selection,
        skip_files=skip_files,
        preflight_file=preflight_file,
    )
    if engine == "ncl":
        return [runner, f"--optimization={mode}", "--eval", form]
    return [
        runner,
        "--noinform",
        "--disable-debugger",
        "--userinit",
        "/dev/null",
        "--eval",
        form,
        "--eval",
        "(sb-ext:exit)",
    ]


def _run_child(
    command: list[str], cwd: Path, timeout: float = CHILD_TIMEOUT_SECONDS,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment.pop(CHECKPOINT_ENV, None)
    try:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
            start_new_session=os.name == "posix",
        )
    except (OSError, ValueError) as error:
        return subprocess.CompletedProcess(command, 127, "", str(error))
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            process.kill()
        stdout, stderr = process.communicate()
        return subprocess.CompletedProcess(command, None, stdout, stderr)
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def _failure_detail(process: subprocess.CompletedProcess[str]) -> str:
    detail = _without_stage_markers(process.stderr or process.stdout).strip()
    if process.returncode is None:
        timeout = f"child exceeded its {CHILD_TIMEOUT_SECONDS:g} second timeout"
        return f"{timeout}: {detail[-4000:]}" if detail else timeout
    if detail:
        return detail[-4000:]
    if process.returncode < 0:
        signal_number = -process.returncode
        try:
            signal_name = signal.Signals(signal_number).name
        except ValueError:
            signal_name = f"signal {signal_number}"
        return f"driver child terminated by signal {signal_number} ({signal_name})"
    return f"driver child exited with status {process.returncode}"


def _json_result(stdout: str) -> dict[str, Any] | None:
    for line in reversed(stdout.splitlines()):
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and all(key in value for key in ("status", "times", "benchmarks")):
            return value
    return None


def _function_name(function: str) -> str:
    return function.rsplit(":", 1)[-1].lstrip(":").lower()


def _source_owner(function: str, cwd: Path) -> str | None:
    name = _function_name(function)
    if name in CLOS_JANDERSON_FUNCTIONS:
        return "clos-janderson.lisp"
    if name == "run-eql-fib":
        return "clos.lisp"
    definition = re.compile(
        r"\(def(?:un|method|generic|macro|setf-expander)\s+"
        + re.escape(name)
        + r"(?=[\s()])",
        re.IGNORECASE,
    )
    matches = [
        file
        for file in SOURCE_FILES
        if definition.search((cwd / "files" / file).read_text(encoding="utf-8"))
    ]
    return matches[0] if len(matches) == 1 else None


def run_supervisor(
    *,
    engine: str,
    runner: str,
    mode: str,
    driver: Path,
    selection: tuple[str, ...],
    cwd: Path,
) -> int:
    checkpoint_path = os.environ.get(CHECKPOINT_ENV)
    if checkpoint_path is None:
        return _run_supervisor(
            engine=engine,
            runner=runner,
            mode=mode,
            driver=driver,
            selection=selection,
            cwd=cwd,
            checkpoint=None,
        )
    with Path(checkpoint_path).open("x", encoding="utf-8") as checkpoint:
        return _run_supervisor(
            engine=engine,
            runner=runner,
            mode=mode,
            driver=driver,
            selection=selection,
            cwd=cwd,
            checkpoint=checkpoint,
        )


def _run_supervisor(
    *,
    engine: str,
    runner: str,
    mode: str,
    driver: Path,
    selection: tuple[str, ...],
    cwd: Path,
    checkpoint: Any,
) -> int:
    failures: dict[str, str] = {}
    timed_out_sources: set[str] = set()
    registered = _registered_functions(cwd)
    requested_rows = selection or _comparison_benchmarks()
    selected_rows = _canonical_rows(requested_rows, registered)
    if checkpoint is not None:
        _write_checkpoint_record(
            checkpoint,
            {"type": "header", "version": 1, "expected": list(selected_rows)},
        )
    for source in SOURCE_FILES:
        process = _run_child(
            _command(
                engine=engine,
                runner=runner,
                mode=mode,
                driver=driver,
                cwd=cwd,
                selection=selection,
                skip_files=tuple(failures),
                preflight_file=source,
            ),
            cwd,
        )
        if process.returncode != 0:
            failures[source] = _failure_detail(process)
            if process.returncode is None:
                timed_out_sources.add(source)

    result = _run_benchmark_workers(
        engine=engine,
        runner=runner,
        mode=mode,
        driver=driver,
        selected_rows=selected_rows,
        registered=registered,
        failures=failures,
        timed_out_sources=timed_out_sources,
        cwd=cwd,
        checkpoint=checkpoint,
    )
    result = {
        "status": result["status"],
        "times": result["times"],
        "benchmarks": {
            name: {
                key: row.get(key)
                for key in ("status", "stage", "times", "condition")
            }
            for name, row in result["benchmarks"].items()
        },
    }
    print(json.dumps(result, separators=(",", ":")))
    return 1 if result["status"] == "failed" else 0


def _write_checkpoint_record(checkpoint: Any, record: dict[str, Any]) -> None:
    checkpoint.write(json.dumps(record, separators=(",", ":")) + "\n")
    checkpoint.flush()


def _write_checkpoint_row(
    checkpoint: Any, name: str, benchmark: dict[str, Any]
) -> None:
    if set(benchmark) != {"status", "stage", "times", "condition"}:
        raise ValueError(f"invalid checkpoint row shape for {name}")
    status = benchmark["status"]
    stage = benchmark["stage"]
    times = benchmark["times"]
    condition = benchmark["condition"]
    if status not in ("passed", "failed", "disabled", "timeout"):
        raise ValueError(f"invalid checkpoint row status for {name}")
    if stage not in ("load", "register", "execute", "finish"):
        raise ValueError(f"invalid checkpoint row stage for {name}")
    if not isinstance(times, list):
        raise ValueError(f"invalid checkpoint row times for {name}")
    if status == "timeout" and times:
        raise ValueError(f"invalid timed out checkpoint row times for {name}")
    for sample in times:
        if isinstance(sample, bool) or not isinstance(sample, (int, float)):
            raise ValueError(f"invalid checkpoint row times for {name}")
        try:
            normalized = float(sample)
        except OverflowError as error:
            raise ValueError(f"invalid checkpoint row times for {name}") from error
        if not math.isfinite(normalized) or normalized < 0 or (
            status != "failed" and normalized == 0
        ):
            raise ValueError(f"invalid checkpoint row times for {name}")
    if condition is not None and not isinstance(condition, str):
        raise ValueError(f"invalid checkpoint row condition for {name}")
    if status == "passed" and (
        stage != "finish" or condition is not None or not times
    ):
        raise ValueError(f"invalid passed checkpoint row for {name}")
    if status in ("failed", "timeout") and not condition:
        raise ValueError(f"invalid {status} checkpoint row for {name}")
    if status == "disabled" and times:
        raise ValueError(f"invalid disabled checkpoint row for {name}")
    _write_checkpoint_record(
        checkpoint,
        {"type": "row", "name": name, "benchmark": benchmark},
    )


def _without_stage_markers(stderr: str) -> str:
    return "".join(
        line
        for line in stderr.splitlines(keepends=True)
        if not line.startswith("CL-BENCH-STAGE ")
    )


def _worker_failure_row(
    name: str, process: subprocess.CompletedProcess[str]
) -> dict[str, Any]:
    markers = re.findall(
        r"^CL-BENCH-STAGE (load|register|execute|finish) (.+)$",
        process.stderr,
        re.MULTILINE,
    )
    stage = markers[-1][0] if markers else "load"
    timed_out = process.returncode is None
    condition = _without_stage_markers(_failure_detail(process))
    return {
        "status": "timeout" if timed_out else "failed",
        "stage": stage,
        "times": [],
        "condition": condition,
    }


def _worker_exception_row(error: Exception) -> dict[str, Any]:
    detail = f"benchmark worker raised {type(error).__name__}"
    message = str(error).strip()
    if message:
        detail += f": {message}"
    return {
        "status": "failed",
        "stage": "load",
        "times": [],
        "condition": detail,
    }


def _run_benchmark_workers(
    *,
    engine: str,
    runner: str,
    mode: str,
    driver: Path,
    selected_rows: tuple[str, ...],
    registered: dict[str, tuple[str, ...]],
    failures: dict[str, str],
    timed_out_sources: set[str],
    cwd: Path,
    checkpoint: Any = None,
) -> dict[str, Any]:
    benchmarks: dict[str, dict[str, Any]] = {}
    commands: dict[str, list[str]] = {}
    for name in selected_rows:
        owner = next(
            (
                _source_owner(function, cwd)
                for function in registered.get(name, ())
                if _source_owner(function, cwd) in failures
            ),
            None,
        )
        if owner is not None:
            benchmarks[name] = {
                "status": "timeout" if owner in timed_out_sources else "failed",
                "stage": "load",
                "times": [],
                "condition": failures[owner],
            }
            if checkpoint is not None:
                _write_checkpoint_row(checkpoint, name, benchmarks[name])
            continue
        commands[name] = _command(
            engine=engine,
            runner=runner,
            mode=mode,
            driver=driver,
            cwd=cwd,
            selection=(name,),
            skip_files=tuple(failures),
        )

    def run_benchmark(name: str) -> tuple[str, dict[str, Any], subprocess.CompletedProcess[str]]:
        if name.casefold() in SERIALIZED_BENCHMARKS:
            with serialized_benchmark_lock:
                process = _run_child(commands[name], cwd)
        else:
            process = _run_child(commands[name], cwd)
        child_result = _json_result(process.stdout)
        child_rows = child_result.get("benchmarks", {}) if child_result else {}
        row = next(
            (value for key, value in child_rows.items() if key.casefold() == name.casefold()),
            None,
        )
        if process.returncode is None:
            row = _worker_failure_row(name, process)
        elif row is None:
            row = _worker_failure_row(name, process)
            if process.returncode == 0:
                row["stage"] = "register"
                row["condition"] = "benchmark was not registered"
        elif process.returncode != 0 and row.get("status") in ("passed", "disabled"):
            row = _worker_failure_row(name, process)
        benchmark = {
            "status": row.get("status", "failed"),
            "stage": row.get("stage", "register"),
            "times": row.get("times", []),
            "condition": row.get("condition"),
        }
        if benchmark["status"] == "failed" and not benchmark["condition"]:
            benchmark["condition"] = "driver reported failure without a condition"
        return name, benchmark, process

    serialized_benchmark_lock = threading.Lock()
    completed_rows: dict[str, dict[str, Any]] = {}
    completed_processes: dict[str, subprocess.CompletedProcess[str]] = {}
    with ThreadPoolExecutor(max_workers=MAX_BENCHMARK_WORKERS) as executor:
        futures = {
            executor.submit(run_benchmark, name): name
            for name in commands
        }
        for future in as_completed(futures):
            name = futures[future]
            try:
                name, row, process = future.result()
            except Exception as error:
                row = _worker_exception_row(error)
                completed_rows[name] = row
                if checkpoint is not None:
                    _write_checkpoint_row(checkpoint, name, row)
                continue
            completed_rows[name] = row
            completed_processes[name] = process
            if checkpoint is not None:
                _write_checkpoint_row(checkpoint, name, row)

    for name in selected_rows:
        process = completed_processes.get(name)
        if process is None:
            continue
        if _json_result(process.stdout) is None and process.stdout:
            print(process.stdout[-4000:], end="", file=sys.stderr)
        diagnostic = _without_stage_markers(process.stderr)
        if diagnostic and (
            process.returncode != 0
            or completed_rows[name]["status"] in ("failed", "timeout")
        ):
            print(diagnostic, end="", file=sys.stderr)

    benchmarks = {
        name: benchmarks[name] if name in benchmarks else completed_rows[name]
        for name in selected_rows
    }

    times = [
        sample
        for benchmark in benchmarks.values()
        if benchmark.get("status") == "passed"
        for sample in benchmark.get("times", [])
    ]
    failed = any(
        benchmark.get("status") in ("failed", "timeout") for benchmark in benchmarks.values()
    )
    return {
        "status": "failed" if failed else "passed",
        "times": times,
        "benchmarks": benchmarks,
    }


def _registered_functions(cwd: Path) -> dict[str, tuple[str, ...]]:
    text = (cwd / "tests.lisp").read_text(encoding="utf-8")
    pattern = re.compile(
        r"^\(defbench\s+([^\s()]+)(.*?)(?=^\(defbench\s|^;; EOF|\Z)",
        re.MULTILINE | re.DOTALL,
    )
    values: dict[str, list[str]] = {}
    for symbol, body in pattern.findall(text):
        short = re.search(r':short\s+"([^"]+)"', body)
        row = short.group(1) if short else symbol.upper()
        function = re.search(
            r":function\s+'(?:[A-Za-z0-9_.-]+:){1,2}([A-Za-z0-9+*/<>=!?-]+)",
            body,
            re.IGNORECASE,
        )
        if function:
            values.setdefault(row, []).append(function.group(1))
    return {name: tuple(functions) for name, functions in values.items()}


def _comparison_benchmarks() -> tuple[str, ...]:
    root = Path(__file__).resolve().parents[2]
    source = root / "scripts" / "optimization_baseline.py"
    module = ast.parse(source.read_text(encoding="utf-8"), filename=str(source))
    for node in module.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "BENCHMARKS"
            for target in node.targets
        ):
            value = ast.literal_eval(node.value)
            if isinstance(value, tuple) and all(isinstance(item, str) for item in value):
                return value
            break
    raise ValueError(f"could not read comparison benchmark set from {source}")


def _canonical_rows(
    requested: tuple[str, ...], registered: dict[str, tuple[str, ...]]
) -> tuple[str, ...]:
    actual = {name.casefold(): name for name in registered}
    return tuple(actual.get(name.casefold(), name) for name in requested)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", choices=("ncl", "sbcl"), required=True)
    parser.add_argument("--runner", required=True)
    parser.add_argument("--mode", choices=("default", "off"), default="default")
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--cwd", type=Path, default=Path.cwd())
    parser.add_argument("benchmarks", nargs="*")
    args = parser.parse_args(argv)
    return run_supervisor(
        engine=args.engine,
        runner=args.runner,
        mode=args.mode,
        driver=args.driver.resolve(),
        selection=tuple(args.benchmarks),
        cwd=args.cwd.resolve(),
    )


if __name__ == "__main__":
    raise SystemExit(main())
