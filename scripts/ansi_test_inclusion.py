#!/usr/bin/env python3
"""Measure ansi-test results and compare NCL with a same-day SBCL baseline."""

from __future__ import annotations

import argparse
import csv
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


RESULT_STATUSES = {"passed", "failed", "timeout", "harness_error", "unexecuted"}
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


def _chapter_runner(chapter: str) -> str:
    return f'''(defvar *root-path*
  (make-pathname :name nil :type nil :version nil :defaults (truename "init.lsp")))
(let ((*default-pathname-defaults* *root-path*))
  (load "gclload1.lsp"))
(let ((*default-pathname-defaults* *root-path*))
  (load "{chapter}/load.lsp"))
(when *load-pathname*
  (mapc #'regression-test:rem-test '{REMOVED_TEST_FORMS}))
(format *standard-output* "{PROTOCOL}~CBEGIN~C{chapter}~%" #\\Tab #\\Tab)
(finish-output *standard-output*)
(regression-test:do-tests :out (make-broadcast-stream) :exit nil)
(dolist (name regression-test:*passed-tests*)
  (format *standard-output* "{PROTOCOL}~CPASS~C~A~%" #\\Tab #\\Tab (symbol-name name)))
(dolist (name regression-test:*failed-tests*)
  (format *standard-output* "{PROTOCOL}~CFAIL~C~A~%" #\\Tab #\\Tab (symbol-name name)))
(format *standard-output* "{PROTOCOL}~CDONE~C{chapter}~%" #\\Tab #\\Tab)
(finish-output *standard-output*)
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
  (format *standard-output* "{PROTOCOL}~CBEGIN~Cbaseline~%" #\\Tab #\\Tab)
  (finish-output *standard-output*)
    (regression-test:do-tests :out (make-broadcast-stream) :exit nil)
  (dolist (entry (cdr regression-test::*entries*))
    (let ((name (regression-test::name entry))
          (source (gethash (regression-test::name entry) sources)))
      (if (regression-test::has-disabled-note entry)
          (format *standard-output* "{PROTOCOL}~CDISABLED~C~A~%" #\\Tab #\\Tab (symbol-name name))
          (format *standard-output* "{PROTOCOL}~CENTRY~C~A~C~A~%"
                  #\\Tab #\\Tab (symbol-name name) #\\Tab (namestring (truename source))))))
  (dolist (name regression-test:*passed-tests*)
    (format *standard-output* "{PROTOCOL}~CPASS~C~A~%" #\\Tab #\\Tab (symbol-name name)))
  (dolist (name regression-test:*failed-tests*)
    (format *standard-output* "{PROTOCOL}~CFAIL~C~A~%" #\\Tab #\\Tab (symbol-name name)))
  (format *standard-output* "{PROTOCOL}~CDONE~Cbaseline~%" #\\Tab #\\Tab)
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


def _parse_records(stdout: str) -> tuple[dict[str, str], dict[str, str], set[str], bool]:
    results: dict[str, str] = {}
    sources: dict[str, str] = {}
    disabled: set[str] = set()
    done = False
    for line in stdout.splitlines():
        if not line.startswith(PROTOCOL + "\t"):
            continue
        parts = line.split("\t")
        if len(parts) == 3 and parts[1] == "DONE":
            done = True
        elif len(parts) == 3 and parts[1] in ("PASS", "FAIL"):
            name = parts[2]
            if name in results:
                raise ValueError(f"duplicate test result: {name}")
            results[name] = "passed" if parts[1] == "PASS" else "failed"
        elif len(parts) == 4 and parts[1] == "ENTRY":
            if parts[2] in sources:
                raise ValueError(f"duplicate test source: {parts[2]}")
            sources[parts[2]] = parts[3]
        elif len(parts) == 3 and parts[1] == "DISABLED":
            if parts[2] in disabled:
                raise ValueError(f"duplicate disabled test: {parts[2]}")
            disabled.add(parts[2])
    return results, sources, disabled, done


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


def measure_baseline(command: Sequence[str], source: Path, timeout: float) -> dict[str, Any]:
    process_status, stdout, stderr, returncode = _run_process(
        command, source, _baseline_runner(), timeout=timeout,
    )
    if process_status != "process" or returncode != 0:
        raise RuntimeError(_diagnostic(stderr, stdout) or process_status)
    results, sources, disabled, done = _parse_records(stdout)
    if not done:
        raise RuntimeError(_diagnostic(stderr, stdout) or "SBCL runner did not finish")
    if set(results) != set(sources) or set(results) & disabled or set(sources) & disabled:
        raise RuntimeError("SBCL results do not match active registered test names")
    tests = [
        {"name": name, "chapter": _chapter_for_source(sources[name], source), "status": status}
        for name, status in results.items()
    ]
    tests.sort(key=lambda row: (row["chapter"], row["name"]))
    report = _measurement(
        "sbcl", command, source, tests,
        len(sources) + len(disabled) + len(REMOVED_TEST_NAMES), stdout, stderr,
    )
    report["active"] = len(sources)
    report["disabled"] = len(disabled)
    report["disabled_tests"] = sorted(disabled)
    report["removed"] = len(REMOVED_TEST_NAMES)
    report["process_exit_status"] = returncode
    report["unexecuted"] = len(sources) - len(results)
    report["status"] = "complete"
    if report["registered"] != report["active"] + report["disabled"] + report["removed"]:
        raise RuntimeError("SBCL registration counts do not balance")
    return report


def _measurement(
    implementation: str,
    command: Sequence[str],
    source: Path,
    tests: list[dict[str, str]],
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
        "active": registered,
        "disabled": 0,
        "passed": counts["passed"],
        "failed": counts["failed"],
        "unexecuted": max(0, registered - len(tests)),
        "status": "complete",
        "tests": tests,
        "first_error": _diagnostic(stderr, stdout),
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


def measure_chapters(
    command: Sequence[str],
    source: Path,
    expected_chapters: Mapping[str, str],
    timeout: float,
    implementation: str,
) -> dict[str, Any]:
    loaders = sorted({chapter.split("/", 1)[0] for chapter in expected_chapters.values()})
    observed: dict[str, str] = {}
    chapter_records: list[dict[str, Any]] = []
    for loader in loaders:
        status, stdout, stderr, returncode = _run_process(
            command, source, _chapter_runner(loader), timeout=timeout,
        )
        parsed: dict[str, str] = {}
        done = False
        parse_error: str | None = None
        try:
            parsed, _, _, done = _parse_records(stdout)
        except ValueError as error:
            parse_error = str(error)
        names = [name for name, chapter in expected_chapters.items() if chapter.split("/", 1)[0] == loader]
        if status == "timeout":
            chapter_status = "timeout"
            error_text = f"chapter exceeded {timeout:g} seconds"
        elif status == "harness_error":
            chapter_status = "harness_error"
            error_text = _diagnostic(stderr, stdout) or "could not start the interpreter"
        elif returncode != 0 or not done or parse_error:
            chapter_status = "harness_error"
            error_text = parse_error or _diagnostic(stderr, stdout) or "chapter runner did not finish successfully"
        else:
            chapter_status = "complete"
            error_text = None
        for name in names:
            if chapter_status == "timeout":
                observed[name] = "timeout"
            elif chapter_status == "harness_error":
                observed[name] = "harness_error"
            else:
                observed[name] = parsed.get(name, "unexecuted")
        chapter_records.append({
            "chapter": loader,
            "status": chapter_status,
            "expected": len(names),
            "first_error": error_text,
        })
    tests = [
        {
            "name": name,
            "chapter": chapter,
            "status": observed.get(name, "unexecuted"),
        }
        for name, chapter in expected_chapters.items()
    ]
    tests.sort(key=lambda row: (row["chapter"], row["name"]))
    counts = Counter(test["status"] for test in tests)
    result = {
        "schema_version": 1,
        "suite": "ansi-test",
        "implementation": implementation,
        "runtime_version": _runtime_version(command),
        "platform": _platform_name(),
        "measured_on": date.today().isoformat(),
        "source_commit": SOURCES["ansi-test"]["commit"],
        "registered": len(tests),
        "active": len(tests),
        "disabled": 0,
        "passed": counts["passed"],
        "failed": counts["failed"],
        "unexecuted": counts["unexecuted"],
        "timeout": counts["timeout"],
        "harness_error": counts["harness_error"],
        "status": "complete" if all(row["status"] == "complete" for row in chapter_records) else "partial",
        "chapters": chapter_records,
        "tests": tests,
        "first_error": next((row["first_error"] for row in chapter_records if row["first_error"]), None),
    }
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


def compare_results(baseline: Mapping[str, str], observed: Mapping[str, str]) -> dict[str, Any]:
    rows = []
    for name, chapter in sorted(baseline.items(), key=lambda item: (item[1], item[0])):
        status = observed.get(name, "unexecuted")
        if status not in RESULT_STATUSES:
            raise ValueError(f"invalid test result status: {status}")
        rows.append({"name": name, "chapter": chapter, "status": status})
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
    if report["status"] == "partial":
        lines.extend([
            "",
            "Partial means one or more chapter loaders did not finish. Tests under those loaders are listed as `harness_error` or `timeout`.",
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
        "| Chapter | Failed | Timeout | Harness error | Unexecuted |",
        "| --- | ---: | ---: | ---: | ---: |",
    ])
    if comparison["by_chapter"]:
        for chapter, counts in comparison["by_chapter"].items():
            lines.append(
                f"| {chapter} | {counts.get('failed', 0)} | {counts.get('timeout', 0)} | "
                f"{counts.get('harness_error', 0)} | {counts.get('unexecuted', 0)} |"
            )
    else:
        lines.append("| none | 0 | 0 | 0 | 0 |")
    lines.extend(["", "## T-ANSI tests", "", "| Test | Chapter | Status |", "| --- | --- | --- |"])
    if comparison["t_ansi"]:
        for row in comparison["t_ansi"]:
            lines.append(f"| `{row['name']}` | {row['chapter']} | {row['status']} |")
    else:
        lines.append("| none | - | - |")
    if report.get("first_error"):
        lines.extend(["", f"First harness error: {report['first_error']}"])
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
        if action == "baseline":
            sub.add_argument("--output-prefix", type=Path)
            sub.add_argument("--report", type=Path, help="write the complete SBCL result JSON")
        if action == "compare":
            sub.add_argument("--baseline", type=Path, required=True, help="SBCL pass-name .txt file")
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
            report = measure_baseline(command, source, args.timeout)
            prefix = args.output_prefix or Path(
                f"conformance/baselines/ansi-test-sbcl-2.6.0-{_platform_slug(report['platform'])}"
            )
            _write_baseline_files(report, prefix)
            if args.report:
                _write_output(report, args.report, "json")
            else:
                print(json.dumps({key: report[key] for key in (
                    "measured_on", "platform", "runtime_version", "registered", "active", "disabled",
                    "removed", "process_exit_status", "passed", "failed", "unexecuted",
                )}, indent=2, sort_keys=True))
            return 0
        expected = load_baseline(args.baseline)
        validate_same_day_baseline(args.baseline, {
            "measured_on": date.today().isoformat(),
            "platform": _platform_name(),
        })
        report = measure_chapters(command, source, expected, args.timeout, "ncl")
        observed = {test["name"]: test["status"] for test in report["tests"]}
        report["comparison"] = compare_results(expected, observed)
        _write_output(report, args.output, args.format)
        return comparison_exit_status(report["comparison"])
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"ansi-test scoreboard: {error}\n")


if __name__ == "__main__":
    sys.exit(main())
