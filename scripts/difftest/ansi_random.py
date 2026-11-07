#!/usr/bin/env python3
"""Run the pinned ansi-test randomized testers with a reproducible NCL state."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Mapping
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

from conformance_scoreboard import SOURCES, checkout_source


PROTOCOL = "NCL-ANSI-RANDOM"
MAX_OUTPUT_CHARS = 12_000
RANDOM_STATE_SKIP_LIMIT = 1_000_000


def _positive_count(value: str) -> int:
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive integer") from error
    if count < 1:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return count


def _nonnegative_count(value: str) -> int:
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a nonnegative integer") from error
    if count < 0:
        raise argparse.ArgumentTypeError("must be a nonnegative integer")
    if count > RANDOM_STATE_SKIP_LIMIT:
        raise argparse.ArgumentTypeError(
            f"must not exceed {RANDOM_STATE_SKIP_LIMIT}"
        )
    return count


def _positive_timeout(value: str) -> float:
    try:
        timeout = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive number") from error
    if timeout <= 0:
        raise argparse.ArgumentTypeError("must be a positive number")
    return timeout


def _lisp_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _safe_diagnostic(value: str) -> str:
    redacted = re.sub(r"(?:/Users|/home|/private|/tmp)/[^\s:]+", "<path>", value)
    redacted = re.sub(r"[A-Za-z]:\\(?:[^\s\\]+\\)*[^\s\\]*", "<path>", redacted)
    return redacted[-MAX_OUTPUT_CHARS:]


def _source_root(cache_dir: Path) -> Path:
    return checkout_source("ansi-test", cache_dir)


def _check_source(source: Path) -> None:
    expected_commit = SOURCES["ansi-test"]["commit"]
    checked_out = subprocess.run(
        ["git", "-C", str(source), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if checked_out.returncode != 0 or checked_out.stdout.strip() != expected_commit:
        actual = checked_out.stdout.strip() or "unavailable"
        raise ValueError(f"ansi-test source must be pinned at {expected_commit}; got {actual}")
    required = (
        "init.lsp",
        "gclload1.lsp",
        "random/random-int-form.lsp",
        "random/random-type-prop.lsp",
        "random/random-type-prop-tests.lsp",
    )
    missing = [name for name in required if not (source / name).is_file()]
    if missing:
        raise ValueError(f"pinned ansi-test checkout is missing: {', '.join(missing)}")


def _runner_prelude(seed_start: int) -> str:
    return f'''(defparameter *ncl-ansi-random-root*
  (make-pathname :name nil :type nil :version nil :defaults (truename "init.lsp")))
(defparameter *ncl-ansi-random-directory*
  (merge-pathnames #P"random/" *ncl-ansi-random-root*))

(defun ncl-ansi-random-emit (kind &rest fields)
  (format *standard-output* "~%{PROTOCOL}|~A" kind)
  (dolist (field fields)
    (format *standard-output* "|~A" field))
  (terpri *standard-output*)
  (finish-output *standard-output*))

(defun ncl-ansi-random-clean (value)
  (with-output-to-string (stream)
    (loop for character across value
          do (write-char (if (member character '(#\\Tab #\\Newline #\\Return))
                             #\\Space
                             character)
                         stream))))

(defun ncl-ansi-random-load-file (name)
  (let ((loader (find-symbol "COMPILE-AND-LOAD" "CL-TEST")))
    (if (and loader (fboundp loader))
        (handler-case
            (funcall (symbol-function loader) name)
          (error (condition)
            (ncl-ansi-random-emit "FALLBACK" name
                                  (ncl-ansi-random-clean (princ-to-string condition)))
            (load name)))
        (progn
          (ncl-ansi-random-emit "FALLBACK" name "compile-and-load unavailable; loaded source")
          (load name)))))

(defun ncl-ansi-random-load-base ()
  (ncl-ansi-random-emit "PHASE" "gclload1:start")
  (let ((*default-pathname-defaults* *ncl-ansi-random-root*))
    (load "gclload1.lsp"))
  (ncl-ansi-random-emit "PHASE" "gclload1:complete")
  (ncl-ansi-random-emit "PHASE" "random-int-form:start")
  (let ((*default-pathname-defaults* *ncl-ansi-random-directory*))
    (ncl-ansi-random-load-file "random-int-form.lsp"))
  (ncl-ansi-random-emit "PHASE" "random-int-form:complete"))

(defun ncl-ansi-random-skip-state ()
  (loop repeat {seed_start} do (random 2147483647)))
'''


def _rtp_runner(seed_count: int, seed_start: int) -> str:
    return _runner_prelude(seed_start) + f'''(handler-case
    (progn
      (ncl-ansi-random-emit "PHASE" "runner-ready")
      (ncl-ansi-random-load-base)
      (ncl-ansi-random-emit "PHASE" "random-type-prop:start")
      (let ((*default-pathname-defaults* *ncl-ansi-random-directory*))
        (ncl-ansi-random-load-file "random-type-prop.lsp")
        (load "random-type-prop-tests.lsp"))
      (ncl-ansi-random-emit "PHASE" "random-type-prop:complete")
      (let* ((regression-package (find-package "REGRESSION-TEST"))
             (cl-test-package (find-package "CL-TEST"))
             (entries-symbol (find-symbol "*ENTRIES*" regression-package))
             (name-function (find-symbol "NAME" regression-package))
             (do-test-function (find-symbol "DO-TEST" regression-package))
             (entries (cdr (symbol-value entries-symbol)))
             (tests (loop for entry in entries
                          for name = (funcall (symbol-function name-function) entry)
                          when (search "RANDOM-TYPE-PROP." (symbol-name name)
                                       :test #'char-equal)
                            collect name))
             (attempted 0)
             (passed 0)
             (failed 0)
             (errors 0))
        (if (null tests)
            (ncl-ansi-random-emit "BLOCKED" "rtp" "registration"
                                  "no RANDOM-TYPE-PROP.* tests registered")
            (let ((*random-state* (make-random-state nil)))
              (ncl-ansi-random-skip-state)
              (set (find-symbol "*DEFAULT-REPS*" cl-test-package) {seed_count})
              (ncl-ansi-random-emit "PHASE" "rtp-tests:start")
              (dolist (name tests)
                (incf attempted)
                (handler-case
                    (if (funcall (symbol-function do-test-function)
                                 name :catch-errors nil)
                        (progn
                          (incf passed)
                          (ncl-ansi-random-emit "RTP" "PASS" (symbol-name name)))
                        (progn
                          (incf failed)
                          (ncl-ansi-random-emit "RTP" "FAIL" (symbol-name name)
                            (with-output-to-string (stream)
                              (let ((*print-pretty* nil) (*print-circle* t))
                                (prin1 (symbol-value
                                        (find-symbol "*RANDOM-TYPE-PROP-RESULT*"
                                                     cl-test-package))
                                       stream))))))
                  (error (condition)
                    (incf errors)
                    (ncl-ansi-random-emit "RTP" "ERROR" (symbol-name name)
                      (ncl-ansi-random-clean (princ-to-string condition))))))
              (ncl-ansi-random-emit "PHASE" "rtp-tests:complete")
              (ncl-ansi-random-emit "RTP-SUMMARY" attempted passed failed errors)))))
  (error (condition)
    (ncl-ansi-random-emit "BLOCKED" "rtp" "load-or-run"
      (ncl-ansi-random-clean (princ-to-string condition)))))
'''


def _loop_runner(seed_count: int, seed_start: int, size: int, nvars: int) -> str:
    # The upstream entry point is intentionally infinite; one worker batch is its bounded period.
    return _runner_prelude(seed_start) + f'''(handler-case
    (progn
      (ncl-ansi-random-emit "PHASE" "runner-ready")
      (ncl-ansi-random-load-base)
      (let* ((*random-state* (make-random-state nil))
             (cl-test-package (find-package "CL-TEST"))
             (test-symbol (find-symbol "TEST-RANDOM-INTEGER-FORMS" cl-test-package)))
        (ncl-ansi-random-skip-state)
        (ncl-ansi-random-emit "PHASE" "loop-random-int-forms:start")
        (let ((failures
                (funcall (symbol-function test-symbol) {size} {nvars} {seed_count}
                         :random-state *random-state*
                         :index (* {seed_start} {seed_count}))))
          (ncl-ansi-random-emit "PHASE" "loop-random-int-forms:complete")
          (ncl-ansi-random-emit "LOOP-SUMMARY" {seed_count}
          (if failures (length failures) 0)
          (with-output-to-string (stream)
            (let ((*print-pretty* nil) (*print-circle* t))
              (prin1 failures stream)))))))
  (error (condition)
    (ncl-ansi-random-emit "BLOCKED" "loop-random-int-forms" "load-or-run"
      (ncl-ansi-random-clean (princ-to-string condition)))))
'''


def _parse_output(group: str, stdout: str, stderr: str, returncode: int | None, timed_out: bool) -> dict[str, Any]:
    tests: list[dict[str, str]] = []
    fallbacks: list[dict[str, str]] = []
    phases: list[str] = []
    blocked: dict[str, str] | None = None
    summary: dict[str, int] | None = None
    loop_reproduction = ""
    for line in stdout.splitlines():
        if not line.startswith(PROTOCOL + "|"):
            continue
        fields = line.split("|")
        if len(fields) >= 5 and fields[1] == "RTP":
            tests.append({"status": fields[2].lower(), "name": fields[3], "detail": "|".join(fields[4:])})
        elif len(fields) == 4 and fields[1] == "RTP":
            tests.append({"status": fields[2].lower(), "name": fields[3], "detail": ""})
        elif len(fields) == 6 and fields[1] == "RTP-SUMMARY":
            summary = {
                "attempted": int(fields[2]),
                "passed": int(fields[3]),
                "failed": int(fields[4]),
                "errors": int(fields[5]),
            }
        elif len(fields) == 5 and fields[1] == "LOOP-SUMMARY":
            summary = {"attempted": int(fields[2]), "mismatches": int(fields[3])}
            loop_reproduction = "|".join(fields[4:])
        elif len(fields) >= 5 and fields[1] == "BLOCKED":
            blocked = {"phase": fields[3], "reason": "|".join(fields[4:])}
        elif len(fields) >= 4 and fields[1] == "FALLBACK":
            fallbacks.append({"file": fields[2], "reason": "|".join(fields[3:])})
        elif len(fields) == 3 and fields[1] == "PHASE":
            phases.append(fields[2])

    if timed_out:
        status = "timeout"
    elif returncode is None or returncode < 0 or returncode >= 128:
        status = "crash"
    elif returncode != 0:
        status = "failed"
    elif blocked is not None:
        status = "unsupported"
    elif group == "rtp" and summary is not None:
        status = "failed" if summary["failed"] or summary["errors"] else "passed"
    elif group == "loop-random-int-forms" and summary is not None:
        status = "failed" if summary["mismatches"] else "passed"
    else:
        status = "unsupported"

    if blocked is not None and phases:
        blocked["last_progress"] = phases[-1]

    result: dict[str, Any] = {
        "group": group,
        "status": status,
        "return_code": returncode,
        "timed_out": timed_out,
        "summary": summary,
        "tests": tests,
        "fallbacks": fallbacks,
        "phases": phases,
        "blocked": blocked,
        "stdout_tail": _safe_diagnostic(stdout),
        "stderr_tail": _safe_diagnostic(stderr),
    }
    if loop_reproduction:
        result["reproduction_data"] = loop_reproduction
    if status == "unsupported" and blocked is None:
        result["blocked"] = {
            "phase": "runner",
            "reason": "random tester did not emit its completion record",
        }
    return result


def _run_group(
    group: str,
    source: Path,
    ncl: str,
    runner: str,
    timeout: float,
) -> tuple[dict[str, Any], str]:
    with tempfile.TemporaryDirectory(prefix="ncl-ansi-random-") as directory:
        runner_path = Path(directory) / f"{group}.lisp"
        runner_path.write_text(runner, encoding="utf-8")
        try:
            completed = subprocess.run(
                [ncl, "--load", str(runner_path)],
                cwd=source,
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                timeout=timeout,
                check=False,
                env=os.environ.copy(),
            )
            timed_out = False
            stdout = completed.stdout
            stderr = completed.stderr
            returncode = completed.returncode
        except subprocess.TimeoutExpired as error:
            timed_out = True
            stdout = error.stdout or ""
            stderr = error.stderr or ""
            if isinstance(stdout, bytes):
                stdout = stdout.decode("utf-8", errors="replace")
            if isinstance(stderr, bytes):
                stderr = stderr.decode("utf-8", errors="replace")
            returncode = None
        except OSError as error:
            timed_out = False
            stdout = ""
            stderr = str(error)
            returncode = None
        return _parse_output(group, stdout, stderr, returncode, timed_out), runner


def _save_artifacts(
    repro_dir: Path,
    seed_start: int,
    seed_count: int,
    group: str,
    result: Mapping[str, Any],
    runner: str,
) -> list[str]:
    if result["status"] not in {"failed", "crash", "timeout"}:
        return []
    repro_dir.mkdir(parents=True, exist_ok=True)
    stem = f"{group}-seed-{seed_start:08d}-count-{seed_count:06d}"
    runner_path = repro_dir / f"{stem}.lisp"
    output_path = repro_dir / f"{stem}.json"
    runner_path.write_text(runner, encoding="utf-8")
    record = dict(result)
    record["runner"] = runner_path.name
    output_path.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return [runner_path.name, output_path.name]


def run_suite(
    *,
    ncl: str,
    seed_count: int,
    seed_start: int,
    cache_dir: Path,
    repro_dir: Path,
    timeout: float,
    source: Path | None = None,
    size: int = 32,
    nvars: int = 4,
) -> dict[str, Any]:
    binary = shutil.which(ncl) if not Path(ncl).is_file() else str(Path(ncl).resolve())
    if binary is None:
        raise ValueError(f"NCL executable not found: {ncl}")
    source_root = source.resolve() if source is not None else _source_root(cache_dir)
    _check_source(source_root)
    groups: list[dict[str, Any]] = []
    for group, runner in (
        ("rtp", _rtp_runner(seed_count, seed_start)),
        (
            "loop-random-int-forms",
            _loop_runner(seed_count, seed_start, size, nvars),
        ),
    ):
        result, exact_runner = _run_group(group, source_root, binary, runner, timeout)
        result["artifacts"] = _save_artifacts(
            repro_dir, seed_start, seed_count, group, result, exact_runner
        )
        groups.append(result)

    failed = any(group["status"] in {"failed", "crash", "timeout"} for group in groups)
    unsupported = any(group["status"] == "unsupported" for group in groups)
    status = "failed" if failed else "partial" if unsupported else "passed"
    report = {
        "schema_version": 1,
        "suite": "ansi-test-random",
        "source_commit": SOURCES["ansi-test"]["commit"],
        "ncl": Path(binary).name,
        "seed_count": seed_count,
        "seed_start": seed_start,
        "random_state": "MAKE-RANDOM-STATE NIL with deterministic seed-start stream skip",
        "status": status,
        "groups": groups,
    }
    repro_dir.mkdir(parents=True, exist_ok=True)
    report_path = repro_dir / "ansi-random-report.json"
    report["report_path"] = report_path.name
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ncl", required=True, help="NCL executable path")
    parser.add_argument("--seed-count", type=_positive_count, default=64)
    parser.add_argument("--seed-start", type=_nonnegative_count, default=0)
    parser.add_argument("--cache-dir", type=Path, default=Path(".cache/conformance"))
    parser.add_argument("--ansi-test-dir", type=Path, help="use an already pinned ansi-test checkout")
    parser.add_argument("--repro-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=_positive_timeout, default=900.0)
    parser.add_argument("--size", type=_positive_count, default=32)
    parser.add_argument("--nvars", type=_positive_count, default=4)
    args = parser.parse_args(argv)
    try:
        report = run_suite(
            ncl=args.ncl,
            seed_count=args.seed_count,
            seed_start=args.seed_start,
            cache_dir=args.cache_dir,
            repro_dir=args.repro_dir,
            timeout=args.timeout,
            source=args.ansi_test_dir,
            size=args.size,
            nvars=args.nvars,
        )
    except (OSError, subprocess.SubprocessError, ValueError, RuntimeError) as error:
        parser.exit(1, f"ansi-test random: {error}\n")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
