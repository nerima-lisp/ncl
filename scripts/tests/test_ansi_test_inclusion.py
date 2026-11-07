import importlib.util
from pathlib import Path
import sys
import unittest
from tempfile import TemporaryDirectory
from unittest.mock import patch


SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
INCLUSION_PATH = SCRIPTS / "ansi_test_inclusion.py"
spec = importlib.util.spec_from_file_location("ansi_test_inclusion", INCLUSION_PATH)
inclusion = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = inclusion
spec.loader.exec_module(inclusion)


class AnsiRunnerTests(unittest.TestCase):
    def test_runner_protocol_avoids_format_and_declares_each_supported_policy_first(self):
        for policy, declaim in inclusion.POLICIES.items():
            with self.subTest(policy=policy):
                runner = inclusion._test_runner("ARRAYS.1", policy)
                baseline = inclusion._baseline_runner(policy)
                self.assertNotIn("(format ", runner.lower())
                self.assertNotIn("(format ", baseline.lower())
                self.assertLess(runner.index(declaim), runner.index('(load "gclload1.lsp")'))
                self.assertLess(baseline.index(declaim), baseline.index('(load "gclload1.lsp")'))
                self.assertIn("write-string", runner)
                self.assertIn("write-char #\\Tab", runner)
                self.assertIn("load:gclload1.lsp", runner)
                self.assertIn("execute-test", runner)
                self.assertIn("princ-to-string condition", runner)

    def test_pre_begin_typeerror_is_classified_as_runner_initialization(self):
        captured_stderr = "ncl: object error: TypeError\n"
        captured_stdout = f"{inclusion.PROTOCOL}\tPHASE\tFORMAT.1\tload:gclload1.lsp\n"
        self.assertEqual(inclusion._ncl_error_kind(captured_stderr), "TypeError")
        with TemporaryDirectory() as directory:
            with patch.object(
                inclusion,
                "_run_process",
                return_value=("process", captured_stdout, captured_stderr, 1),
            ):
                row = inclusion._run_test(
                    ["ncl", "--load"], Path(directory), "FORMAT.1", "printer/format", 30,
                )

        self.assertEqual(row["status"], "harness_error")
        self.assertEqual(row["failure_class"], "runner-initialization")
        self.assertEqual(row["startup_phase"], "load:gclload1.lsp")
        self.assertEqual(row["error_kind"], "TypeError")
        self.assertEqual(row["termination"], "exit")
        self.assertEqual(row["exit_code"], 1)
        self.assertEqual(row["process_exit_status"], 1)
        self.assertEqual(row["diagnostic"], "ncl: object error: TypeError")
        self.assertIn("before BEGIN during load:gclload1.lsp", row["reason"])
        summary = inclusion.failure_summary([row])
        self.assertEqual(summary[0]["failure_class"], "runner-initialization")
        self.assertEqual(summary[0]["condition_type"], "TypeError")
        self.assertEqual(summary[0]["startup_phase"], "load:gclload1.lsp")
        self.assertEqual(summary[0]["count"], 1)

    def test_ncl_typeerror_after_begin_remains_a_test_error(self):
        stdout = f"{inclusion.PROTOCOL}\tBEGIN\tFORMAT.1\n"
        with TemporaryDirectory() as directory:
            with patch.object(
                inclusion,
                "_run_process",
                return_value=("process", stdout, "ncl: object error: TypeError\n", 1),
            ):
                row = inclusion._run_test(
                    ["ncl", "--load"], Path(directory), "FORMAT.1", "printer/format", 30,
                )
        self.assertEqual(row["status"], "error")
        self.assertEqual(row["failure_class"], "ncl-error")
        self.assertNotIn("startup_phase", row)

    def test_captured_condition_type_and_multiline_tabbed_message_are_preserved(self):
        protocol = inclusion.PROTOCOL
        stdout = "\n".join((
            f"{protocol}\tPHASE\tARRAYS.1\texecute-test",
            f"{protocol}\tBEGIN\tARRAYS.1",
            f"{protocol}\tERROR\tARRAYS.1\tSIMPLE-ERROR",
            f"{protocol}\tMESSAGE\tbad\tvalue",
            "continued detail",
            f"{protocol}\tDONE\tARRAYS.1",
        ))
        parsed = inclusion._parse_records(stdout)
        self.assertEqual(parsed["phases"]["ARRAYS.1"], "execute-test")
        self.assertEqual(parsed["conditions"]["ARRAYS.1"], {
            "condition_type": "SIMPLE-ERROR",
            "message": "bad\tvalue\ncontinued detail",
        })
        with TemporaryDirectory() as directory:
            with patch.object(inclusion, "_run_process", return_value=("process", stdout, "", 0)):
                row = inclusion._run_test(
                    ["sbcl", "--script"], Path(directory), "ARRAYS.1", "arrays", 30,
                )
        self.assertEqual(row["failure_class"], "condition")
        self.assertEqual(row["condition_type"], "SIMPLE-ERROR")
        self.assertEqual(row["message"], "bad\tvalue\ncontinued detail")
        self.assertEqual(row["exit_code"], 0)

    def test_termination_categories_keep_assertions_nonzero_exits_timeouts_signals_and_spawn_errors(self):
        cases = (
            (("process", f"{inclusion.PROTOCOL}\tBEGIN\tARRAYS.1", "ordinary error\n", 2), "exit", "exit", 2),
            (("timeout", "partial output", "timed out", None), "timeout", "timeout", None),
            (("harness_error", "", "cannot spawn", None), "harness", "spawn_error", None),
            (("process", "", "Segmentation fault\n", -11), "crash", "signal", None),
            (("process", "\n".join((
                f"{inclusion.PROTOCOL}\tBEGIN\tARRAYS.1",
                f"{inclusion.PROTOCOL}\tFAIL\tARRAYS.1",
                f"{inclusion.PROTOCOL}\tDONE\tARRAYS.1",
            )), "", 0), "assertion", "exit", 0),
        )
        for process_result, failure_class, termination, exit_code in cases:
            with self.subTest(failure_class=failure_class), TemporaryDirectory() as directory:
                with patch.object(inclusion, "_run_process", return_value=process_result):
                    row = inclusion._run_test(
                        ["ncl"], Path(directory), "ARRAYS.1", "arrays", 1,
                    )
            self.assertEqual(row["failure_class"], failure_class)
            self.assertEqual(row["termination"], termination)
            self.assertEqual(row.get("exit_code"), exit_code)
            if termination == "signal":
                self.assertEqual(row["signal_number"], 11)


if __name__ == "__main__":
    unittest.main()
