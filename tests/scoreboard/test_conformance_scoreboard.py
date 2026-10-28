import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock, patch


MODULE_PATH = Path(__file__).parents[2] / "scripts" / "conformance_scoreboard.py"
spec = importlib.util.spec_from_file_location("conformance_scoreboard", MODULE_PATH)
scoreboard = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = scoreboard
spec.loader.exec_module(scoreboard)
INCLUSION_PATH = Path(__file__).parents[2] / "scripts" / "ansi_test_inclusion.py"
inclusion_spec = importlib.util.spec_from_file_location("ansi_test_inclusion", INCLUSION_PATH)
inclusion = importlib.util.module_from_spec(inclusion_spec)
assert inclusion_spec.loader is not None
sys.modules[inclusion_spec.name] = inclusion
inclusion_spec.loader.exec_module(inclusion)


class ScoreboardTests(unittest.TestCase):
    def _run_timeout_scoreboard(self, checkpoint: bytes) -> tuple[int, dict]:
        ansi_output = json.dumps({"passed": 1, "failed": 0, "unexecuted": 0})

        def run_command(command, *, cwd, timeout, runner=scoreboard.subprocess.run, env=None):
            if command == ["ansi"]:
                return scoreboard.ProcessResult("passed", 0, ansi_output)
            self.assertEqual(command, ["bench"])
            self.assertEqual(timeout, scoreboard.DEFAULT_COMMAND_TIMEOUT)
            self.assertIsNotNone(env)
            checkpoint_path = Path(env[scoreboard.CL_BENCH_CHECKPOINT_ENV])
            self.assertTrue(checkpoint_path.parent.is_dir())
            checkpoint_path.write_bytes(checkpoint)
            return scoreboard.ProcessResult("timeout", None, stderr="timed out")

        with TemporaryDirectory() as directory:
            source = Path(directory)
            with patch.object(scoreboard, "checkout_source", return_value=source):
                with patch.object(scoreboard, "run_command", side_effect=run_command):
                    with patch.object(sys, "stdout") as stdout:
                        exit_status = scoreboard.main([
                            "--ansi-command", "ansi",
                            "--cl-bench-command", "bench",
                        ])
        report_text = "".join(call.args[0] for call in stdout.write.call_args_list)
        return exit_status, json.loads(report_text)

    def test_classifies_success_failure_timeout_and_signal_crash(self):
        self.assertEqual(scoreboard.classify_process(0), "passed")
        self.assertEqual(scoreboard.classify_process(1), "failed")
        self.assertEqual(scoreboard.classify_process(None, timed_out=True), "timeout")
        self.assertEqual(scoreboard.classify_process(-9), "crash")

    def test_run_command_timeout_is_mockable(self):
        runner = Mock(side_effect=subprocess.TimeoutExpired(["ncl"], 1, output=b"partial"))
        result = scoreboard.run_command(["ncl"], cwd=Path("."), timeout=1, runner=runner)
        self.assertEqual(result.status, "timeout")
        self.assertEqual(result.stdout, "partial")

    def test_timeout_checkpoint_keeps_completed_rows_without_a_score(self):
        records = [
            {"type": "header", "version": 1, "expected": ["ACKERMANN", "BIGNUM"]},
            {
                "type": "row",
                "name": "ACKERMANN",
                "benchmark": {
                    "status": "passed",
                    "stage": "finish",
                    "times": [0.25],
                    "condition": None,
                },
            },
        ]
        checkpoint = "".join(json.dumps(record) + "\n" for record in records).encode()

        exit_status, report = self._run_timeout_scoreboard(checkpoint)

        bench = report["cl-bench"]
        self.assertEqual(exit_status, 1)
        self.assertEqual(report["schema_version"], 2)
        self.assertEqual(bench["status"], "timeout")
        self.assertEqual(bench["driver_status"], "partial")
        self.assertEqual(
            bench["progress"],
            {"complete": False, "expected": ["ACKERMANN", "BIGNUM"], "completed": 1},
        )
        self.assertEqual(set(bench["benchmarks"]), {"ACKERMANN"})
        self.assertEqual(bench["benchmarks"]["ACKERMANN"]["times"], [0.25])
        self.assertNotIn("samples", bench)
        self.assertNotIn("geometric_mean", bench)

    def test_timeout_checkpoint_without_rows_is_explicitly_incomplete(self):
        checkpoint = json.dumps({
            "type": "header",
            "version": 1,
            "expected": ["ACKERMANN"],
        }).encode() + b"\n"

        exit_status, report = self._run_timeout_scoreboard(checkpoint)

        bench = report["cl-bench"]
        self.assertEqual(exit_status, 1)
        self.assertEqual(bench["status"], "timeout")
        self.assertEqual(bench["driver_status"], "partial")
        self.assertEqual(
            bench["progress"],
            {"complete": False, "expected": ["ACKERMANN"], "completed": 0},
        )
        self.assertEqual(bench["benchmarks"], {})
        self.assertNotIn("samples", bench)
        self.assertNotIn("geometric_mean", bench)

    def test_checkpoint_ignores_torn_tail_and_rejects_duplicate_or_unexpected_rows(self):
        header = {"type": "header", "version": 1, "expected": ["ACKERMANN"]}
        row = {
            "type": "row",
            "name": "ACKERMANN",
            "benchmark": {
                "status": "passed",
                "stage": "finish",
                "times": [0.25],
                "condition": None,
            },
        }
        with TemporaryDirectory() as directory:
            path = Path(directory) / "checkpoint.jsonl"
            path.write_text(json.dumps(header) + "\n" + json.dumps(row) + "\n{\"type\":", encoding="utf-8")
            report, progress = scoreboard.read_cl_bench_checkpoint(path)
            self.assertEqual(set(report["benchmarks"]), {"ACKERMANN"})
            self.assertEqual(progress["completed"], 1)
            self.assertTrue(progress["complete"])

            for invalid in (
                json.dumps(row) + "\n",
                json.dumps({**row, "name": "UNEXPECTED"}) + "\n",
            ):
                path.write_text(json.dumps(header) + "\n" + json.dumps(row) + "\n" + invalid, encoding="utf-8")
                with self.assertRaises(ValueError):
                    scoreboard.read_cl_bench_checkpoint(path)

    def test_geometric_mean(self):
        self.assertAlmostEqual(scoreboard.geometric_mean([1, 4, 16]), 4)
        with self.assertRaises(ValueError):
            scoreboard.geometric_mean([])
        with self.assertRaises(ValueError):
            scoreboard.geometric_mean([0.0, 1.0])

    def test_cached_checkout_uses_fixed_url_and_commit(self):
        with TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(scoreboard.subprocess, "run") as run:
                run.side_effect = [
                    Mock(stdout=""), Mock(stdout=""), Mock(stdout=""),
                    Mock(stdout=scoreboard.SOURCES["ansi-test"]["commit"] + "\n"),
                ]
                path = scoreboard.checkout_source("ansi-test", cache)
            self.assertEqual(path, cache / "ansi-test")
            commands = [call.args[0] for call in run.call_args_list]
            self.assertEqual(commands[0][0:3], ["git", "clone", scoreboard.SOURCES["ansi-test"]["url"]])
            self.assertIn(scoreboard.SOURCES["ansi-test"]["commit"], commands[2])

    def test_json_and_markdown_outputs_include_counts_mean_and_commit(self):
        ansi = scoreboard.ProcessResult("passed", 0)
        bench = scoreboard.ProcessResult("passed", 0)
        result = scoreboard.make_scoreboard(ansi, {"passed": 3, "failed": 1, "unexecuted": 2}, bench, [1, 4])
        self.assertEqual(result["ansi-test"]["failed"], 1)
        self.assertAlmostEqual(result["cl-bench"]["geometric_mean"], 2)
        markdown = scoreboard.render_markdown(result)
        self.assertIn("3 passed, 1 failed, 2 unexecuted", markdown)
        self.assertIn(scoreboard.SOURCES["cl-bench"]["commit"], markdown)

    def test_empty_benchmark_samples_are_reported_without_a_mean(self):
        ansi = scoreboard.ProcessResult("passed", 0)
        bench = scoreboard.ProcessResult("failed", 1)
        result = scoreboard.make_scoreboard(ansi, None, bench, [])
        self.assertEqual(result["cl-bench"]["samples"], 0)
        self.assertNotIn("geometric_mean", result["cl-bench"])

    def test_baseline_measurement_uses_registered_source_for_chapter(self):
        with TemporaryDirectory() as directory:
            source = Path(directory) / "ansi-test"
            source_file = source / "printer" / "format" / "format.lsp"
            source_file.parent.mkdir(parents=True)
            source_file.write_text("", encoding="utf-8")
            stdout = "\n".join((
                f"{inclusion.PROTOCOL}\tENTRY\tA.1\t{source_file}",
                f"{inclusion.PROTOCOL}\tPASS\tA.1",
                f"{inclusion.PROTOCOL}\tDONE\tbaseline",
            ))
            with patch.object(inclusion, "_run_process", return_value=("process", stdout, "", 0)):
                with patch.object(inclusion, "_runtime_version", return_value="SBCL 2.6.0"):
                    report = inclusion.measure_baseline(["sbcl", "--script"], source, 1)

        self.assertEqual(
            report["tests"],
            [{"name": "A.1", "chapter": "printer/format", "status": "passed"}],
        )
        self.assertEqual(report["registered"], 3)
        self.assertEqual(report["unexecuted"], 0)

    def test_clean_suite_scratch_removes_files_and_directory(self):
        with TemporaryDirectory() as directory:
            source = Path(directory)
            scratch = source / "sandbox" / "scratch"
            scratch.mkdir(parents=True)
            (scratch / "foo.txt").write_text("", encoding="utf-8")

            inclusion._clean_suite_scratch(source)

            self.assertFalse(scratch.exists())

    def test_baseline_writer_preserves_dotted_platform_prefix(self):
        with TemporaryDirectory() as directory:
            prefix = Path(directory) / "ansi-test-sbcl-2.6.0-macos-arm64"
            report = {
                "status": "complete",
                "unexecuted": 0,
                "runtime_version": "SBCL 2.6.0",
                "active": 1,
                "tests": [{"name": "A.1", "chapter": "arrays", "status": "passed"}],
                "measured_on": "2026-10-17",
                "platform": "macOS arm64",
                "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
                "registered": 3,
                "removed": 2,
                "process_exit_status": 0,
                "disabled": 0,
                "disabled_tests": [],
                "passed": 1,
                "failed": 0,
            }

            inclusion._write_baseline_files(report, prefix)

            names_path = Path(str(prefix) + ".txt")
            self.assertEqual(names_path.read_text(encoding="utf-8"), "A.1\n")
            self.assertEqual(inclusion.load_baseline(names_path), {"A.1": "arrays"})
            metadata = Path(str(prefix) + ".md").read_text(encoding="utf-8")
            self.assertIn("upstream `doit.lsp` says they misbehave", metadata)
            self.assertFalse(prefix.with_suffix(".txt").exists())

    def test_baseline_metadata_explains_sbcl_note_disabling(self):
        with TemporaryDirectory() as directory:
            prefix = Path(directory) / "baseline"
            report = {
                "status": "complete",
                "unexecuted": 0,
                "runtime_version": "SBCL 2.6.0",
                "active": 1,
                "tests": [{"name": "OPEN.IO.21", "chapter": "streams", "status": "passed"}],
                "measured_on": "2026-10-17",
                "platform": "macOS arm64",
                "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
                "registered": 3,
                "removed": 2,
                "process_exit_status": 0,
                "disabled": 1,
                "disabled_tests": ["OPEN.IO.30"],
                "passed": 1,
                "failed": 0,
            }

            inclusion._write_baseline_files(report, prefix)

            metadata = prefix.with_suffix(".md").read_text(encoding="utf-8")
            self.assertIn("The note assumes `OPEN` with `:if-exists :new-version` does not fail", metadata)
            self.assertIn("ansi-test disables it for SBCL", metadata)

    def test_inclusion_comparator_reports_missing_tests_by_chapter(self):
        baseline = {
            "A.1": "arrays",
            "A.2": "arrays",
            "CONS.1": "cons",
            "CONS.2": "cons",
            "SYMBOL.1": "symbols",
        }
        observed = {
            "A.1": "passed",
            "A.2": "failed",
            "CONS.1": "timeout",
            "SYMBOL.1": "harness_error",
        }

        result = inclusion.compare_results(baseline, observed)

        self.assertEqual(result["expected"], 5)
        self.assertEqual(result["passed"], 1)
        self.assertEqual(result["t_ansi_count"], 4)
        self.assertEqual(
            [(row["name"], row["status"]) for row in result["t_ansi"]],
            [
                ("A.2", "failed"),
                ("CONS.1", "timeout"),
                ("CONS.2", "unexecuted"),
                ("SYMBOL.1", "harness_error"),
            ],
        )
        self.assertEqual(
            result["by_chapter"],
            {
                "arrays": {"failed": 1},
                "cons": {"timeout": 1, "unexecuted": 1},
                "symbols": {"harness_error": 1},
            },
        )
        markdown = inclusion.render_markdown({
            "measured_on": "2026-10-17",
            "platform": "macOS arm64",
            "implementation": "ncl",
            "runtime_version": "NCL",
            "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
            "status": "complete",
            "comparison": result,
        })
        self.assertIn("`A.2` | arrays | failed", markdown)
        self.assertIn("`CONS.2` | cons | unexecuted", markdown)
        self.assertIn("`SYMBOL.1` | symbols | harness_error", markdown)
        self.assertEqual(inclusion.comparison_exit_status(result), 1)
        self.assertEqual(
            inclusion.comparison_exit_status(
                inclusion.compare_results({"A.1": "arrays"}, {"A.1": "passed"})
            ),
            0,
        )

    def test_inclusion_diagnostic_prefers_fatal_error_over_sbcl_noise(self):
        diagnostic = inclusion._diagnostic(
            "; compilation unit finished\n; caught 1 fatal ERROR condition\n"
            "WARNING:\n   Invalid qualifiers\nUnhandled SIMPLE-ERROR in thread main thread\n",
            "",
        )

        self.assertEqual(diagnostic, "Unhandled SIMPLE-ERROR in thread main thread")
        self.assertIsNone(inclusion._diagnostic("", "STYLE-WARNING: ignored warning\n"))

    def test_partial_inclusion_report_explains_unfinished_chapter_loaders(self):
        markdown = inclusion.render_markdown({
            "measured_on": "2026-10-17",
            "platform": "macOS arm64",
            "implementation": "ncl",
            "runtime_version": "NCL",
            "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
            "status": "partial",
            "comparison": {
                "expected": 1,
                "passed": 0,
                "t_ansi_count": 1,
                "t_ansi": [{"name": "A.1", "chapter": "arrays", "status": "harness_error"}],
                "by_chapter": {"arrays": {"harness_error": 1}},
            },
        })

        self.assertIn("Partial means one or more chapter loaders did not finish", markdown)
        self.assertIn("Tests under those loaders are listed as `harness_error` or `timeout`", markdown)

    def test_inclusion_loads_current_baseline_files_and_metadata(self):
        with TemporaryDirectory() as directory:
            baseline = Path(directory) / "baseline.txt"
            baseline.write_text("A.1\nCONS.1\n", encoding="utf-8")
            baseline.with_suffix(".tsv").write_text(
                "name\tchapter\nA.1\tarrays\nCONS.1\tcons\n", encoding="utf-8"
            )
            baseline.with_suffix(".md").write_text(
                "# ANSI-Test baseline\n"
                "- Measured: 2026-10-17\n"
                "- Platform: macOS arm64\n"
                "- SBCL: 2.6.0\n"
                "- ansi-test commit: `ca06bd919661af162c67407c9d994e881870bdb3`\n",
                encoding="utf-8",
            )

            self.assertEqual(
                inclusion.load_baseline(baseline),
                {"A.1": "arrays", "CONS.1": "cons"},
            )
            inclusion.validate_same_day_baseline(baseline, {
                "measured_on": "2026-10-17",
                "platform": "macOS arm64",
            })
            with self.assertRaisesRegex(ValueError, "same day"):
                inclusion.validate_same_day_baseline(baseline, {
                    "measured_on": "2026-10-18",
                    "platform": "macOS arm64",
                })

    def test_unmeasured_ansi_status_is_preserved_as_successful_measurement(self):
        ansi_output = json.dumps({
            "passed": 0,
            "failed": 0,
            "unexecuted": 0,
            "status": "unmeasured",
            "reason": "No same-day Linux baseline.",
        })
        bench_output = json.dumps({
            "status": "passed",
            "times": [0.001],
            "benchmarks": {
                "ACKERMANN": {
                    "status": "passed",
                    "stage": "finish",
                    "times": [0.001],
                    "condition": None,
                },
            },
        })
        with TemporaryDirectory() as directory:
            with patch.object(scoreboard, "checkout_source", return_value=Path(directory)):
                with patch.object(
                    scoreboard,
                    "run_command",
                    side_effect=[
                        scoreboard.ProcessResult("passed", 0, ansi_output),
                        scoreboard.ProcessResult("passed", 0, bench_output),
                    ],
                ):
                    with patch.object(sys, "stdout") as stdout:
                        status = scoreboard.main([
                            "--ansi-command", "ansi",
                            "--cl-bench-command", "bench",
                        ])
        report = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))
        self.assertEqual(status, 0)
        self.assertEqual(report["ansi-test"]["status"], "unmeasured")
        self.assertEqual(report["ansi-test"]["reason"], "No same-day Linux baseline.")
        self.assertIn("No same-day Linux baseline.", scoreboard.render_markdown(report))

    def test_main_reads_driver_json_for_success_and_failure_results(self):
        ansi_output = json.dumps({"passed": 1, "failed": 0, "unexecuted": 0})
        bench_output = json.dumps({
            "status": "failed",
            "times": [0.001, 0.002],
            "benchmarks": {
                "ACKERMANN": {
                    "status": "passed",
                    "stage": "finish",
                    "times": [0.001, 0.002],
                    "condition": None,
                },
                "CLOS/defclass": {
                    "status": "failed",
                    "stage": "load",
                    "times": [],
                    "condition": "TypeError: class-0-0 is not a type specifier",
                },
            },
        })
        with TemporaryDirectory() as directory:
            with patch.object(scoreboard, "checkout_source", return_value=Path(directory)):
                with patch.object(
                    scoreboard,
                    "run_command",
                    side_effect=[
                        scoreboard.ProcessResult("passed", 0, ansi_output),
                        scoreboard.ProcessResult("failed", 1, bench_output),
                    ],
                ):
                    with patch.object(sys, "stdout") as stdout:
                        status = scoreboard.main([
                            "--ansi-command", "ansi",
                            "--cl-bench-command", "bench",
                        ])
        self.assertEqual(status, 0)
        report = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))
        self.assertEqual(report["ansi-test"]["status"], "passed")
        self.assertEqual(report["cl-bench"]["status"], "failed")
        self.assertEqual(report["cl-bench"]["samples"], 2)
        self.assertEqual(report["cl-bench"]["driver_status"], "failed")
        self.assertEqual(
            report["cl-bench"]["benchmarks"]["CLOS/defclass"],
            {
                "status": "failed",
                "stage": "load",
                "times": [],
                "condition": "TypeError: class-0-0 is not a type specifier",
            },
        )
        self.assertEqual(
            report["cl-bench"]["benchmarks"]["ACKERMANN"]["status"],
            "passed",
        )

    def test_malformed_driver_stage_is_a_harness_failure(self):
        ansi_output = json.dumps({"passed": 1, "failed": 0, "unexecuted": 0})
        bench_output = json.dumps({
            "status": "passed",
            "times": [0.001],
            "benchmarks": {
                "ACKERMANN": {
                    "status": "passed",
                    "stage": "load-file",
                    "times": [0.001],
                    "condition": None,
                },
            },
        })
        with TemporaryDirectory() as directory:
            with patch.object(scoreboard, "checkout_source", return_value=Path(directory)):
                with patch.object(
                    scoreboard,
                    "run_command",
                    side_effect=[
                        scoreboard.ProcessResult("passed", 0, ansi_output),
                        scoreboard.ProcessResult("passed", 0, bench_output),
                    ],
                ):
                    with patch.object(sys, "stdout") as stdout:
                        status = scoreboard.main([
                            "--ansi-command", "ansi",
                            "--cl-bench-command", "bench",
                        ])
        report = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))

        self.assertEqual(status, 1)
        self.assertEqual(report["cl-bench"]["status"], "failed")
        self.assertIn("invalid stage", report["cl-bench"]["driver_error"])
        self.assertNotIn("benchmarks", report["cl-bench"])


if __name__ == "__main__":
    unittest.main()
