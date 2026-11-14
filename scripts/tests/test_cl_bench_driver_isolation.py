from __future__ import annotations

import contextlib
import io
import json
import os
import shutil
import sys
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch


REPO_ROOT = Path(__file__).resolve().parents[2]
DRIVER_DIR = REPO_ROOT / "conformance" / "cl-bench"
sys.path.insert(0, str(DRIVER_DIR))

import driver_supervisor


class DriverCommandTests(unittest.TestCase):
    def test_sbcl_command_disables_system_and_user_initialization(self) -> None:
        with patch.object(driver_supervisor, "_eval_form", return_value="(progn)"):
            command = driver_supervisor._command(
                engine="sbcl",
                runner="sbcl",
                mode="default",
                driver=Path("driver.lisp"),
                cwd=Path("corpus"),
                selection=("FIB",),
            )

        self.assertIn("--no-sysinit", command)
        self.assertIn("--userinit", command)
        self.assertLess(command.index("--no-sysinit"), command.index("--eval"))

    def test_benchmark_controls_are_injected_without_loop_payload(self) -> None:
        with patch.object(driver_supervisor, "_package_bootstrap_forms", return_value=("(defpackage)", ())):
            form = driver_supervisor._eval_form(
                mode="run",
                driver=Path("driver.lisp"),
                cwd=Path("corpus"),
                selection=("FIB",),
                runs=3,
                minimum_sample_seconds=0.1,
            )

        self.assertIn("*cl-bench-runs-override* 3", form)
        self.assertIn("*cl-bench-minimum-sample-seconds* 0.1", form)
        self.assertNotIn("loop", form.lower())

    def test_cli_passes_runs_floor_and_child_timeout_to_supervisor(self) -> None:
        with patch.object(driver_supervisor, "run_supervisor", return_value=0) as run:
            exit_code = driver_supervisor.main([
                "--engine", "ncl",
                "--runner", "ncl",
                "--driver", "driver.lisp",
                "--cwd", ".",
                "--runs", "3",
                "--minimum-sample-seconds", "0.1",
                "--child-timeout-seconds", "45",
                "--include-sample-metadata",
                "FIB",
            ])

        self.assertEqual(exit_code, 0)
        self.assertEqual(run.call_args.kwargs["runs"], 3)
        self.assertEqual(run.call_args.kwargs["minimum_sample_seconds"], 0.1)
        self.assertEqual(run.call_args.kwargs["child_timeout_seconds"], 45.0)
        self.assertTrue(run.call_args.kwargs["include_sample_metadata"])

    def test_supervisor_cli_keeps_existing_defaults(self) -> None:
        with patch.object(driver_supervisor, "run_supervisor", return_value=0) as run:
            exit_code = driver_supervisor.main([
                "--engine", "sbcl",
                "--runner", "sbcl",
                "--driver", "driver.lisp",
                "--cwd", ".",
            ])

        self.assertEqual(exit_code, 0)
        self.assertIsNone(run.call_args.kwargs["runs"])
        self.assertEqual(
            run.call_args.kwargs["minimum_sample_seconds"],
            driver_supervisor.MINIMUM_SAMPLE_SECONDS,
        )
        self.assertEqual(
            run.call_args.kwargs["child_timeout_seconds"],
            driver_supervisor.CHILD_TIMEOUT_SECONDS,
        )
        self.assertFalse(run.call_args.kwargs["include_sample_metadata"])

    def test_configured_child_timeout_bounds_each_subprocess(self) -> None:
        class Child:
            pid = 123
            returncode = 0

            def communicate(self, *, timeout):
                self.timeout = timeout
                return "", ""

        child = Child()
        with patch.object(driver_supervisor.subprocess, "Popen", return_value=child):
            result = driver_supervisor._run_child(["runner"], Path("."), timeout=45.0)

        self.assertEqual(result.returncode, 0)
        self.assertEqual(child.timeout, 45.0)
        self.assertEqual(
            driver_supervisor._failure_detail(
                subprocess.CompletedProcess(["runner"], None, "", ""), 45.0
            ),
            "child exceeded its 45 second timeout",
        )

    def test_child_timeout_is_forwarded_only_when_non_default(self) -> None:
        process = subprocess.CompletedProcess(["runner"], 0, "", "")
        with patch.object(
            driver_supervisor, "_run_child", return_value=process
        ) as run_child:
            default_result = driver_supervisor._run_child_with_timeout(
                ["runner"], Path("."), driver_supervisor.CHILD_TIMEOUT_SECONDS
            )
            run_child.assert_called_once_with(["runner"], Path("."))

            run_child.reset_mock()
            custom_result = driver_supervisor._run_child_with_timeout(
                ["runner"], Path("."), 45.0
            )

        self.assertIs(default_result, process)
        self.assertIs(custom_result, process)
        run_child.assert_called_once_with(["runner"], Path("."), timeout=45.0)


@unittest.skipUnless(shutil.which("sbcl"), "SBCL is required for the driver fixture")
class DriverIsolationTests(unittest.TestCase):
    def test_signal_termination_is_reported_as_a_failed_stage_condition(self) -> None:
        row = driver_supervisor._worker_failure_row(
            "fixture/sigsegv",
            subprocess.CompletedProcess(
                ["ncl"],
                -11,
                stdout="",
                stderr="CL-BENCH-STAGE execute fixture/sigsegv\n",
            ),
        )

        self.assertEqual(row["status"], "failed")
        self.assertEqual(row["stage"], "execute")
        self.assertEqual(row["times"], [])
        self.assertEqual(
            row["condition"],
            "driver child terminated by signal 11 (SIGSEGV)",
        )

    def test_package_bootstrap_forms_are_emitted_before_driver_load(self) -> None:
        with tempfile.TemporaryDirectory(prefix="cl-bench-package-bootstrap-test-") as temporary:
            cwd = Path(temporary)
            (cwd / "files").mkdir()
            (cwd / "package.lisp").write_text(
                "(defpackage #:cl-bench (:use #:cl) (:export #:*misc-dir*))\n",
                encoding="utf-8",
            )
            (cwd / "files" / "fixture.lisp").write_text(
                """#|
(defpackage :cl-bench.ignored (:use :cl))
|#
(defpackage :cl-bench.fixture
  (:use :common-lisp)
  (:documentation "parentheses ) and ( stay in this string"))
""",
                encoding="utf-8",
            )

            with patch.object(driver_supervisor, "SOURCE_FILES", ("fixture.lisp",)):
                support_package, source_packages = driver_supervisor._package_bootstrap_forms(cwd)
                child_form = driver_supervisor._eval_form(
                    mode="run",
                    driver=DRIVER_DIR / "sbcl-driver.lisp",
                    cwd=cwd,
                    selection=("fixture/example",),
                )

        self.assertEqual(len(source_packages), 1)
        self.assertIn("#:*misc-dir*", support_package)
        self.assertIn("cl-bench.fixture", source_packages[0])
        self.assertNotIn("cl-bench.ignored", source_packages[0])
        support_eval = f"(eval '{support_package})"
        source_eval = f"(eval '{source_packages[0]})"
        driver_load = f"(load {driver_supervisor._lisp_string(str(DRIVER_DIR / 'sbcl-driver.lisp'))})"
        self.assertLess(child_form.index(support_eval), child_form.index(source_eval))
        self.assertLess(child_form.index(source_eval), child_form.index(driver_load))

    def test_source_registration_and_execution_failures_are_isolated(self) -> None:
        with tempfile.TemporaryDirectory(prefix="cl-bench-driver-test-") as temporary:
            cwd = Path(temporary)
            (cwd / "files").mkdir()
            (cwd / "package.lisp").write_text(
                "(defpackage :cl-bench (:use :cl) (:export :*misc-dir*))\n",
                encoding="utf-8",
            )
            (cwd / "files" / "broken.lisp").write_text(
                """(defpackage :cl-bench.fixture
  (:use :cl)
  (:export :broken-load :broken-execute :later-finish :exit-without-json))
(in-package :cl-bench.fixture)
(defun broken-load () nil)
(error "synthetic source load failure")
""",
                encoding="utf-8",
            )
            (cwd / "files" / "runners.lisp").write_text(
                """(in-package :cl-bench.fixture)
(defun broken-execute () (error "synthetic execution failure"))
(defun later-finish () (sleep 0.01) t)
(defun exit-without-json () (sb-ext:exit :code 23))
""",
                encoding="utf-8",
            )
            (cwd / "tests.lisp").write_text(
                """(in-package :cl-bench)
(defbench load-fails
  :short "fixture/load"
  :function 'cl-bench.fixture:broken-load
  :runs 1)
(defbench register-fails
  :short "fixture/register"
  :function (error "synthetic registration failure")
  :runs 1)
(defbench execute-fails
  :short "fixture/execute"
  :function 'cl-bench.fixture:broken-execute
  :runs 1)
(defbench duplicate-name
  :short "fixture/later"
  :function 'cl-bench.fixture:later-finish
  :runs 1)
(defbench duplicate-name
  :short "fixture/later-alias"
  :function 'cl-bench.fixture:later-finish
  :runs 1)
(defbench exits-without-json
  :short "fixture/exit"
  :function 'cl-bench.fixture:exit-without-json
  :runs 1)
;; EOF
""",
                encoding="utf-8",
            )
            checkpoint = cwd / "checkpoint.jsonl"

            selection = (
                "fixture/load",
                "fixture/register",
                "fixture/execute",
                "fixture/later",
                "fixture/later-alias",
                "fixture/exit",
            )
            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                patch.object(driver_supervisor, "SOURCE_FILES", ("broken.lisp", "runners.lisp")),
                patch.dict(
                    os.environ,
                    {driver_supervisor.CHECKPOINT_ENV: str(checkpoint)},
                ),
                contextlib.redirect_stdout(stdout),
                contextlib.redirect_stderr(stderr),
            ):
                exit_code = driver_supervisor.main(
                    [
                        "--engine",
                        "sbcl",
                        "--runner",
                        shutil.which("sbcl") or "sbcl",
                        "--mode",
                        "default",
                        "--driver",
                        str(DRIVER_DIR / "sbcl-driver.lisp"),
                        "--cwd",
                        str(cwd),
                        "--include-sample-metadata",
                        *selection,
                    ]
                )
            checkpoint_text = checkpoint.read_text(encoding="utf-8")

        self.assertEqual(exit_code, 1)
        report = json.loads(stdout.getvalue())
        self.assertEqual(set(report), {"status", "times", "benchmarks"})
        self.assertEqual(report["status"], "failed")
        self.assertEqual(set(report["benchmarks"]), set(selection))
        expected = {
            "fixture/load": ("failed", "load"),
            "fixture/register": ("failed", "register"),
            "fixture/execute": ("failed", "execute"),
            "fixture/later": ("passed", "finish"),
            "fixture/later-alias": ("passed", "finish"),
            "fixture/exit": ("failed", "execute"),
        }
        for name, (status, stage) in expected.items():
            row = report["benchmarks"][name]
            self.assertEqual(set(row), {
                "status", "stage", "times", "condition", "wall_time_seconds",
                "sample_metadata",
            })
            self.assertEqual(
                (row["status"], row["stage"]),
                (status, stage),
                f"{name}: {row}; child diagnostics: {stderr.getvalue()}",
            )
            if status == "failed":
                self.assertTrue(row["condition"])
                self.assertEqual(row["times"], [])
            else:
                self.assertIsNone(row["condition"])
                self.assertEqual(len(row["times"]), 1)
                self.assertEqual(len(row["sample_metadata"]), 1)
                sample = row["sample_metadata"][0]
                self.assertIsInstance(sample["duration_seconds"], (int, float))
                self.assertIsInstance(sample["repetitions"], int)
                self.assertIsInstance(sample["minimum_duration_met"], bool)
        self.assertEqual(
            report["times"],
            report["benchmarks"]["fixture/later"]["times"]
            + report["benchmarks"]["fixture/later-alias"]["times"],
        )
        checkpoint_records = [
            json.loads(line) for line in checkpoint_text.splitlines()
        ]
        self.assertEqual(
            checkpoint_records[0],
            {"type": "header", "version": 1, "expected": list(selection)},
        )
        self.assertEqual(
            {record["name"] for record in checkpoint_records[1:]},
            set(selection),
        )
        self.assertEqual(len(checkpoint_records), len(selection) + 1)


    def test_global_registration_form_failure_fails_every_selected_worker(self) -> None:
        with tempfile.TemporaryDirectory(prefix="cl-bench-global-register-test-") as temporary:
            cwd = Path(temporary)
            (cwd / "files").mkdir()
            (cwd / "package.lisp").write_text(
                "(defpackage :cl-bench (:use :cl) (:export :*misc-dir*))\n",
                encoding="utf-8",
            )
            (cwd / "files" / "healthy.lisp").write_text(
                """(defpackage :cl-bench.fixture
  (:use :cl)
  (:export :run-healthy))
(in-package :cl-bench.fixture)
(defun run-healthy () t)
""",
                encoding="utf-8",
            )
            (cwd / "tests.lisp").write_text(
                """(in-package :cl-bench)
(defbench before-global-error
  :short "fixture/before-global-error"
  :function 'cl-bench.fixture:run-healthy
  :runs 1)
(error "synthetic global registration failure")
(defbench after-global-error
  :short "fixture/after-global-error"
  :function 'cl-bench.fixture:run-healthy
  :runs 1)
""",
                encoding="utf-8",
            )

            selection = (
                "fixture/before-global-error",
                "fixture/after-global-error",
                "fixture/never-registered",
            )
            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                patch.object(driver_supervisor, "SOURCE_FILES", ("healthy.lisp",)),
                contextlib.redirect_stdout(stdout),
                contextlib.redirect_stderr(stderr),
            ):
                exit_code = driver_supervisor.main(
                    [
                        "--engine",
                        "sbcl",
                        "--runner",
                        shutil.which("sbcl") or "sbcl",
                        "--mode",
                        "default",
                        "--driver",
                        str(DRIVER_DIR / "sbcl-driver.lisp"),
                        "--cwd",
                        str(cwd),
                        *selection,
                    ]
                )

        self.assertEqual(exit_code, 1)
        report = json.loads(stdout.getvalue())
        self.assertEqual(report["status"], "failed")
        self.assertEqual(set(report["benchmarks"]), set(selection))
        self.assertEqual(report["times"], [])
        for row in report["benchmarks"].values():
            self.assertEqual(set(row), {"status", "stage", "times", "condition"})
            self.assertEqual((row["status"], row["stage"]), ("failed", "register"))
            self.assertEqual(row["times"], [])
            self.assertIn("synthetic global registration failure", row["condition"])


class DriverCheckpointEnvironmentTests(unittest.TestCase):
    def test_valid_passed_row_requires_positive_finite_samples(self) -> None:
        valid = {
            "status": "passed",
            "stage": "finish",
            "times": [0.01, 0.02],
            "condition": None,
        }
        self.assertTrue(driver_supervisor._valid_passed_row(valid))
        for times in ([], [0], [float("nan")], [True], ["0.01"]):
            with self.subTest(times=times):
                self.assertFalse(
                    driver_supervisor._valid_passed_row({**valid, "times": times})
                )

    def test_completed_benchmark_samples_survive_driver_level_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            cwd = Path(temporary)
            (cwd / "files").mkdir()
            (cwd / "package.lisp").write_text(
                "(defpackage #:cl-bench (:use #:cl) (:export #:*misc-dir*))\n",
                encoding="utf-8",
            )
            (cwd / "files" / "fixture.lisp").write_text(
                """(defpackage #:cl-bench.fixture (:use #:cl))
(in-package #:cl-bench.fixture)
(defun run-fib () 1)
""",
                encoding="utf-8",
            )
            child_output = json.dumps(
                {
                    "status": "failed",
                    "times": [0.01, 0.02],
                    "benchmarks": {
                        "fixture/fib": {
                            "status": "passed",
                            "stage": "finish",
                            "times": [0.01, 0.02],
                            "condition": None,
                        }
                    },
                }
            )
            child = subprocess.CompletedProcess(
                ["ncl"], 1, child_output, "cl-bench benchmark execution failed\n"
            )
            stderr = io.StringIO()
            with (
                patch.object(driver_supervisor, "SOURCE_FILES", ("fixture.lisp",)),
                patch.object(driver_supervisor, "_run_child", return_value=child),
                contextlib.redirect_stderr(stderr),
            ):
                report = driver_supervisor._run_benchmark_workers(
                    engine="ncl",
                    runner="ncl",
                    mode="off",
                    driver=Path("ncl-driver.lisp"),
                    selected_rows=("fixture/fib",),
                    registered={"fixture/fib": ("cl-bench.fixture:run-fib",)},
                    failures={},
                    timed_out_sources=set(),
                    preflight_wall_times={},
                    cwd=cwd,
                )

        self.assertEqual(report["status"], "passed")
        self.assertEqual(report["times"], [0.01, 0.02])
        self.assertIn("valid passing row", stderr.getvalue())
        self.assertIn("driver exited 1", stderr.getvalue())
        self.assertIn("cl-bench benchmark execution failed", stderr.getvalue())

    def test_driver_failure_does_not_preserve_inconsistent_partial_json(self) -> None:
        selected = {
            "fixture/fib": {
                "status": "passed",
                "stage": "finish",
                "times": [0.01, 0.02],
                "condition": None,
            }
        }
        extra = {
            "fixture/other": {
                "status": "failed",
                "stage": "load",
                "times": [],
                "condition": "fixture failure",
            }
        }
        cases = (
            ("mismatched times", {"status": "failed", "times": [0.03, 0.04], "benchmarks": selected}),
            ("multiple rows", {"status": "failed", "times": [0.01, 0.02], "benchmarks": selected | extra}),
        )
        for description, payload in cases:
            with self.subTest(description=description), tempfile.TemporaryDirectory() as temporary:
                cwd = Path(temporary)
                (cwd / "files").mkdir()
                (cwd / "package.lisp").write_text(
                    "(defpackage #:cl-bench (:use #:cl) (:export #:*misc-dir*))\n",
                    encoding="utf-8",
                )
                (cwd / "files" / "fixture.lisp").write_text(
                    "(defun run-fib () 1)\n",
                    encoding="utf-8",
                )
                child = subprocess.CompletedProcess(
                    ["ncl"], 1, json.dumps(payload), "cl-bench benchmark execution failed\n"
                )
                stderr = io.StringIO()
                with (
                    patch.object(driver_supervisor, "SOURCE_FILES", ("fixture.lisp",)),
                    patch.object(driver_supervisor, "_run_child", return_value=child),
                    contextlib.redirect_stderr(stderr),
                ):
                    report = driver_supervisor._run_benchmark_workers(
                        engine="ncl",
                        runner="ncl",
                        mode="off",
                        driver=Path("ncl-driver.lisp"),
                        selected_rows=("fixture/fib",),
                        registered={"fixture/fib": ("cl-bench.fixture:run-fib",)},
                        failures={},
                        timed_out_sources=set(),
                        preflight_wall_times={},
                        cwd=cwd,
                    )

                self.assertEqual(report["status"], "failed")
                self.assertEqual(report["benchmarks"]["fixture/fib"]["status"], "failed")
                self.assertEqual(report["benchmarks"]["fixture/fib"]["times"], [])
                self.assertNotIn("valid passing row", stderr.getvalue())

    def test_comparison_benchmarks_are_read_from_the_constants_module(self) -> None:
        benchmarks = driver_supervisor._comparison_benchmarks()

        self.assertEqual(len(benchmarks), 65)
        self.assertEqual(benchmarks[0], "1D-ARRAYS")
        self.assertIn("TRTAK", benchmarks)
        self.assertEqual(len(set(benchmarks)), len(benchmarks))

    def test_checkpoint_path_is_created_exclusively(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            checkpoint = Path(temporary) / "checkpoint.jsonl"
            checkpoint.write_text("preserve this\n", encoding="utf-8")
            with patch.dict(os.environ, {driver_supervisor.CHECKPOINT_ENV: str(checkpoint)}):
                with self.assertRaises(FileExistsError):
                    driver_supervisor.run_supervisor(
                        engine="sbcl",
                        runner="sbcl",
                        mode="default",
                        driver=Path("driver.lisp"),
                        selection=(),
                        cwd=Path(temporary),
                    )

            self.assertEqual(checkpoint.read_text(encoding="utf-8"), "preserve this\n")

    def test_checkpoint_environment_is_removed_before_child_launch(self) -> None:
        child = MagicMock()
        child.returncode = 0
        child.communicate.return_value = ("", "")
        with patch.dict(os.environ, {driver_supervisor.CHECKPOINT_ENV: "/tmp/checkpoint.jsonl"}):
            with patch.object(
                driver_supervisor.subprocess,
                "Popen",
                return_value=child,
            ) as popen:
                driver_supervisor._run_child(["ncl"], Path("."))

        child_environment = popen.call_args.kwargs["env"]
        self.assertNotIn(driver_supervisor.CHECKPOINT_ENV, child_environment)


if __name__ == "__main__":
    unittest.main()
