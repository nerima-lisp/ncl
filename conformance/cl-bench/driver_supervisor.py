#!/usr/bin/env python3
"""Isolate cl-bench source loads before running the selected suite once."""

from __future__ import annotations

import argparse
import ast
import json
import re
import signal
import subprocess
import sys
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


def _run_child(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)


def _failure_detail(process: subprocess.CompletedProcess[str]) -> str:
    detail = _without_stage_markers(process.stderr or process.stdout).strip()
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
    failures: dict[str, str] = {}
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

    registered = _registered_functions(cwd)
    requested_rows = selection or _comparison_benchmarks()
    selected_rows = _canonical_rows(requested_rows, registered)
    result = _run_benchmark_workers(
        engine=engine,
        runner=runner,
        mode=mode,
        driver=driver,
        selected_rows=selected_rows,
        registered=registered,
        failures=failures,
        cwd=cwd,
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
    return {
        "status": "failed",
        "stage": stage,
        "times": [],
        "condition": _without_stage_markers(_failure_detail(process)),
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
    cwd: Path,
) -> dict[str, Any]:
    benchmarks: dict[str, dict[str, Any]] = {}
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
                "status": "failed",
                "stage": "load",
                "times": [],
                "condition": failures[owner],
            }
            continue
        process = _run_child(
            _command(
                engine=engine,
                runner=runner,
                mode=mode,
                driver=driver,
                cwd=cwd,
                selection=(name,),
                skip_files=tuple(failures),
            ),
            cwd,
        )
        child_result = _json_result(process.stdout)
        child_rows = child_result.get("benchmarks", {}) if child_result else {}
        row = next(
            (value for key, value in child_rows.items() if key.casefold() == name.casefold()),
            None,
        )
        if row is None:
            if child_result is None and process.stdout:
                print(process.stdout[-4000:], end="", file=sys.stderr)
            row = _worker_failure_row(name, process)
            if process.returncode == 0:
                row["stage"] = "register"
                row["condition"] = "benchmark was not registered"
        elif process.returncode != 0 and row.get("status") in ("passed", "disabled"):
            row = _worker_failure_row(name, process)
        benchmarks[name] = {
            "status": row.get("status", "failed"),
            "stage": row.get("stage", "register"),
            "times": row.get("times", []),
            "condition": row.get("condition"),
        }
        if benchmarks[name]["status"] == "failed" and not benchmarks[name]["condition"]:
            benchmarks[name]["condition"] = "driver reported failure without a condition"
        diagnostic = _without_stage_markers(process.stderr)
        if diagnostic and (process.returncode != 0 or row.get("status") == "failed"):
            print(diagnostic, end="", file=sys.stderr)

    times = [
        sample
        for benchmark in benchmarks.values()
        if benchmark.get("status") == "passed"
        for sample in benchmark.get("times", [])
    ]
    failed = any(
        benchmark.get("status") == "failed" for benchmark in benchmarks.values()
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
