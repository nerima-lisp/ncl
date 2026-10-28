#!/usr/bin/env python3
"""Measure ansi-test results and compare NCL with a same-day SBCL baseline."""

from __future__ import annotations

import argparse
import csv
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from datetime import date
from pathlib import Path
from typing import Any, Mapping, Sequence

from conformance_scoreboard import SOURCES, checkout_source


RESULT_STATUSES = {"passed", "failed", "error", "timeout", "harness_error", "unexecuted"}
PROTOCOL = "ANSI-TEST-SCOREBOARD"
REMOVED_TEST_NAMES = ("LOAD-PATHNAME.1", "LOAD-TRUENAME.1")
REMOVED_TEST_FORMS = "(cl-test::load-pathname.1 cl-test::load-truename.1)"


def _text(value: str | bytes | None) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def _platform_name() -> str:
    system = "macOS" if platform.system() == "Darwin" else platform.system()
    return f"{system} {platform.machine()}"


def _platform_slug(value: str) -> str:
    return value.lower().replace(" ", "-")


def shard_for_test(name: str, shard_count: int) -> int:
    if shard_count < 1:
        raise ValueError("shard count must be a positive integer")
    digest = hashlib.sha256(name.encode("utf-8")).digest()
    return int.from_bytes(digest[:8], "big") % shard_count


def _validate_shard(shard_index: int | None, shard_count: int | None) -> None:
    if (shard_index is None) != (shard_count is None):
        raise ValueError("shard index and shard count must be supplied together")
    if shard_index is not None and shard_count is not None:
        if shard_count < 1 or not 0 <= shard_index < shard_count:
            raise ValueError("shard index must be within a positive shard count")


def _in_shard(name: str, shard_index: int | None, shard_count: int | None) -> bool:
    return shard_index is None or shard_for_test(name, shard_count) == shard_index


def _lisp_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _test_runner(test_name: str) -> str:
    test_symbol = _lisp_string(test_name)
    return f'''(defvar *root-path*
  (make-pathname :name nil :type nil :version nil :defaults (truename "init.lsp")))
(let ((*default-pathname-defaults* *root-path*))
  (load "gclload1.lsp"))
(let ((*default-pathname-defaults* *root-path*))
  (load "gclload2.lsp"))
(when *load-pathname*
  (mapc #'regression-test:rem-test '{REMOVED_TEST_FORMS}))
(let ((name (find-symbol {test_symbol} "CL-TEST")))
  (unless (and name (gethash name regression-test::*entries-table*))
    (error "No registered ANSI test named ~A" {test_symbol}))
  (format *standard-output* "{PROTOCOL}~CBEGIN~C~A~%" #\\Tab #\\Tab (symbol-name name))
  (finish-output *standard-output*)
  (handler-case
      (let ((passed
              (let ((*default-pathname-defaults* regression-test::*sandbox-path*))
                (regression-test:do-test name :catch-errors nil))))
        (format *standard-output* "{PROTOCOL}~C~A~C~A~%"
                #\\Tab (if passed "PASS" "FAIL") #\\Tab (symbol-name name)))
    (error (condition)
      (handler-case (format *error-output* "~A~%" condition) (error () nil))
      (format *standard-output* "{PROTOCOL}~CERROR~C~A~%"
              #\\Tab #\\Tab (symbol-name name))))
  (format *standard-output* "{PROTOCOL}~CDONE~C~A~%" #\\Tab #\\Tab (symbol-name name))
  (finish-output *standard-output*))
'''


def _baseline_runner() -> str:
    return f'''(defvar *root-path*
  (make-pathname :name nil :type nil :version nil :defaults (truename "init.lsp")))
(let ((*default-pathname-defaults* *root-path*))
  (load "gclload1.lsp"))
(let ((original (symbol-function 'regression-test::add-entry))
      (sources (make-hash-table :test (hash-table-test regression-test::*entries-table*))))
  (setf (symbol-function 'regression-test::add-entry)
        (lambda (&rest arguments)
          (let* ((entry (first arguments))
                 (name (regression-test::name entry))
                 (source *load-truename*))
            (prog1 (apply original arguments)
              (setf (gethash name sources) source)))))
    (let ((*default-pathname-defaults* *root-path*))
      (load "gclload2.lsp"))
  (when *load-pathname*
    (mapc #'regression-test:rem-test '{REMOVED_TEST_FORMS}))
  (format *standard-output* "{PROTOCOL}~CBEGIN~Cinventoried~%" #\\Tab #\\Tab)
  (finish-output *standard-output*)
  (dolist (entry (cdr regression-test::*entries*))
    (let ((name (regression-test::name entry))
          (source (gethash (regression-test::name entry) sources)))
      (if (regression-test::has-disabled-note entry)
          (format *standard-output* "{PROTOCOL}~CDISABLED~C~A~%" #\\Tab #\\Tab (symbol-name name))
          (format *standard-output* "{PROTOCOL}~CENTRY~C~A~C~A~%"
                  #\\Tab #\\Tab (symbol-name name) #\\Tab (namestring (truename source))))))
  (format *standard-output* "{PROTOCOL}~CDONE~Cinventoried~%" #\\Tab #\\Tab)
  (finish-output *standard-output*)
  (sb-ext:exit :code 0))
'''


def _diagnostic(stderr: str, stdout: str) -> str | None:
    fallback = None
    for output in (stderr, stdout):
        for line in output.splitlines():
            stripped = line.strip()
            if (
                not stripped
                or stripped.startswith(PROTOCOL)
                or stripped.upper().startswith(("WARNING:", "STYLE-WARNING:"))
            ):
                continue
            safe_line = re.sub(r"(?:/Users|/home|/private|/tmp)/\S+", "<path>", stripped)
            safe_line = re.sub(r"[A-Za-z]:\\(?:[^\s\\]+\\)*[^\s\\]*", "<path>", safe_line)
            if re.search(r"\b(unhandled|traceback|panic)\b", safe_line, re.IGNORECASE):
                return safe_line[:300]
            if fallback is None and not line.startswith((";", " ", "\t", "WARNING:")):
                fallback = safe_line[:300]
    return fallback


def _parse_records(stdout: str) -> dict[str, Any]:
    results: dict[str, str] = {}
    sources: dict[str, str] = {}
    disabled: set[str] = set()
    begun: set[str] = set()
    done: set[str] = set()
    for line in stdout.splitlines():
        if not line.startswith(PROTOCOL + "\t"):
            continue
        parts = line.split("\t")
        if len(parts) == 3 and parts[1] == "DONE":
            if parts[2] in done:
                raise ValueError(f"duplicate completion marker: {parts[2]}")
            done.add(parts[2])
        elif len(parts) == 3 and parts[1] == "BEGIN":
            if parts[2] in begun:
                raise ValueError(f"duplicate begin marker: {parts[2]}")
            begun.add(parts[2])
        elif len(parts) == 3 and parts[1] in ("PASS", "FAIL", "ERROR"):
            name = parts[2]
            if name in results:
                raise ValueError(f"duplicate test result: {name}")
            results[name] = {"PASS": "passed", "FAIL": "failed", "ERROR": "error"}[parts[1]]
        elif len(parts) == 4 and parts[1] == "ENTRY":
            if parts[2] in sources:
                raise ValueError(f"duplicate test source: {parts[2]}")
            sources[parts[2]] = parts[3]
        elif len(parts) == 3 and parts[1] == "DISABLED":
            if parts[2] in disabled:
                raise ValueError(f"duplicate disabled test: {parts[2]}")
            disabled.add(parts[2])
        else:
            raise ValueError(f"invalid ANSI test protocol record: {line[:160]}")
    return {"results": results, "sources": sources, "disabled": disabled, "begun": begun, "done": done}


def _run_process(
    command: Sequence[str],
    source: Path,
    runner_text: str,
    *,
    timeout: float,
) -> tuple[str, str, str, int | None]:
    _clean_suite_scratch(source)
    with tempfile.TemporaryDirectory(prefix="ansi-test-scoreboard-") as directory:
        runner_path = Path(directory) / "runner.lisp"
        runner_path.write_text(runner_text, encoding="utf-8")
        try:
            completed = subprocess.run(
                [*command, str(runner_path)],
                cwd=source,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=timeout,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            return "timeout", _text(error.stdout), _text(error.stderr), None
        except (OSError, ValueError) as error:
            return "harness_error", "", str(error), None
    return "process", completed.stdout, completed.stderr, completed.returncode


def _clean_suite_scratch(source: Path) -> None:
    # ENSURE-DIRECTORIES-EXIST.8 leaves its scratch directory behind between runs.
    scratch = source / "sandbox" / "scratch"
    if scratch.exists():
        shutil.rmtree(scratch)


def _chapter_for_source(source: str, root: Path) -> str:
    source_path = Path(source)
    try:
        relative = source_path.resolve().relative_to(root.resolve())
    except (OSError, ValueError) as error:
        raise ValueError("baseline source path is outside the pinned checkout") from error
    if len(relative.parts) < 2:
        raise ValueError("baseline test source has no chapter directory")
    return relative.parent.as_posix()


def _copy_suite_tree(source: Path, destination: Path) -> None:
    source = source.resolve()

    def ignore_artifacts(directory: str, names: list[str]) -> set[str]:
        ignored = {
            name for name in names
            if name == ".git" or name.lower().endswith(".fasl")
        }
        if Path(directory).resolve() == (source / "sandbox").resolve():
            ignored.add("scratch")
        return ignored

    shutil.copytree(source, destination, ignore=ignore_artifacts)


def _run_test_batch(
    command: Sequence[str],
    source: Path,
    tests: Sequence[tuple[str, str]],
    timeout: float,
) -> list[dict[str, Any]]:
    return [
        _run_test(command, source, name, chapter, timeout)
        for name, chapter in tests
    ]


def _run_test_rows(
    command: Sequence[str],
    source: Path,
    tests: Sequence[tuple[str, str]],
    timeout: float,
    jobs: int,
) -> list[dict[str, Any]]:
    if jobs < 1:
        raise ValueError("jobs must be a positive integer")
    if not tests:
        return []
    worker_count = min(jobs, len(tests))
    with tempfile.TemporaryDirectory(prefix="ansi-test-workers-") as directory:
        worker_sources = []
        batches = []
        for index in range(worker_count):
            worker_source = Path(directory) / f"worker-{index}"
            _copy_suite_tree(source, worker_source)
            worker_sources.append(worker_source)
            batches.append(tests[index::worker_count])
        with ThreadPoolExecutor(max_workers=worker_count) as executor:
            futures = [
                executor.submit(_run_test_batch, command, worker_source, batch, timeout)
                for worker_source, batch in zip(worker_sources, batches)
            ]
            rows = [row for future in futures for row in future.result()]
    rows.sort(key=lambda row: (row["chapter"], row["name"]))
    return rows


def measure_baseline(
    command: Sequence[str], source: Path, timeout: float, jobs: int = 1,
    shard_index: int | None = None, shard_count: int | None = None,
) -> dict[str, Any]:
    _validate_shard(shard_index, shard_count)
    with tempfile.TemporaryDirectory(prefix="ansi-test-inventory-") as directory:
        inventory_source = Path(directory) / "ansi-test"
        _copy_suite_tree(source, inventory_source)
        process_status, stdout, stderr, returncode = _run_process(
            command, inventory_source, _baseline_runner(), timeout=timeout,
        )
        if process_status != "process" or returncode != 0:
            raise RuntimeError(_diagnostic(stderr, stdout) or process_status)
        records = _parse_records(stdout)
        sources = records["sources"]
        disabled = records["disabled"]
        if records["begun"] != {"inventoried"} or records["done"] != {"inventoried"}:
            raise RuntimeError(_diagnostic(stderr, stdout) or "SBCL runner did not finish")
        if records["results"] or set(sources) & disabled:
            raise RuntimeError("SBCL inventory returned test results or duplicate active names")
        manifest = [
            {"name": name, "chapter": _chapter_for_source(filename, inventory_source)}
            for name, filename in sources.items()
        ]
        manifest.sort(key=lambda test: (test["chapter"], test["name"]))
        test_chapters = [
            (name, _chapter_for_source(filename, inventory_source))
            for name, filename in sources.items()
            if _in_shard(name, shard_index, shard_count)
        ]
    tests = _run_test_rows(command, source, test_chapters, timeout, jobs)
    tests.sort(key=lambda row: (row["chapter"], row["name"]))
    report = _measurement(
        "sbcl", command, source, tests,
        len(sources) + len(disabled) + len(REMOVED_TEST_NAMES), stdout, stderr,
    )
    report["active"] = len(tests)
    report["active_total"] = len(sources)
    report["disabled"] = len(disabled)
    report["disabled_tests"] = sorted(disabled)
    report["removed"] = len(REMOVED_TEST_NAMES)
    report["manifest"] = manifest
    report["process_exit_status"] = returncode
    report["unexecuted"] = sum(test["status"] == "unexecuted" for test in tests)
    report["status"] = "complete"
    if report["registered"] != report["active_total"] + report["disabled"] + report["removed"]:
        raise RuntimeError("SBCL registration counts do not balance")
    if shard_index is not None:
        report["shard_index"] = shard_index
        report["shard_count"] = shard_count
    return report


def _measurement(
    implementation: str,
    command: Sequence[str],
    source: Path,
    tests: list[dict[str, Any]],
    registered: int,
    stdout: str,
    stderr: str,
) -> dict[str, Any]:
    counts = Counter(test["status"] for test in tests)
    return {
        "schema_version": 1,
        "suite": "ansi-test",
        "implementation": implementation,
        "runtime_version": _runtime_version(command),
        "platform": _platform_name(),
        "measured_on": date.today().isoformat(),
        "source_commit": SOURCES["ansi-test"]["commit"],
        "registered": registered,
        "active": len(tests),
        "disabled": 0,
        "passed": counts["passed"],
        "failed": counts["failed"],
        "error": counts["error"],
        "timeout": counts["timeout"],
        "harness_error": counts["harness_error"],
        "unexecuted": max(0, registered - len(tests)),
        "status": "complete",
        "tests": tests,
        "first_error": next(
            (test.get("reason") for test in tests if test.get("reason")),
            _diagnostic(stderr, stdout),
        ),
    }


def _runtime_version(command: Sequence[str]) -> str:
    try:
        result = subprocess.run(
            [command[0], "--version"], capture_output=True, text=True,
            encoding="utf-8", errors="replace", timeout=5, check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return "unknown"
    first_line = next((line.strip() for line in (result.stdout + result.stderr).splitlines() if line.strip()), "")
    return first_line[:120] if result.returncode == 0 and first_line else "unknown"


def _run_test(
    command: Sequence[str],
    source: Path,
    name: str,
    chapter: str,
    timeout: float,
) -> dict[str, Any]:
    process_status, stdout, stderr, returncode = _run_process(
        command, source, _test_runner(name), timeout=timeout,
    )
    diagnostic = _diagnostic(stderr, stdout)
    status: str
    reason: str | None = None
    if process_status == "timeout":
        status, reason = "timeout", diagnostic or f"test exceeded {timeout:g} seconds"
    elif process_status == "harness_error":
        status, reason = "harness_error", diagnostic or "could not start the interpreter"
    else:
        try:
            records = _parse_records(stdout)
        except ValueError as error:
            records = {"results": {}, "begun": set(), "done": set()}
            parse_error = str(error)
        else:
            parse_error = None
        result = records["results"].get(name)
        began = name in records["begun"]
        completed = name in records["done"]
        if not began:
            if returncode not in (None, 0) or diagnostic:
                status = "error"
                reason = parse_error or diagnostic or f"interpreter exited with status {returncode} before test start"
            else:
                status, reason = "harness_error", parse_error or "test runner did not start the requested test"
        elif returncode != 0:
            status = "error"
            reason = diagnostic or f"interpreter exited with status {returncode}"
        elif parse_error:
            status, reason = "error", parse_error
        elif not completed or result is None:
            status, reason = "error", diagnostic or "test runner stopped before reporting an outcome"
        elif records["begun"] != {name} or records["done"] != {name} or set(records["results"]) != {name}:
            status, reason = "error", "test runner returned markers for an unexpected test"
        else:
            status = result
            if status == "error":
                reason = diagnostic or "ansi-test signaled an error"
            elif status == "failed":
                reason = diagnostic or "ansi-test reported a test failure"
    row: dict[str, Any] = {"name": name, "chapter": chapter, "status": status}
    if reason:
        row["reason"] = reason
    if returncode is not None:
        row["process_exit_status"] = returncode
    return row


def measure_tests(
    command: Sequence[str],
    source: Path,
    expected_tests: Mapping[str, str],
    timeout: float,
    implementation: str,
    selected_test: str | None = None,
    jobs: int = 1,
    shard_index: int | None = None,
    shard_count: int | None = None,
) -> dict[str, Any]:
    _validate_shard(shard_index, shard_count)
    if selected_test is not None and shard_index is not None:
        raise ValueError("a named test cannot be combined with shard selection")
    if selected_test is not None and selected_test not in expected_tests:
        raise ValueError(f"test is not in the SBCL passing baseline: {selected_test}")
    selected = {selected_test: expected_tests[selected_test]} if selected_test is not None else {
        name: chapter for name, chapter in expected_tests.items()
        if _in_shard(name, shard_index, shard_count)
    }
    tests = _run_test_rows(command, source, list(selected.items()), timeout, jobs)
    result = _measurement(
        implementation, command, source, tests, len(selected), "", "",
    )
    result["registered"] = len(selected)
    result["active"] = len(selected)
    result["unexecuted"] = sum(test["status"] == "unexecuted" for test in tests)
    result["status"] = "complete"
    if selected_test is not None:
        result["selected_test"] = selected_test
    if shard_index is not None:
        result["shard_index"] = shard_index
        result["shard_count"] = shard_count
    return result


def load_baseline(path: Path) -> dict[str, str]:
    names = [line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if names != sorted(set(names)):
        raise ValueError("baseline pass names must be sorted and unique")
    chapter_path = path.with_suffix(".tsv")
    with chapter_path.open(encoding="utf-8", newline="") as stream:
        rows = list(csv.reader(stream, delimiter="\t"))
    if not rows or rows[0] != ["name", "chapter"]:
        raise ValueError("baseline chapter map must start with name<TAB>chapter")
    chapters: dict[str, str] = {}
    for row in rows[1:]:
        if len(row) != 2 or not row[0] or not row[1] or row[0] in chapters:
            raise ValueError("baseline chapter map contains an invalid or duplicate row")
        chapters[row[0]] = row[1]
    if set(names) != set(chapters):
        raise ValueError("baseline pass names and chapter map differ")
    return chapters


def compare_results(
    baseline: Mapping[str, str],
    observed: Mapping[str, str | Mapping[str, Any]],
) -> dict[str, Any]:
    rows = []
    for name, chapter in sorted(baseline.items(), key=lambda item: (item[1], item[0])):
        result = observed.get(name, "unexecuted")
        if isinstance(result, Mapping):
            status = result.get("status", "unexecuted")
            reason = result.get("reason")
        else:
            status = result
            reason = None
        if status not in RESULT_STATUSES:
            raise ValueError(f"invalid test result status: {status}")
        row = {"name": name, "chapter": chapter, "status": status}
        if reason:
            row["reason"] = str(reason)
        rows.append(row)
    t_ansi = [row for row in rows if row["status"] != "passed"]
    by_chapter: dict[str, Counter[str]] = {}
    for row in t_ansi:
        by_chapter.setdefault(row["chapter"], Counter())[row["status"]] += 1
    return {
        "expected": len(rows),
        "passed": len(rows) - len(t_ansi),
        "t_ansi_count": len(t_ansi),
        "t_ansi": t_ansi,
        "by_chapter": {
            chapter: dict(sorted(counts.items()))
            for chapter, counts in sorted(by_chapter.items())
        },
    }


def comparison_exit_status(comparison: Mapping[str, Any]) -> int:
    return 1 if comparison["t_ansi_count"] else 0


def _metadata_paths(baseline: Path) -> tuple[Path, Path]:
    return baseline.with_suffix(".tsv"), baseline.with_suffix(".md")


def _read_metadata(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^- ([A-Za-z_ -]+): (.+)$", line)
        if match:
            key = re.sub(r"[ -]+", "_", match.group(1).lower())
            value = match.group(2).strip().strip("`")
            aliases = {
                "measured": "measured_on",
                "sbcl": "sbcl_version",
                "ansi_test_commit": "source_commit",
            }
            fields[aliases.get(key, key)] = value
    return fields


def validate_same_day_baseline(baseline_path: Path, measurement: Mapping[str, Any]) -> None:
    _, metadata_path = _metadata_paths(baseline_path)
    metadata = _read_metadata(metadata_path)
    if metadata.get("measured_on") != measurement["measured_on"]:
        raise ValueError("SBCL baseline and NCL measurement must be from the same day")
    if metadata.get("platform") != measurement["platform"]:
        raise ValueError("SBCL baseline and NCL measurement must use the same platform")
    if metadata.get("sbcl_version") != "2.6.0":
        raise ValueError("SBCL baseline metadata must pin version 2.6.0")
    if metadata.get("source_commit") != SOURCES["ansi-test"]["commit"]:
        raise ValueError("SBCL baseline uses a different ansi-test commit")


def render_markdown(report: Mapping[str, Any]) -> str:
    comparison = report.get("comparison")
    lines = ["# ansi-test inclusion scoreboard", ""]
    lines.extend([
        f"- Measured on: {report['measured_on']}",
        f"- Platform: {report['platform']}",
        f"- Implementation: {report['implementation']} ({report['runtime_version']})",
        f"- ansi-test commit: `{report['source_commit']}`",
        f"- Measurement status: {report['status']}",
    ])
    if comparison is None:
        lines.extend(["", "ANSI inclusion comparison: unmeasured because no same-day SBCL 2.6.0 baseline was supplied."])
        return "\n".join(lines) + "\n"
    lines.extend([
        "",
        f"- SBCL passing tests expected: {comparison['expected']}",
        f"- NCL passed: {comparison['passed']}",
        f"- T-ANSI missing: {comparison['t_ansi_count']}",
        "",
        "## T-ANSI by chapter",
        "",
        "| Chapter | Failed | Error | Timeout | Harness error | Unexecuted |",
        "| --- | ---: | ---: | ---: | ---: | ---: |",
    ])
    if comparison["by_chapter"]:
        for chapter, counts in comparison["by_chapter"].items():
            lines.append(
                f"| {chapter} | {counts.get('failed', 0)} | {counts.get('error', 0)} | "
                f"{counts.get('timeout', 0)} | {counts.get('harness_error', 0)} | "
                f"{counts.get('unexecuted', 0)} |"
            )
    else:
        lines.append("| none | 0 | 0 | 0 | 0 | 0 |")
    lines.extend(["", "## T-ANSI tests", "", "| Test | Chapter | Status | Reason |", "| --- | --- | --- | --- |"])
    if comparison["t_ansi"]:
        for row in comparison["t_ansi"]:
            reason = str(row.get("reason", "")).replace("|", "\\|").replace("\n", " ")
            lines.append(f"| `{row['name']}` | {row['chapter']} | {row['status']} | {reason} |")
    else:
        lines.append("| none | - | - | - |")
    if report.get("first_error"):
        lines.extend(["", f"First recorded error: {report['first_error']}"])
    return "\n".join(lines) + "\n"


def _write_output(report: Mapping[str, Any], output: Path | None, format_name: str) -> None:
    rendered = render_markdown(report) if format_name == "markdown" else json.dumps(report, indent=2, sort_keys=True)
    rendered += "" if rendered.endswith("\n") else "\n"
    if output is None:
        print(rendered, end="")
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8")


def _load_source(cache_dir: Path, timeout: float) -> Path:
    return checkout_source("ansi-test", cache_dir, timeout=timeout)


def _parse_command(value: str) -> list[str]:
    command = shlex.split(value)
    if not command:
        raise ValueError("interpreter command cannot be empty")
    return command


def _write_baseline_files(report: Mapping[str, Any], output_prefix: Path) -> None:
    if report["status"] != "complete" or report["unexecuted"]:
        raise ValueError("cannot write a baseline from an incomplete measurement")
    if report["runtime_version"] != "SBCL 2.6.0":
        raise ValueError("baseline generation requires SBCL 2.6.0")
    if report["active"] != len(report["tests"]):
        raise ValueError("baseline active test results do not match registration")
    passed = sorted(
        (test for test in report["tests"] if test["status"] == "passed"),
        key=lambda test: test["name"],
    )
    names = [test["name"] for test in passed]
    if len(names) != len(set(names)):
        raise ValueError("baseline contains duplicate passing test names")
    chapters = {test["name"]: test["chapter"] for test in passed}
    output_prefix.parent.mkdir(parents=True, exist_ok=True)
    output_prefix.with_name(output_prefix.name + ".txt").write_text(
        "".join(name + "\n" for name in names), encoding="utf-8",
    )
    chapter_path = output_prefix.with_name(output_prefix.name + ".tsv")
    with chapter_path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
        writer.writerow(["name", "chapter"])
        writer.writerows((name, chapters[name]) for name in names)
    metadata = [
        "# ANSI-Test SBCL baseline",
        "",
        f"- Measured: {report['measured_on']}",
        f"- Platform: {report['platform']}",
        f"- SBCL: 2.6.0",
        f"- ansi-test commit: `{report['source_commit']}`",
        f"- Registered tests: {report['registered']}",
        f"- Removed by the runner: {report['removed']} ({', '.join(REMOVED_TEST_NAMES)})",
        f"- SBCL process exit status: {report['process_exit_status']}",
        f"- Executed: {report['active']}",
        f"- Disabled by an ANSI-Test note: {report['disabled']}",
        f"- Passed: {report['passed']}",
        f"- Failed: {report['failed']}",
        f"- Error: {report['error']}",
        f"- Timed out: {report['timeout']}",
        f"- Harness errors: {report['harness_error']}",
        f"- Unexecuted active tests: {report['unexecuted']}",
    ]
    metadata.extend([
        "",
        "The runner removes these two tests because upstream `doit.lsp` says they misbehave when invoked from a file being loaded.",
    ])
    if report["disabled_tests"]:
        metadata.extend([
            "",
            "The note-disabled tests are "
            + ", ".join(f"`{name}`" for name in report["disabled_tests"])
            + ".",
            "The note assumes `OPEN` with `:if-exists :new-version` does not fail; ansi-test disables it for SBCL.",
        ])
    metadata.extend([
        "",
        "Passing test names are in the adjacent `.txt` file; the chapter map is in `.tsv`.",
        "",
    ])
    output_prefix.with_name(output_prefix.name + ".md").write_text(
        "\n".join(metadata), encoding="utf-8",
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    for action in ("baseline", "compare"):
        sub = subparsers.add_parser(action)
        sub.add_argument("--command", required=True, help="interpreter command prefix, for example 'sbcl --script'")
        sub.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
        sub.add_argument("--timeout", type=float, default=900.0, help="process timeout in seconds")
        sub.add_argument("--jobs", type=int, default=1, help="maximum concurrent per-test processes")
        sub.add_argument("--shard-index", type=int, help="zero-based deterministic shard index")
        sub.add_argument("--shard-count", type=int, help="total number of deterministic shards")
        if action == "baseline":
            sub.add_argument("--output-prefix", type=Path)
            sub.add_argument("--report", type=Path, help="write the complete SBCL result JSON")
        if action == "compare":
            sub.add_argument("--baseline", type=Path, required=True, help="SBCL pass-name .txt file")
            sub.add_argument("--test", help="run one named test from the SBCL passing baseline")
        if action != "baseline":
            sub.add_argument("--format", choices=("json", "markdown"), default="json")
            sub.add_argument("--output", type=Path)
    unmeasured = subparsers.add_parser("unmeasured")
    unmeasured.add_argument("--reason", required=True)
    unmeasured.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    if args.action == "unmeasured":
        report = {
            "schema_version": 1,
            "suite": "ansi-test",
            "implementation": "ncl",
            "platform": _platform_name(),
            "measured_on": date.today().isoformat(),
            "source_commit": SOURCES["ansi-test"]["commit"],
            "status": "unmeasured",
            "reason": args.reason,
            "passed": 0,
            "failed": 0,
            "unexecuted": 0,
            "tests": [],
        }
        _write_output(report, args.output, "json")
        return 0
    try:
        command = _parse_command(args.command)
        source = _load_source(args.cache_dir, args.timeout)
        if args.action == "baseline":
            version = _runtime_version(command)
            if version != "SBCL 2.6.0":
                raise ValueError(f"baseline generation requires SBCL 2.6.0, got {version}")
            report = measure_baseline(
                command, source, args.timeout, args.jobs,
                args.shard_index, args.shard_count,
            )
            prefix = args.output_prefix or Path(
                f"conformance/baselines/ansi-test-sbcl-2.6.0-{_platform_slug(report['platform'])}"
            )
            _write_baseline_files(report, prefix)
            if args.report:
                _write_output(report, args.report, "json")
            else:
                print(json.dumps({key: report[key] for key in (
                    "measured_on", "platform", "runtime_version", "registered", "active", "disabled",
                    "removed", "process_exit_status", "passed", "failed", "error", "timeout",
                    "harness_error", "unexecuted",
                )}, indent=2, sort_keys=True))
            return 0
        expected = load_baseline(args.baseline)
        validate_same_day_baseline(args.baseline, {
            "measured_on": date.today().isoformat(),
            "platform": _platform_name(),
        })
        report = measure_tests(
            command, source, expected, args.timeout, "ncl", getattr(args, "test", None), args.jobs,
            args.shard_index, args.shard_count,
        )
        observed = {test["name"]: test for test in report["tests"]}
        comparison_baseline = {name: expected[name] for name in observed}
        report["comparison"] = compare_results(comparison_baseline, observed)
        _write_output(report, args.output, args.format)
        return comparison_exit_status(report["comparison"])
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"ansi-test scoreboard: {error}\n")


if __name__ == "__main__":
    sys.exit(main())
