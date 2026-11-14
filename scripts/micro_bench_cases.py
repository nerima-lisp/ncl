"""Micro workload definitions and their Lisp measurement driver."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence

from benchmark_constants import REPO_ROOT


CASE_DIR = REPO_ROOT / "bench/micro/cases"
DEFAULT_RUNS = 5
DEFAULT_ITERATIONS = 10_000
PROCESS_TIMEOUT = 60.0
RESULT_PREFIX = "MICRO_RESULT "
ALLOCATION_COUNTERS = {
    "ncl": "NCL-GC:BYTES-CONSED-SINCE-GC snapshot delta, only if GC-COUNT is stable",
    "sbcl": "SB-EXT:GET-BYTES-CONSED snapshot delta",
}


@dataclass(frozen=True)
class MicroCase:
    name: str
    source: str
    iterations: int
    expected_result: Callable[[int], int]


MICRO_CASES = (
    MicroCase("float-loop", "float-loop.lisp", DEFAULT_ITERATIONS, lambda n: n * (n - 1) // 4),
    MicroCase("fixnum-loop", "fixnum-loop.lisp", DEFAULT_ITERATIONS, lambda n: n * (n - 1) // 2),
    MicroCase("clos-dispatch", "clos-dispatch.lisp", DEFAULT_ITERATIONS, lambda n: 2 * n),
    MicroCase("structure-access", "structure-access.lisp", DEFAULT_ITERATIONS, lambda n: 40 * n),
    MicroCase("array-access", "array-access.lisp", DEFAULT_ITERATIONS, lambda n: n),
    MicroCase("cons-allocation", "cons-allocation.lisp", DEFAULT_ITERATIONS, lambda n: n),
    MicroCase("tail-recursion", "tail-recursion.lisp", DEFAULT_ITERATIONS, lambda n: n),
    MicroCase("closure", "closure.lisp", DEFAULT_ITERATIONS, lambda n: n * (n - 1) // 2 + 7 * n),
    MicroCase("key-call", "key-call.lisp", DEFAULT_ITERATIONS, lambda n: n * (n + 1) // 2),
)
MICRO_CASE_BY_NAME = {case.name: case for case in MICRO_CASES}


def _positive_count(value: str) -> int:
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a positive integer") from error
    if count < 1:
        raise argparse.ArgumentTypeError("must be a positive integer")
    return count


def _paired_count(value: str) -> int:
    count = _positive_count(value)
    if count < 5:
        raise argparse.ArgumentTypeError("at least 5 paired runs are required")
    return count


def _select_cases(names: Sequence[str] | None) -> tuple[MicroCase, ...]:
    if names is None:
        return MICRO_CASES
    by_casefold = {name.casefold(): case for name, case in MICRO_CASE_BY_NAME.items()}
    selected: list[MicroCase] = []
    seen: set[str] = set()
    for name in names:
        if not isinstance(name, str) or name.casefold() not in by_casefold:
            raise ValueError(f"unknown micro benchmark: {name}")
        case = by_casefold[name.casefold()]
        if case.name in seen:
            raise ValueError(f"duplicate micro benchmark: {case.name}")
        selected.append(case)
        seen.add(case.name)
    if not selected:
        raise ValueError("at least one micro benchmark must be selected")
    return tuple(selected)


def _lisp_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _driver_source(case: MicroCase, engine: str, iterations: int, repetitions: int = 1) -> str:
    if not isinstance(repetitions, int) or isinstance(repetitions, bool) or repetitions < 1:
        raise ValueError("repetitions must be a positive integer")
    source = _lisp_string(str((CASE_DIR / case.source).resolve()))
    measured_calls = "\n".join(
        f"      (micro-case {iterations})" for _ in range(repetitions)
    )
    if engine == "ncl":
        missing_counter_reason = "NCL-GC:BYTES-CONSED-SINCE-GC unavailable"
        allocation_setup = '''(defun micro-counter-info ()
  (let* ((package (find-package "NCL-GC"))
         (bytes-symbol (and package (find-symbol "BYTES-CONSED-SINCE-GC" package)))
         (gc-symbol (and package (find-symbol "GC-COUNT" package))))
    (list (and bytes-symbol (fboundp bytes-symbol) bytes-symbol)
          (and gc-symbol (fboundp gc-symbol) gc-symbol))))

(defun micro-collect ()
  (let* ((package (find-package "NCL-GC"))
         (symbol (and package (find-symbol "GC" package))))
    (when (and symbol (fboundp symbol))
      (funcall symbol))))'''
        gc_logic = '''(gc-count-counter (second counter-info))
         (counter-name (if counter "NCL-GC:BYTES-CONSED-SINCE-GC" "unavailable"))
         (gc-required t)'''
    else:
        missing_counter_reason = "SB-EXT:GET-BYTES-CONSED unavailable"
        allocation_setup = '''(defun micro-counter-info ()
  (let* ((package (find-package "SB-EXT"))
         (symbol (and package (find-symbol "GET-BYTES-CONSED" package))))
    (and symbol (fboundp symbol) (list symbol))))

(defun micro-collect ()
  (let* ((package (find-package "SB-EXT"))
         (symbol (and package (find-symbol "GC" package))))
    (when (and symbol (fboundp symbol))
      (funcall symbol))))'''
        gc_logic = '''(gc-count-counter nil)
         (counter-name (if counter "SB-EXT:GET-BYTES-CONSED" "unavailable"))
         (gc-required nil)'''

    return f'''(load {source})

{allocation_setup}

(defun micro-disassemble-available-p ()
  (let* ((package (find-package "COMMON-LISP"))
         (symbol (and package (find-symbol "DISASSEMBLE" package))))
    (and symbol (fboundp symbol))))

(let* ((counter-info (micro-counter-info))
       (counter (first counter-info))
       {gc_logic})
  (micro-case {iterations})
  (micro-collect)
  (let* ((gc-count-before (and gc-count-counter (funcall gc-count-counter)))
         (allocated-before (and counter (funcall counter)))
         (started (get-internal-real-time))
         (repetitions {repetitions})
         (result nil)
         (ticks 0))
    (setf result
          (progn
{measured_calls}
          ))
    (setf ticks (- (get-internal-real-time) started))
    (let* ((allocated-after (and counter (funcall counter)))
         (gc-count-after (and gc-count-counter (funcall gc-count-counter)))
         (gc-count-valid
           (or (not gc-required)
               (and (integerp gc-count-before)
                    (>= gc-count-before 0)
                    (integerp gc-count-after)
                    (>= gc-count-after 0)
                    (= gc-count-before gc-count-after))))
         (allocated-bytes
           (and counter gc-count-valid
                (integerp allocated-before)
                (>= allocated-before 0)
                (integerp allocated-after)
                (>= allocated-after 0)
                (<= allocated-before allocated-after)
                (- allocated-after allocated-before)))
         (allocation-json (if allocated-bytes
                              (format nil "~D" allocated-bytes)
                              "null"))
         (gc-before-json (if (and (integerp gc-count-before) (>= gc-count-before 0))
                             (format nil "~D" gc-count-before)
                             "null"))
         (gc-after-json (if (and (integerp gc-count-after) (>= gc-count-after 0))
                            (format nil "~D" gc-count-after)
                            "null"))
         (allocation-reason
           (cond ((null counter) "{missing_counter_reason}")
                 ((and gc-required (null gc-count-counter))
                  "NCL-GC:GC-COUNT unavailable")
                 ((and gc-required (not gc-count-valid))
                  (if (and (integerp gc-count-before) (integerp gc-count-after))
                      "GC occurred during sample"
                      "NCL-GC:GC-COUNT returned invalid values"))
                 ((not (and (integerp allocated-before)
                            (integerp allocated-after)))
                  "allocation counter returned invalid values")
                 ((> allocated-before allocated-after)
                  "allocation counter reset during sample")
                 (t ""))))
    (format t "MICRO_RESULT {{\\\"case\\\":~S,\\\"engine\\\":~S,\\\"result\\\":~D,\\\"ticks\\\":~D,\\\"units_per_second\\\":~D,\\\"allocated_bytes\\\":~A,\\\"allocation_counter\\\":~S,\\\"allocation_reason\\\":~S,\\\"gc_count_before\\\":~A,\\\"gc_count_after\\\":~A,\\\"disassemble_available\\\":~A}}~%"
            "{case.name}" "{engine}" result ticks internal-time-units-per-second
            allocation-json counter-name allocation-reason gc-before-json gc-after-json
            (if (micro-disassemble-available-p) "true" "false"))
    (format t "~%MICRO_REPETITIONS ~D~%" repetitions))))
'''
