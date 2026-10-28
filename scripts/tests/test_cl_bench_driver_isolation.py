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
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[2]
DRIVER_DIR = REPO_ROOT / "conformance" / "cl-bench"
sys.path.insert(0, str(DRIVER_DIR))

import driver_supervisor


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
            self.assertEqual(set(row), {"status", "stage", "times", "condition"})
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
        with patch.dict(os.environ, {driver_supervisor.CHECKPOINT_ENV: "/tmp/checkpoint.jsonl"}):
            with patch.object(
                driver_supervisor.subprocess,
                "run",
                return_value=subprocess.CompletedProcess(["ncl"], 0, "", ""),
            ) as run:
                driver_supervisor._run_child(["ncl"], Path("."))

        child_environment = run.call_args.kwargs["env"]
        self.assertNotIn(driver_supervisor.CHECKPOINT_ENV, child_environment)


if __name__ == "__main__":
    unittest.main()
