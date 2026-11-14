import argparse
import json
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPTS_DIR = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))
import micro_bench as micro
import conformance_scoreboard as scoreboard


class FakeRunner:
    def __init__(
        self,
        *,
        iterations=100,
        seconds_per_repetition=0.1,
        zero_first_sample=False,
        fail_case=None,
    ):
        self.calls = []
        self.drivers = []
        self.repetition_counts = []
        self.iterations = iterations
        self.seconds_per_repetition = seconds_per_repetition
        self.zero_first_sample = zero_first_sample
        self.fail_case = fail_case

    def __call__(self, command, *, cwd, timeout):
        command = list(command)
        self.calls.append((command, cwd, timeout))
        if command[-1:] == ["--version"]:
            version = "SBCL 2.6.10\n" if command[0] == "sbcl" else "NCL test\n"
            return scoreboard.ProcessResult("passed", 0, version)

        engine = "sbcl" if command[0] == "sbcl" else "ncl"
        driver = Path(command[-1]).read_text(encoding="utf-8")
        self.drivers.append(driver)
        repetitions = int(re.search(r"\(repetitions ([0-9]+)\)", driver).group(1))
        self.repetition_counts.append(repetitions)
        case = next(
            item.name
            for item in micro.MICRO_CASES
            if f'"{item.name}" "{engine}" result' in driver
        )
        if self.fail_case == (case, engine):
            return scoreboard.ProcessResult("failed", 134, "", "stack overflow")
        ticks = (
            0
            if self.zero_first_sample and len(self.repetition_counts) == 1
            else round(self.seconds_per_repetition * repetitions * 1_000_000)
        )
        record = {
            "case": case,
            "engine": engine,
            "result": micro.MICRO_CASE_BY_NAME[case].expected_result(self.iterations),
            "ticks": ticks,
            "units_per_second": 1_000_000,
            "allocated_bytes": 100 * repetitions if engine == "sbcl" else None,
            "allocation_counter": (
                "SB-EXT:GET-BYTES-CONSED" if engine == "sbcl" else "unavailable"
            ),
            "allocation_reason": (
                "" if engine == "sbcl"
                else "NCL-GC:BYTES-CONSED-SINCE-GC unavailable"
            ),
            "gc_count_before": None,
            "gc_count_after": None,
            "disassemble_available": engine == "sbcl",
        }
        return scoreboard.ProcessResult(
            "passed",
            0,
            micro.RESULT_PREFIX
            + json.dumps(record)
            + f"\nMICRO_REPETITIONS {repetitions}\n",
        )


class MicroBenchTests(unittest.TestCase):
    def test_manifest_contains_ten_nonempty_micro_cases(self):
        expected = {
            "float-loop",
            "fixnum-loop",
            "car-access",
            "clos-dispatch",
            "structure-access",
            "array-access",
            "cons-allocation",
            "tail-recursion",
            "closure",
            "key-call",
        }
        self.assertEqual({case.name for case in micro.MICRO_CASES}, expected)
        self.assertEqual(len(micro.MICRO_CASES), 10)
        workload_forms = {
            "float-loop": "(float index 1.0d0)",
            "fixnum-loop": "(incf sum index)",
            "car-access": "(incf sum (car cell))",
            "clos-dispatch": "(defgeneric micro-dispatch",
            "structure-access": "(micro-record-left record)",
            "array-access": "(aref array",
            "cons-allocation": "(cons index nil)",
            "tail-recursion": "(walk (1- remaining)",
            "closure": "(lambda (value) (+ value offset))",
            "key-call": "(defun micro-key-helper (&key",
        }
        for case in micro.MICRO_CASES:
            source = micro.CASE_DIR / case.source
            self.assertTrue(source.is_file(), case.name)
            content = source.read_text(encoding="utf-8")
            self.assertIn("(defun micro-case", content)
            self.assertIn(workload_forms[case.name], content, case.name)
            depth = 0
            in_string = False
            escaped = False
            for character in content:
                if in_string:
                    if escaped:
                        escaped = False
                    elif character == "\\":
                        escaped = True
                    elif character == '"':
                        in_string = False
                elif character == '"':
                    in_string = True
                elif character == "(":
                    depth += 1
                elif character == ")":
                    depth -= 1
                    self.assertGreaterEqual(depth, 0, case.name)
            self.assertEqual(depth, 0, case.name)
            self.assertFalse(in_string, case.name)

    def test_defaults_use_five_alternating_pairs_and_fixed_workload(self):
        arguments = micro._parser().parse_args(["--ncl", "ncl"])
        self.assertEqual(arguments.runs, 5)
        self.assertEqual(arguments.iterations, 10_000)
        self.assertEqual(arguments.sbcl, "sbcl")
        self.assertEqual(micro._paired_count("5"), 5)
        with self.assertRaises(argparse.ArgumentTypeError):
            micro._paired_count("4")

    def test_case_outputs_match_the_declared_work_for_ten_iterations(self):
        self.assertEqual(
            {case.name: case.expected_result(10) for case in micro.MICRO_CASES},
            {
                "float-loop": 22,
                "fixnum-loop": 45,
                "car-access": 410,
                "clos-dispatch": 20,
                "structure-access": 400,
                "array-access": 10,
                "cons-allocation": 10,
                "tail-recursion": 10,
                "closure": 115,
                "key-call": 55,
            },
        )

    def test_driver_uses_exact_counter_apis_and_rejects_samples_spanning_gc(self):
        case = micro.MICRO_CASES[0]
        ncl_driver = micro._driver_source(case, "ncl", 10)
        sbcl_driver = micro._driver_source(case, "sbcl", 10)

        self.assertIn('find-symbol "BYTES-CONSED-SINCE-GC"', ncl_driver)
        self.assertIn('find-symbol "GC-COUNT"', ncl_driver)
        self.assertIn("(= gc-count-before gc-count-after)", ncl_driver)
        self.assertNotIn("GET-BYTES-CONSED", ncl_driver)
        self.assertIn('find-symbol "GET-BYTES-CONSED"', sbcl_driver)
        self.assertNotIn('find-package "NCL-GC"', sbcl_driver)
        self.assertNotIn('find-symbol "GC-COUNT"', sbcl_driver)
        for generated in (ncl_driver, sbcl_driver):
            self.assertIn("MICRO_REPETITIONS", generated)
            self.assertIn("(progn", generated)
            self.assertNotIn("(loop", generated)
            depth = 0
            in_string = False
            escaped = False
            for character in generated:
                if in_string:
                    if escaped:
                        escaped = False
                    elif character == "\\":
                        escaped = True
                    elif character == '"':
                        in_string = False
                elif character == '"':
                    in_string = True
                elif character == "(":
                    depth += 1
                elif character == ")":
                    depth -= 1
                    self.assertGreaterEqual(depth, 0)
            self.assertEqual(depth, 0)
            self.assertFalse(in_string)

    def test_ncl_driver_uses_static_forms_and_micro_retries_until_minimum(self):
        case = micro.MICRO_CASE_BY_NAME["float-loop"]
        ncl_driver = micro._driver_source(case, "ncl", 10, repetitions=3)
        self.assertNotIn("(loop", ncl_driver)
        self.assertIn("(progn", ncl_driver)
        self.assertEqual(ncl_driver.count("(micro-case 10)"), 4)

        for engine in ("ncl", "sbcl"):
            runner = FakeRunner(
                iterations=10,
                seconds_per_repetition=0.025,
                zero_first_sample=True,
            )
            with tempfile.TemporaryDirectory() as temporary:
                with patch.object(micro, "_run_command", side_effect=runner):
                    sample = micro._sample(
                        binary=engine,
                        engine=engine,
                        case=case,
                        pair=1,
                        iterations=10,
                        driver_path=Path(temporary) / "micro-adaptive-driver.lisp",
                    )

            with self.subTest(engine=engine):
                self.assertEqual(runner.repetition_counts, [1, 2, 4])
                self.assertTrue(all("(loop" not in driver for driver in runner.drivers))
                self.assertEqual(sample["status"], "passed")
                self.assertEqual(sample["result"], case.expected_result(10))
                self.assertEqual(sample["repetitions"], 4)
                self.assertEqual(sample["iteration_count"], 40)
                self.assertAlmostEqual(sample["elapsed_seconds"], micro.MINIMUM_SAMPLE_SECONDS)
                self.assertAlmostEqual(sample["seconds"], 0.025)
                if engine == "sbcl":
                    self.assertEqual(sample["allocated_bytes_per_batch"], 100)
                else:
                    self.assertIsNone(sample["allocated_bytes"])

    def test_suite_preserves_results_allocation_gaps_and_pair_order(self):
        runner = FakeRunner(iterations=100)
        with (
            patch.dict("os.environ", {"NCL_BENCH_REVISION": "a" * 40}),
            patch.object(micro, "_run_command", side_effect=runner),
        ):
            report = micro.run_micro_suite(
                ncl="ncl",
                sbcl="sbcl",
                runs=5,
                iterations=100,
            )

        self.assertEqual(report["status"], "partial")
        self.assertEqual(len(report["cases"]), 10)
        self.assertEqual(
            report["protocol"]["pair_order"],
            [["ncl", "sbcl"], ["sbcl", "ncl"], ["ncl", "sbcl"], ["sbcl", "ncl"], ["ncl", "sbcl"]],
        )
        case = report["cases"]["float-loop"]
        self.assertEqual(case["engines"]["ncl"]["sample_count"], 5)
        self.assertEqual(case["engines"]["ncl"]["allocation_status"], "unavailable")
        self.assertEqual(case["engines"]["ncl"]["range_seconds"], 0.0)
        paired = case["paired_comparison"]
        self.assertEqual(paired["sample_count"], 5)
        self.assertEqual(paired["median_delta_seconds"], 0.0)
        self.assertEqual((paired["ncl_wins"], paired["sbcl_wins"], paired["ties"]), (0, 0, 5))
        self.assertTrue(paired["noise_below"])
        self.assertIn(
            "NCL-GC:BYTES-CONSED-SINCE-GC",
            case["engines"]["ncl"]["allocation_reason"],
        )
        self.assertEqual(case["engines"]["sbcl"]["allocation_status"], "measured")
        self.assertEqual(case["engines"]["sbcl"]["median_allocated_bytes"], 100)
        self.assertEqual(case["engines"]["sbcl"]["samples"][0]["repetitions"], 1)
        self.assertEqual(case["engines"]["sbcl"]["samples"][0]["iteration_count"], 100)
        self.assertAlmostEqual(case["engines"]["sbcl"]["samples"][0]["elapsed_seconds"], 0.1)
        self.assertAlmostEqual(case["engines"]["sbcl"]["median_seconds"], 0.1)
        self.assertEqual(report["measurement"]["sbcl_version"], "2.6.10")
        self.assertEqual(report["measurement"]["ncl_sha"], "a" * 40)
        self.assertEqual(report["protocol"]["minimum_sample_seconds"], micro.MINIMUM_SAMPLE_SECONDS)
        self.assertEqual(case["output_parity"]["status"], "passed")
        self.assertEqual(case["codegen_inspection"]["status"], "unsupported")
        self.assertEqual(
            case["codegen_inspection"]["reason"],
            "COMMON-LISP:DISASSEMBLE is not fbound in NCL.",
        )

        measured_calls = [
            Path(command[-1]).name
            for command, _cwd, _timeout in runner.calls
            if "--script" in command
        ]
        first_case_calls = [name for name in measured_calls if name.startswith("float-loop-")]
        self.assertEqual(
            first_case_calls,
            [
                "float-loop-1-ncl.lisp",
                "float-loop-1-sbcl.lisp",
                "float-loop-2-sbcl.lisp",
                "float-loop-2-ncl.lisp",
                "float-loop-3-ncl.lisp",
                "float-loop-3-sbcl.lisp",
                "float-loop-4-sbcl.lisp",
                "float-loop-4-ncl.lisp",
                "float-loop-5-ncl.lisp",
                "float-loop-5-sbcl.lisp",
            ],
        )

    def test_runtime_failure_remains_in_the_report_with_its_reason(self):
        runner = FakeRunner(iterations=10, fail_case=("tail-recursion", "ncl"))
        with patch.object(micro, "_run_command", side_effect=runner):
            report = micro.run_micro_suite(
                ncl="ncl",
                sbcl="sbcl",
                runs=5,
                iterations=10,
                cases=["tail-recursion"],
            )

        sample = report["cases"]["tail-recursion"]["engines"]["ncl"]["samples"][0]
        self.assertEqual(sample["status"], "failed")
        self.assertEqual(sample["return_code"], 134)
        self.assertEqual(sample["reason"], "stack overflow")
        self.assertEqual(report["cases"]["tail-recursion"]["output_parity"]["status"], "unverified")

    def test_invalid_or_duplicate_case_names_are_rejected(self):
        for names, message in (
            (["not-a-case"], "unknown micro benchmark"),
            (["float-loop", "FLOAT-LOOP"], "duplicate micro benchmark"),
            ([], "at least one micro benchmark"),
        ):
            with self.subTest(names=names):
                with self.assertRaisesRegex(ValueError, message):
                    micro._select_cases(names)

    def test_nonpositive_timer_and_negative_allocation_values_are_rejected(self):
        case = micro.MICRO_CASES[0]
        record = {
            "case": case.name,
            "engine": "ncl",
            "result": 1,
            "ticks": 0,
            "units_per_second": 1_000_000,
            "allocated_bytes": 1,
            "allocation_counter": "counter",
            "allocation_reason": "",
            "gc_count_before": None,
            "gc_count_after": None,
            "disassemble_available": False,
        }
        output = micro.RESULT_PREFIX + json.dumps(record) + "\nMICRO_REPETITIONS 1"
        with self.assertRaisesRegex(ValueError, "time must be positive"):
            micro._parse_result(output, case, "ncl")
        record["ticks"] = 100_000
        record["allocated_bytes"] = -1
        with self.assertRaisesRegex(ValueError, "non-negative integer"):
            micro._parse_result(
                micro.RESULT_PREFIX + json.dumps(record) + "\nMICRO_REPETITIONS 1",
                case,
                "ncl",
            )
        record["allocated_bytes"] = 1
        record["allocation_counter"] = "unavailable"
        record["allocation_reason"] = "NCL-GC:BYTES-CONSED-SINCE-GC unavailable"
        with self.assertRaisesRegex(ValueError, "without an allocation counter"):
            micro._parse_result(
                micro.RESULT_PREFIX + json.dumps(record) + "\nMICRO_REPETITIONS 1",
                case,
                "ncl",
            )

    def test_ncl_bytes_are_invalid_when_gc_epoch_changes(self):
        case = micro.MICRO_CASES[0]
        record = {
            "case": case.name,
            "engine": "ncl",
            "result": case.expected_result(10),
            "ticks": 100_000,
            "units_per_second": 1_000_000,
            "allocated_bytes": 1,
            "allocation_counter": "NCL-GC:BYTES-CONSED-SINCE-GC",
            "allocation_reason": "",
            "gc_count_before": 4,
            "gc_count_after": 5,
            "disassemble_available": False,
        }
        with self.assertRaisesRegex(ValueError, "stable GC count snapshots"):
            micro._parse_result(
                micro.RESULT_PREFIX + json.dumps(record) + "\nMICRO_REPETITIONS 1",
                case,
                "ncl",
                iterations=10,
            )
        record["allocated_bytes"] = None
        record["allocation_reason"] = "GC occurred during sample"
        parsed = micro._parse_result(
            micro.RESULT_PREFIX + json.dumps(record) + "\nMICRO_REPETITIONS 1",
            case,
            "ncl",
            iterations=10,
        )
        self.assertIsNone(parsed["allocated_bytes"])
        self.assertEqual(parsed["allocation_reason"], "GC occurred during sample")


if __name__ == "__main__":
    unittest.main()
