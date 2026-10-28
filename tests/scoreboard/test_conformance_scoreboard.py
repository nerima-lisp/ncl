import importlib.util
import json
import subprocess
import sys
import threading
import time
import unittest
from collections import Counter
from datetime import date
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
MERGE_PATH = Path(__file__).parents[2] / "scripts" / "merge_ansi_test_shards.py"
merge_spec = importlib.util.spec_from_file_location("merge_ansi_test_shards", MERGE_PATH)
merge = importlib.util.module_from_spec(merge_spec)
assert merge_spec.loader is not None
sys.modules[merge_spec.name] = merge
merge_spec.loader.exec_module(merge)
DRIVER_PATH = Path(__file__).parents[2] / "conformance" / "cl-bench" / "driver_supervisor.py"
driver_spec = importlib.util.spec_from_file_location("cl_bench_driver_supervisor", DRIVER_PATH)
driver = importlib.util.module_from_spec(driver_spec)
assert driver_spec.loader is not None
sys.modules[driver_spec.name] = driver
driver_spec.loader.exec_module(driver)


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

    def test_timeout_checkpoint_keeps_a_timed_out_benchmark_row(self):
        records = [
            {"type": "header", "version": 1, "expected": ["ACKERMANN"]},
            {
                "type": "row",
                "name": "ACKERMANN",
                "benchmark": {
                    "status": "timeout",
                    "stage": "execute",
                    "times": [],
                    "condition": "child exceeded its 20 second timeout",
                },
            },
        ]
        checkpoint = "".join(json.dumps(record) + "\n" for record in records).encode()

        exit_status, report = self._run_timeout_scoreboard(checkpoint)

        bench = report["cl-bench"]
        self.assertEqual(exit_status, 1)
        self.assertEqual(bench["status"], "timeout")
        self.assertEqual(bench["driver_status"], "partial")
        self.assertEqual(
            bench["progress"],
            {"complete": True, "expected": ["ACKERMANN"], "completed": 1},
        )
        self.assertEqual(
            bench["benchmarks"]["ACKERMANN"],
            {
                "status": "timeout",
                "stage": "execute",
                "times": [],
                "condition": "child exceeded its 20 second timeout",
            },
        )

    def test_driver_report_accepts_timeout_rows_without_samples(self):
        report = scoreboard.validate_cl_bench_report({
            "status": "failed",
            "times": [0.25],
            "benchmarks": {
                "ACKERMANN": {
                    "status": "passed",
                    "stage": "finish",
                    "times": [0.25],
                    "condition": None,
                },
                "BIGNUM": {
                    "status": "timeout",
                    "stage": "execute",
                    "times": [],
                    "condition": "child exceeded its 20 second timeout",
                },
            },
        })

        self.assertEqual(report["benchmarks"]["BIGNUM"]["status"], "timeout")
        self.assertEqual(report["times"], [0.25])
        with self.assertRaisesRegex(ValueError, "timed out benchmark.*sample times"):
            scoreboard.validate_cl_bench_report({
                "status": "failed",
                "times": [],
                "benchmarks": {
                    "BIGNUM": {
                        "status": "timeout",
                        "stage": "execute",
                        "times": [0.25],
                        "condition": "too slow",
                    },
                },
            })

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
        ansi_report = {
            "error": 1,
            "tests": [{"name": "A.1", "status": "error", "reason": "type error"}],
            "comparison": {"t_ansi": [{"name": "A.1", "status": "error"}]},
        }
        result = scoreboard.make_scoreboard(
            ansi, {"passed": 3, "failed": 1, "unexecuted": 2}, bench, [1, 4], ansi_report,
        )
        self.assertEqual(result["ansi-test"]["failed"], 1)
        self.assertEqual(result["ansi-test"]["error"], 1)
        self.assertEqual(result["ansi-test"]["tests"][0]["reason"], "type error")
        self.assertEqual(result["ansi-test"]["comparison"]["t_ansi"][0]["status"], "error")
        self.assertAlmostEqual(result["cl-bench"]["geometric_mean"], 2)
        markdown = scoreboard.render_markdown(result)
        self.assertIn("3 passed, 1 failed, 2 unexecuted", markdown)
        self.assertIn("type error", markdown)
        self.assertIn(scoreboard.SOURCES["cl-bench"]["commit"], markdown)

    def test_scoreboard_accepts_merged_ansi_report_directly(self):
        ansi_report = {
            "status": "complete",
            "measured_on": "2026-10-17",
            "passed": 0,
            "failed": 0,
            "error": 1,
            "timeout": 0,
            "harness_error": 0,
            "unexecuted": 0,
            "tests": [{
                "name": "A.1", "chapter": "arrays", "status": "error",
                "reason": "ReadEvalUnavailable", "process_exit_status": 1,
            }],
            "comparison": {
                "expected": 1, "passed": 0, "t_ansi_count": 1,
                "t_ansi": [{"name": "A.1", "chapter": "arrays", "status": "error", "reason": "ReadEvalUnavailable"}],
                "by_chapter": {"arrays": {"error": 1}},
            },
        }
        with TemporaryDirectory() as directory:
            report_path = Path(directory) / "ansi.json"
            report_path.write_text(json.dumps(ansi_report), encoding="utf-8")
            with patch.object(scoreboard, "checkout_source", return_value=Path(directory)) as checkout:
                with patch.object(
                    scoreboard, "run_command",
                    return_value=scoreboard.ProcessResult(
                        "passed", 0, json.dumps({
                            "status": "passed",
                            "times": [1.0],
                            "benchmarks": {
                                "ACKERMANN": {
                                    "status": "passed",
                                    "stage": "finish",
                                    "times": [1.0],
                                    "condition": None,
                                },
                            },
                        }),
                    ),
                ):
                    with patch.object(sys, "stdout") as stdout:
                        status = scoreboard.main([
                            "--ansi-report", str(report_path), "--cl-bench-command", "bench",
                        ])

        result = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))
        self.assertEqual(status, 0)
        self.assertEqual(checkout.call_args.args[0], "cl-bench")
        self.assertEqual(result["measured_on"], "2026-10-17")
        self.assertEqual(result["ansi-test"]["status"], "failed")
        self.assertEqual(result["ansi-test"]["tests"][0]["process_exit_status"], 1)
        self.assertEqual(result["ansi-test"]["comparison"]["t_ansi_count"], 1)

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

            def run_inventory(command, inventory_source, runner_text, *, timeout):
                del command, runner_text, timeout
                stdout = "\n".join((
                    f"{inclusion.PROTOCOL}\tBEGIN\tinventoried",
                    f"{inclusion.PROTOCOL}\tENTRY\tA.1\t{inventory_source / 'printer' / 'format' / 'format.lsp'}",
                    f"{inclusion.PROTOCOL}\tDONE\tinventoried",
                ))
                return "process", stdout, "", 0

            with patch.object(inclusion, "_run_process", side_effect=run_inventory):
                with patch.object(
                    inclusion, "_run_test",
                    return_value={"name": "A.1", "chapter": "printer/format", "status": "passed", "process_exit_status": 0},
                ):
                    with patch.object(inclusion, "_runtime_version", return_value="SBCL 2.6.0"):
                        report = inclusion.measure_baseline(["sbcl", "--script"], source, 1)

        self.assertEqual(
            report["tests"],
            [{"name": "A.1", "chapter": "printer/format", "status": "passed", "process_exit_status": 0}],
        )
        self.assertEqual(report["registered"], 3)
        self.assertEqual(report["unexecuted"], 0)
        self.assertEqual(report["active_total"], 1)
        self.assertEqual(report["manifest"], [{"name": "A.1", "chapter": "printer/format"}])

    def test_shard_assignment_is_deterministic_and_covers_each_test_once(self):
        names = [f"ANSI.TEST.{index}" for index in range(200)]
        shards = [
            {name for name in names if inclusion.shard_for_test(name, 32) == shard}
            for shard in range(32)
        ]

        self.assertEqual(inclusion.shard_for_test("ARRAYS.1", 32), inclusion.shard_for_test("ARRAYS.1", 32))
        self.assertEqual(set().union(*shards), set(names))
        self.assertEqual(sum(map(len, shards)), len(names))
        with self.assertRaisesRegex(ValueError, "positive"):
            inclusion.shard_for_test("ARRAYS.1", 0)

    def test_test_measurement_selects_only_its_stable_shard(self):
        expected = {f"T.{index}": "arrays" for index in range(20)}
        shard_index, shard_count = 3, 7
        selected = sorted(
            name for name in expected
            if inclusion.shard_for_test(name, shard_count) == shard_index
        )
        with TemporaryDirectory() as directory:
            with patch.object(
                inclusion, "_run_test",
                side_effect=lambda command, source, name, chapter, timeout: {
                    "name": name, "chapter": chapter, "status": "passed",
                    "process_exit_status": 0,
                },
            ) as run_test:
                with patch.object(inclusion, "_runtime_version", return_value="NCL"):
                    report = inclusion.measure_tests(
                        ["ncl"], Path(directory), expected, 1, "ncl",
                        shard_index=shard_index, shard_count=shard_count,
                    )

        self.assertEqual(run_test.call_count, len(selected))
        self.assertEqual([row["name"] for row in report["tests"]], selected)
        self.assertEqual(report["shard_index"], shard_index)
        self.assertEqual(report["shard_count"], shard_count)

    def _write_shard_fixtures(self, root: Path, shard_count: int = 2):
        names = [f"T.{index}" for index in range(20)]
        manifest = [
            {"name": name, "chapter": "arrays" if index % 2 else "symbols"}
            for index, name in enumerate(names)
        ]
        manifest.sort(key=lambda row: (row["chapter"], row["name"]))
        manifest_map = {row["name"]: row["chapter"] for row in manifest}
        baseline_rows = {}
        for index in range(shard_count):
            baseline_rows[index] = [
                {
                    "name": name,
                    "chapter": chapter,
                    "status": "failed" if name == "T.19" else "passed",
                    "process_exit_status": 0,
                    **({"reason": "SBCL assertion"} if name == "T.19" else {}),
                }
                for name, chapter in manifest_map.items()
                if inclusion.shard_for_test(name, shard_count) == index
            ]
        baseline_passes = {
            name: chapter for name, chapter in manifest_map.items()
            if name != "T.19"
        }
        for index in range(shard_count):
            artifact = root / f"ansi-shard-{index}"
            artifact.mkdir(parents=True)
            sbcl_rows = baseline_rows[index]
            sbcl_counts = Counter(row["status"] for row in sbcl_rows)
            sbcl_report = {
                "schema_version": 1, "suite": "ansi-test", "implementation": "sbcl",
                "runtime_version": "SBCL 2.6.0", "platform": inclusion._platform_name(),
                "measured_on": date.today().isoformat(),
                "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
                "registered": len(manifest) + len(inclusion.REMOVED_TEST_NAMES),
                "active": len(sbcl_rows), "active_total": len(manifest), "disabled": 0,
                "disabled_tests": [], "removed": len(inclusion.REMOVED_TEST_NAMES),
                "status": "complete", "tests": sbcl_rows,
                "manifest": manifest, "process_exit_status": 0,
                "unexecuted": sbcl_counts["unexecuted"], "shard_index": index,
                "shard_count": shard_count,
                **{status: sbcl_counts[status] for status in merge.COUNTED_STATUSES},
            }
            ncl_expected = {
                name: chapter for name, chapter in baseline_passes.items()
                if inclusion.shard_for_test(name, shard_count) == index
            }
            ncl_rows = [
                {
                    "name": name, "chapter": chapter,
                    "status": "error" if name == "T.0" else "passed",
                    "process_exit_status": -11 if name == "T.0" else 0,
                    **({"reason": "segmentation fault"} if name == "T.0" else {}),
                }
                for name, chapter in ncl_expected.items()
            ]
            ncl_counts = Counter(row["status"] for row in ncl_rows)
            ncl_report = {
                "schema_version": 1, "suite": "ansi-test", "implementation": "ncl",
                "runtime_version": "NCL", "platform": inclusion._platform_name(),
                "measured_on": date.today().isoformat(),
                "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
                "registered": len(ncl_rows), "active": len(ncl_rows),
                "status": "complete", "tests": ncl_rows,
                "shard_index": index, "shard_count": shard_count,
                "unexecuted": ncl_counts["unexecuted"],
                **{status: ncl_counts[status] for status in merge.COUNTED_STATUSES},
                "comparison": inclusion.compare_results(ncl_expected, {row["name"]: row for row in ncl_rows}),
            }
            (artifact / "sbcl-report.json").write_text(json.dumps(sbcl_report), encoding="utf-8")
            (artifact / "ncl-report.json").write_text(json.dumps(ncl_report), encoding="utf-8")
            (artifact / "ncl-exit-status").write_text(
                str(inclusion.comparison_exit_status(ncl_report["comparison"])), encoding="utf-8",
            )
        return baseline_passes

    def test_merge_shards_writes_complete_baseline_and_comparison_reports(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            expected = self._write_shard_fixtures(root)
            baseline_prefix = root / "baseline" / "ansi-test-sbcl-2.6.0-linux-x86_64"
            baseline_json = root / "baseline-report.json"
            comparison_json = root / "ncl-report.json"

            baseline, comparison = merge.merge_shards(
                root, 2, baseline_prefix, baseline_json, comparison_json,
            )
            self.assertEqual(inclusion.load_baseline(Path(str(baseline_prefix) + ".txt")), expected)
            self.assertEqual(baseline["active_total"], 20)
            self.assertEqual(baseline["failed"], 1)
            self.assertEqual(comparison["comparison"]["expected"], len(expected))
            self.assertEqual(comparison["comparison"]["t_ansi_count"], 1)
            error_row = next(row for row in comparison["tests"] if row["name"] == "T.0")
            self.assertEqual(error_row["reason"], "segmentation fault")
            self.assertEqual(error_row["process_exit_status"], -11)
            self.assertEqual(json.loads(baseline_json.read_text(encoding="utf-8"))["registered"], 22)
            self.assertEqual(json.loads(comparison_json.read_text(encoding="utf-8"))["passed"], len(expected) - 1)

    def test_merge_shards_rejects_missing_source_date_and_platform_mismatches(self):
        mutations = (
            ("source commit", lambda report: report.update(source_commit="wrong")),
            ("measurement date", lambda report: report.update(measured_on="1900-01-01")),
            ("platform", lambda report: report.update(platform="different platform")),
        )
        for expected_message, mutate in mutations:
            with self.subTest(expected_message=expected_message), TemporaryDirectory() as directory:
                root = Path(directory)
                self._write_shard_fixtures(root)
                report_path = root / "ansi-shard-1" / "sbcl-report.json"
                report = json.loads(report_path.read_text(encoding="utf-8"))
                mutate(report)
                report_path.write_text(json.dumps(report), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, expected_message):
                    merge.merge_shards(
                        root, 2, root / "baseline", root / "baseline.json", root / "comparison.json",
                    )

    def test_merge_shards_rejects_baseline_shard_without_passes(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            self._write_shard_fixtures(root)
            report_path = root / "ansi-shard-0" / "sbcl-report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            for index, row in enumerate(report["tests"]):
                row["status"] = "timeout" if index % 2 == 0 else "error"
            counts = Counter(row["status"] for row in report["tests"])
            report.update({status: counts[status] for status in merge.COUNTED_STATUSES})
            report_path.write_text(json.dumps(report), encoding="utf-8")

            baseline_prefix = root / "baseline"
            with self.assertRaisesRegex(ValueError, "no passed tests"):
                merge.merge_shards(
                    root, 2, baseline_prefix, root / "baseline.json", root / "comparison.json",
                )
            self.assertFalse(Path(str(baseline_prefix) + ".txt").exists())
            self.assertFalse((root / "baseline.json").exists())
            self.assertFalse((root / "comparison.json").exists())

    def test_merge_shards_rejects_missing_and_duplicate_rows(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            self._write_shard_fixtures(root)
            (root / "ansi-shard-1" / "ncl-report.json").unlink()
            with self.assertRaises(FileNotFoundError):
                merge.merge_shards(
                    root, 2, root / "baseline", root / "baseline.json", root / "comparison.json",
                )

        with TemporaryDirectory() as directory:
            root = Path(directory)
            self._write_shard_fixtures(root)
            report_path = root / "ansi-shard-0" / "sbcl-report.json"
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["tests"].append(report["tests"][0])
            report["active"] += 1
            report_path.write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "duplicate"):
                merge.merge_shards(
                    root, 2, root / "baseline", root / "baseline.json", root / "comparison.json",
                )

    def test_merge_shards_checks_comparison_exit_status_before_writing_baseline(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            self._write_shard_fixtures(root)
            error_shard = inclusion.shard_for_test("T.0", 2)
            (root / f"ansi-shard-{error_shard}" / "ncl-exit-status").write_text("0\n", encoding="utf-8")
            baseline_prefix = root / "baseline"
            with self.assertRaisesRegex(ValueError, "process exit status"):
                merge.merge_shards(
                    root, 2, baseline_prefix, root / "baseline.json", root / "comparison.json",
                )
            self.assertFalse(Path(str(baseline_prefix) + ".txt").exists())

    def test_parallel_workers_use_clean_copies_and_return_sorted_rows(self):
        with TemporaryDirectory() as directory:
            source = Path(directory) / "ansi-test"
            (source / "sandbox" / "scratch").mkdir(parents=True)
            (source / "sandbox" / "scratch" / "leftover.txt").write_text("stale", encoding="utf-8")
            (source / "arrays").mkdir()
            (source / "arrays" / "stale.fasl").write_bytes(b"compiled")
            (source / "init.lsp").write_text("", encoding="utf-8")
            tests = [
                ("Z.1", "zeta"),
                ("B.2", "beta"),
                ("A.2", "alpha"),
                ("A.1", "alpha"),
            ]
            source_root = source.resolve()
            barrier = threading.Barrier(2)
            seen = []
            seen_lock = threading.Lock()

            def run_test(command, worker_source, name, chapter, timeout):
                del command, timeout
                clean_copy = (
                    worker_source.resolve() != source_root
                    and (worker_source / "init.lsp").is_file()
                    and not (worker_source / "arrays" / "stale.fasl").exists()
                    and not (worker_source / "sandbox" / "scratch" / "leftover.txt").exists()
                )
                (worker_source / "sandbox" / "scratch").mkdir(parents=True, exist_ok=True)
                (worker_source / "sandbox" / "scratch" / f"{name}.txt").write_text(
                    name, encoding="utf-8",
                )
                with seen_lock:
                    seen.append((worker_source.resolve(), name, clean_copy))
                barrier.wait(timeout=2)
                return {"name": name, "chapter": chapter, "status": "passed"}

            with patch.object(inclusion, "_run_test", side_effect=run_test):
                rows = inclusion._run_test_rows(["ncl"], source, tests, 1, 2)

        worker_roots = {worker_source for worker_source, _, _ in seen}
        self.assertEqual(len(worker_roots), 2)
        self.assertEqual(len(seen), len(tests))
        self.assertTrue(all(clean_copy for _, _, clean_copy in seen))
        self.assertEqual(
            [row["name"] for row in rows],
            ["A.1", "A.2", "B.2", "Z.1"],
        )

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
                "error": 0,
                "timeout": 0,
                "harness_error": 0,
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
                "error": 0,
                "timeout": 0,
                "harness_error": 0,
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
            "A.2": {"status": "failed", "reason": "assertion failed"},
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
        self.assertIn("assertion failed", markdown)
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

    def test_inclusion_report_lists_per_test_reason(self):
        markdown = inclusion.render_markdown({
            "measured_on": "2026-10-17",
            "platform": "macOS arm64",
            "implementation": "ncl",
            "runtime_version": "NCL",
            "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
            "status": "complete",
            "comparison": {
                "expected": 1,
                "passed": 0,
                "t_ansi_count": 1,
                "t_ansi": [{"name": "A.1", "chapter": "arrays", "status": "error", "reason": "type error"}],
                "by_chapter": {"arrays": {"error": 1}},
            },
        })

        self.assertIn("| `A.1` | arrays | error | type error |", markdown)

    def test_test_runner_escapes_name_and_requires_explicit_completion(self):
        name = 'A."1'
        runner = inclusion._test_runner(name)

        self.assertIn(
            f'(find-symbol {inclusion._lisp_string(name)} "CL-TEST")',
            runner,
        )
        self.assertIn("regression-test:do-test name :catch-errors nil", runner)
        self.assertIn(f"{inclusion.PROTOCOL}~CDONE~C~A", runner)

    def test_one_test_error_does_not_stop_later_tests_and_keeps_reasons(self):
        first = "\n".join((
            f"{inclusion.PROTOCOL}\tBEGIN\tA.1",
            f"{inclusion.PROTOCOL}\tERROR\tA.1",
            f"{inclusion.PROTOCOL}\tDONE\tA.1",
        ))
        crashed = f"{inclusion.PROTOCOL}\tBEGIN\tA.2\n"
        third = "\n".join((
            f"{inclusion.PROTOCOL}\tBEGIN\tA.3",
            f"{inclusion.PROTOCOL}\tPASS\tA.3",
            f"{inclusion.PROTOCOL}\tDONE\tA.3",
        ))
        process_output = {"A.1": ("process", first, "SIMPLE-ERROR: failed test\n", 0)}
        process_output["A.2"] = ("process", crashed, "Segmentation fault\n", -11)
        process_output["A.3"] = ("process", third, "", 0)

        def run_test_process(command, source, runner_text, *, timeout):
            del command, source, timeout
            name = next(name for name in process_output if f'"{name}"' in runner_text)
            return process_output[name]

        with TemporaryDirectory() as directory:
            with patch.object(inclusion, "_run_process", side_effect=run_test_process) as run_process:
                with patch.object(inclusion, "_runtime_version", return_value="NCL"):
                    report = inclusion.measure_tests(
                        ["ncl", "--load"], Path(directory),
                        {"A.1": "arrays", "A.2": "arrays", "A.3": "arrays"}, 1, "ncl",
                        jobs=2,
                    )

        self.assertEqual(run_process.call_count, 3)
        self.assertEqual([row["status"] for row in report["tests"]], ["error", "error", "passed"])
        self.assertEqual(report["tests"][0]["reason"], "SIMPLE-ERROR: failed test")
        self.assertEqual(report["tests"][1]["reason"], "Segmentation fault")
        self.assertEqual(report["tests"][1]["process_exit_status"], -11)
        self.assertEqual(report["passed"], 1)
        self.assertEqual(report["error"], 2)

    def test_suite_initialization_error_is_kept_on_the_selected_test(self):
        with TemporaryDirectory() as directory:
            with patch.object(
                inclusion, "_run_process",
                return_value=("process", "", "ReadEvalUnavailable while loading universe.lsp\n", 1),
            ):
                with patch.object(inclusion, "_runtime_version", return_value="NCL"):
                    report = inclusion.measure_tests(
                        ["ncl", "--load"], Path(directory),
                        {"ARRAYS.1": "universe"}, 1, "ncl",
                    )

        self.assertEqual(report["tests"][0]["status"], "error")
        self.assertEqual(report["tests"][0]["reason"], "ReadEvalUnavailable while loading universe.lsp")
        self.assertEqual(report["tests"][0]["process_exit_status"], 1)

    def test_compare_test_option_selects_one_baseline_test(self):
        with TemporaryDirectory() as directory:
            baseline = Path(directory) / "baseline.txt"
            baseline.write_text("A.1\nA.2\n", encoding="utf-8")
            baseline.with_suffix(".tsv").write_text(
                "name\tchapter\nA.1\tarrays\nA.2\tarrays\n", encoding="utf-8",
            )
            baseline.with_suffix(".md").write_text(
                f"- Measured: {inclusion.date.today().isoformat()}\n"
                f"- Platform: {inclusion._platform_name()}\n"
                "- SBCL: 2.6.0\n"
                f"- ansi-test commit: `{inclusion.SOURCES['ansi-test']['commit']}`\n",
                encoding="utf-8",
            )
            with patch.object(inclusion, "_load_source", return_value=Path(directory)):
                with patch.object(inclusion, "_runtime_version", return_value="NCL"):
                    stdout_records = "\n".join((
                        f"{inclusion.PROTOCOL}\tBEGIN\tA.1",
                        f"{inclusion.PROTOCOL}\tPASS\tA.1",
                        f"{inclusion.PROTOCOL}\tDONE\tA.1",
                    ))
                    with patch.object(
                        inclusion, "_run_process",
                        return_value=("process", stdout_records, "", 0),
                    ) as run_process:
                        with patch.object(sys, "stdout") as stdout:
                            status = inclusion.main([
                                "compare", "--command", "ncl --load", "--baseline", str(baseline),
                                "--test", "A.1", "--jobs", "2", "--cache-dir", directory,
                            ])

        report = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))
        self.assertEqual(status, 0)
        self.assertEqual(run_process.call_count, 1)
        self.assertEqual(report["selected_test"], "A.1")
        self.assertEqual(report["comparison"]["expected"], 1)
        self.assertEqual([row["name"] for row in report["tests"]], ["A.1"])

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


class ClBenchSupervisorTests(unittest.TestCase):
    def _run_workers(
        self,
        names,
        run_child,
        *,
        registered=None,
        failures=None,
        timed_out_sources=None,
        checkpoint=None,
    ):
        with patch.object(
            driver,
            "_command",
            side_effect=lambda **arguments: list(arguments["selection"]),
        ):
            with patch.object(driver, "_run_child", side_effect=run_child):
                return driver._run_benchmark_workers(
                    engine="ncl",
                    runner="ncl",
                    mode="default",
                    driver=Path("driver.lisp"),
                    selected_rows=tuple(names),
                    registered=registered or {name: () for name in names},
                    failures=failures or {},
                    timed_out_sources=timed_out_sources or set(),
                    cwd=Path("."),
                    checkpoint=checkpoint,
                )

    def _passed_child(self, command, name=None):
        benchmark = name or command[0]
        row = {
            "status": "passed",
            "stage": "finish",
            "times": [1.0],
            "condition": None,
        }
        return subprocess.CompletedProcess(
            command,
            0,
            json.dumps({"status": "passed", "times": [1.0], "benchmarks": {benchmark: row}}),
            "",
        )

    def test_child_timeout_kills_process_group_and_waits_for_communicate(self):
        child = Mock(pid=42, returncode=None)
        child.communicate.side_effect = [
            subprocess.TimeoutExpired(["ncl"], 0.01),
            ("partial stdout", "partial stderr"),
        ]
        with patch.object(driver.subprocess, "Popen", return_value=child) as popen:
            with patch.object(driver.os, "killpg") as killpg:
                result = driver._run_child(["ncl"], Path("."), timeout=0.01)

        self.assertIsNone(result.returncode)
        self.assertEqual(result.stdout, "partial stdout")
        self.assertEqual(result.stderr, "partial stderr")
        self.assertEqual(child.communicate.call_count, 2)
        self.assertEqual(popen.call_args.kwargs["start_new_session"], driver.os.name == "posix")
        killpg.assert_called_once_with(42, driver.signal.SIGKILL)

    def test_default_child_timeout_is_the_budgeted_limit(self):
        child = Mock(pid=42, returncode=0)
        child.communicate.return_value = ("", "")
        with patch.object(driver.subprocess, "Popen", return_value=child):
            driver._run_child(["ncl"], Path("."))

        child.communicate.assert_called_once_with(timeout=driver.CHILD_TIMEOUT_SECONDS)

    def test_child_timeout_budget_reserves_workflow_overhead(self):
        child_limit = driver.CHILD_TIMEOUT_SECONDS
        child_count = len(driver.SOURCE_FILES) + 65

        self.assertEqual(len(driver.SOURCE_FILES), 13)
        self.assertEqual(child_limit, 20)
        self.assertEqual(child_count, 78)
        self.assertEqual(child_count * child_limit, 1560)
        self.assertEqual(1800 - child_count * child_limit, 240)

    def test_parallel_workers_aggregate_rows_in_selection_order(self):
        names = ("FIRST", "SECOND", "THIRD", "FOURTH")
        lock = threading.Lock()
        first_two_started = threading.Event()
        active = 0
        peak = 0

        def run_child(command, cwd):
            nonlocal active, peak
            with lock:
                active += 1
                peak = max(peak, active)
                if active >= 2:
                    first_two_started.set()
            try:
                if not first_two_started.wait(timeout=2):
                    raise AssertionError("benchmark workers did not overlap")
                time.sleep(0.01)
                return self._passed_child(command)
            finally:
                with lock:
                    active -= 1

        result = self._run_workers(names, run_child)

        self.assertGreaterEqual(peak, 2)
        self.assertEqual(tuple(result["benchmarks"]), names)
        self.assertEqual(set(result["benchmarks"]), set(names))

    def test_shared_output_benchmarks_are_serialized(self):
        names = ("COMPILER", "LOAD-FASL", "FPRINT/PRETTY", "FPRINT/UGLY")
        lock = threading.Lock()
        active = 0
        peak = 0

        def run_child(command, cwd):
            nonlocal active, peak
            with lock:
                active += 1
                peak = max(peak, active)
            try:
                time.sleep(0.01)
                return self._passed_child(command)
            finally:
                with lock:
                    active -= 1

        result = self._run_workers(names, run_child)

        self.assertEqual(peak, 1)
        self.assertEqual(tuple(result["benchmarks"]), names)

    def test_benchmark_child_timeout_retains_id_and_stage(self):
        names = ("ACKERMANN",)
        process = subprocess.CompletedProcess(
            ["ACKERMANN"], None, "", "CL-BENCH-STAGE execute 1\npartial output"
        )

        result = self._run_workers(names, lambda command, cwd: process)

        self.assertEqual(result["status"], "failed")
        self.assertEqual(
            result["benchmarks"]["ACKERMANN"],
            {
                "status": "timeout",
                "stage": "execute",
                "times": [],
                "condition": "child exceeded its 20 second timeout: partial output",
            },
        )

    def test_source_preflight_timeout_becomes_timeout_rows_at_load(self):
        names = ("ACKERMANN", "BIGNUM")
        with patch.object(driver, "_source_owner", return_value="arrays.lisp"):
            result = self._run_workers(
                names,
                lambda command, cwd: self.fail("timed-out source must not launch a benchmark child"),
                registered={name: (name.lower(),) for name in names},
                failures={"arrays.lisp": "child exceeded its 20 second timeout"},
                timed_out_sources={"arrays.lisp"},
            )

        self.assertEqual(result["status"], "failed")
        self.assertEqual(
            tuple((row["status"], row["stage"]) for row in result["benchmarks"].values()),
            (("timeout", "load"), ("timeout", "load")),
        )

    def test_source_preflight_children_are_bounded_and_timeout_rows_are_emitted(self):
        registered = {"ACKERMANN": ("ackermann",)}
        preflight_results = [
            subprocess.CompletedProcess(["source"], None, "", ""),
            *[
                subprocess.CompletedProcess(["source"], 0, "", "")
                for _ in driver.SOURCE_FILES[1:]
            ],
        ]
        with patch.object(driver, "_registered_functions", return_value=registered):
            with patch.object(driver, "_command", return_value=["source"]):
                with patch.object(driver, "_source_owner", return_value=driver.SOURCE_FILES[0]):
                    with patch.object(driver, "_run_child", side_effect=preflight_results) as run_child:
                        with patch.object(sys, "stdout") as stdout:
                            exit_status = driver._run_supervisor(
                                engine="ncl",
                                runner="ncl",
                                mode="default",
                                driver=Path("driver.lisp"),
                                selection=("ACKERMANN",),
                                cwd=Path("."),
                                checkpoint=None,
                            )

        report = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))
        self.assertEqual(exit_status, 1)
        self.assertEqual(run_child.call_count, len(driver.SOURCE_FILES))
        self.assertEqual(len(driver.SOURCE_FILES), 13)
        self.assertEqual(
            report["benchmarks"]["ACKERMANN"],
            {
                "status": "timeout",
                "stage": "load",
                "times": [],
                "condition": "child exceeded its 20 second timeout",
            },
        )

    def test_supervisor_emits_all_65_rows_in_input_order_with_timeout_rows(self):
        names = driver._comparison_benchmarks()
        self.assertEqual(len(names), 65)
        timeout_names = {names[index] for index in range(0, len(names), 9)}
        exception_name = names[1]
        registered = {name: () for name in names}

        def command(**arguments):
            if arguments.get("preflight_file") is not None:
                return ["source"]
            return ["benchmark", arguments["selection"][0]]

        def run_child(child_command, cwd):
            if child_command[0] == "source":
                return subprocess.CompletedProcess(child_command, 0, "", "")
            name = child_command[1]
            if name in timeout_names:
                return subprocess.CompletedProcess(
                    child_command, None, "", "CL-BENCH-STAGE execute 1\n"
                )
            if name == exception_name:
                raise RuntimeError("injected worker failure")
            return self._passed_child(child_command, name)

        with TemporaryDirectory() as directory:
            checkpoint_path = Path(directory) / "checkpoint.jsonl"
            with checkpoint_path.open("x", encoding="utf-8") as checkpoint:
                with patch.object(driver, "_registered_functions", return_value=registered):
                    with patch.object(driver, "_command", side_effect=command):
                        with patch.object(driver, "_run_child", side_effect=run_child):
                            with patch.object(sys, "stdout") as stdout:
                                exit_status = driver._run_supervisor(
                                    engine="ncl",
                                    runner="ncl",
                                    mode="default",
                                    driver=Path("driver.lisp"),
                                    selection=names,
                                    cwd=Path("."),
                                    checkpoint=checkpoint,
                                )
            checkpoint_report, progress = scoreboard.read_cl_bench_checkpoint(checkpoint_path)

        report = json.loads("".join(call.args[0] for call in stdout.write.call_args_list))
        self.assertEqual(exit_status, 1)
        self.assertEqual(list(report["benchmarks"]), list(names))
        self.assertEqual(len(report["benchmarks"]), 65)
        self.assertEqual(
            {name for name, row in report["benchmarks"].items() if row["status"] == "timeout"},
            timeout_names,
        )
        self.assertTrue(all(report["benchmarks"][name]["stage"] == "execute" for name in timeout_names))
        self.assertEqual(report["benchmarks"][exception_name]["status"], "failed")
        self.assertEqual(report["benchmarks"][exception_name]["stage"], "load")
        self.assertIn("RuntimeError: injected worker failure", report["benchmarks"][exception_name]["condition"])
        self.assertEqual(set(checkpoint_report["benchmarks"]), set(names))
        self.assertEqual(progress, {"complete": True, "expected": list(names), "completed": 65})
        self.assertEqual(
            checkpoint_report["benchmarks"][exception_name],
            report["benchmarks"][exception_name],
        )


if __name__ == "__main__":
    unittest.main()
