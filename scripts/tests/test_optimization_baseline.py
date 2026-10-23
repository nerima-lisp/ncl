import getpass
import json
import socket
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPTS_DIR = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))
import optimization_baseline as baseline
import conformance_scoreboard as scoreboard


class FakeRunner:
    def __init__(self, root, *, fail_one_benchmark=False):
        self.source = root / "pinned-source"
        self.source.mkdir()
        self.fail_one_benchmark = fail_one_benchmark
        self.calls = []
        self.revision_commands = []
        self.ncl_counts = {"off": 0, "default": 0}
        self.sbcl_count = 0
        self.path_text = str(root / "private" / "artifact.log")
        self.hostname = socket.gethostname()
        self.username = getpass.getuser()

    def __call__(self, command, *, cwd, timeout):
        command = list(command)
        if command[0] == "jj":
            self.revision_commands.append(command)
            return scoreboard.ProcessResult("passed", 0, "a" * 40 + "\n")
        if command[-1:] == ["--version"]:
            if command[0] == "sbcl":
                return scoreboard.ProcessResult("passed", 0, "SBCL 2.6.0\n")
            return scoreboard.ProcessResult("passed", 0, "ncl 0.1.0\n")

        if command[0] == str(baseline.NCL_DRIVER):
            mode = command[2]
            call = self.ncl_counts[mode]
            self.ncl_counts[mode] += 1
            if call == 0:
                value = 100.0
            else:
                value = {"off": [2.0, 4.0, 8.0], "default": [1.0, 2.0, 3.0]}[mode][call - 1]
            runtime = "ncl"
        elif command[0] == "sbcl":
            call = self.sbcl_count
            self.sbcl_count += 1
            value = [100.0, 0.5, 1.0, 1.5][call]
            mode = None
            runtime = "sbcl"
        else:
            raise AssertionError(f"unexpected command: {command}")

        self.calls.append((runtime, mode, call, command, cwd, timeout))
        benchmarks = {}
        for name in baseline.BENCHMARKS:
            disabled = runtime == "sbcl" and name == baseline.DISABLED_BENCHMARK
            status = "disabled" if disabled else "passed"
            times = [] if disabled else [value]
            error = None
            failed = (
                self.fail_one_benchmark
                and runtime == "ncl"
                and mode == "off"
                and call == 1
                and name == "ACKERMANN"
            )
            if failed:
                status = "failed"
                times = [999.0]
                error = f"failed at {self.path_text} host={self.hostname} user={self.username}"
            benchmarks[name] = {"status": status, "times": times, "error": error}
        payload = {
            "status": "failed" if any(row["status"] == "failed" for row in benchmarks.values()) else "passed",
            "times": [
                value for row in benchmarks.values()
                if row["status"] == "passed"
                for value in row["times"]
            ],
            "benchmarks": benchmarks,
        }
        stdout = (
            f"diagnostic path={self.path_text} host={self.hostname} user={self.username}\n"
            + json.dumps(payload)
            + f"\nfinished {self.path_text} host={self.hostname} user={self.username}\n"
        )
        stderr = (
            f"home={Path.home()} quoted='{self.path_text}' "
            f"file={Path(self.path_text).as_uri()} host={self.hostname} user={self.username}\n"
        )
        if failed := any(row["status"] == "failed" for row in benchmarks.values()):
            return scoreboard.ProcessResult("failed", 23, stdout, stderr)
        return scoreboard.ProcessResult("passed", 0, stdout, stderr)


class OptimizationBaselineTests(unittest.TestCase):
    def _run(self, root, *, fail_one_benchmark=False):
        fake_runner = FakeRunner(root, fail_one_benchmark=fail_one_benchmark)
        with patch.object(baseline, "_timed_command", side_effect=lambda command, **kwargs: (
            fake_runner(command, cwd=kwargs["cwd"], timeout=kwargs["timeout"]),
            0.125,
        )), patch.object(baseline.scoreboard, "checkout_source", return_value=fake_runner.source) as checkout:
            report = baseline.run_baseline(
                ncl=str(root / "ncl"),
                sbcl="sbcl",
                cache_dir=root / "cache",
            )
        checkout.assert_called_once_with("cl-bench", root / "cache")
        return report, fake_runner

    def test_warmup_pair_order_65_statuses_and_geometric_means(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            report, runner = self._run(root)

        self.assertEqual(len(baseline.BENCHMARKS), 65)
        self.assertEqual(report["status"], "passed")
        self.assertEqual(runner.revision_commands, [[
            "jj", "log", "--ignore-working-copy", "-r", "@-", "-T", "commit_id",
        ]])
        self.assertEqual(report["build_revision"]["commit"], "a" * 40)
        self.assertEqual(report["build_revision"]["source"], "committed source at @-")
        self.assertEqual(
            [(runtime, mode, call) for runtime, mode, call, *_ in runner.calls],
            [
                ("ncl", "off", 0),
                ("ncl", "default", 0),
                ("ncl", "off", 1),
                ("ncl", "default", 1),
                ("ncl", "default", 2),
                ("ncl", "off", 2),
                ("ncl", "off", 3),
                ("ncl", "default", 3),
                ("sbcl", None, 0),
                ("sbcl", None, 1),
                ("sbcl", None, 2),
                ("sbcl", None, 3),
            ],
        )
        measured_ncl_order = [
            run["mode"] for run in report["runs"]
            if run["phase"] == "measurement" and "mode" in run
        ]
        self.assertEqual(
            measured_ncl_order,
            ["off", "default", "default", "off", "off", "default"],
        )
        self.assertEqual(report["protocol"]["ncl_pair_order"], [
            ["off", "default"],
            ["default", "off"],
            ["off", "default"],
        ])
        self.assertEqual(report["scores"]["common_benchmark_count"], 64)
        self.assertNotIn(baseline.DISABLED_BENCHMARK, report["scores"]["common_benchmarks"])
        self.assertNotIn(baseline.DISABLED_BENCHMARK, report["scores"]["missing_benchmarks"])
        for runtime, expected in (("ncl_off", 4.0), ("ncl_default", 2.0), ("sbcl", 1.0)):
            self.assertAlmostEqual(
                report["scores"]["geometric_mean_seconds"][runtime], expected
            )
        for mode, expected in (("off_over_sbcl", 4.0), ("default_over_sbcl", 2.0)):
            self.assertAlmostEqual(
                report["scores"]["ncl_over_sbcl_ratio"][mode], expected
            )
        self.assertAlmostEqual(
            report["scores"]["benchmark_ncl_over_sbcl_ratio"]["off_over_sbcl"]["ACKERMANN"],
            4.0,
        )
        self.assertEqual(
            report["scores"]["disabled_benchmarks"][baseline.DISABLED_BENCHMARK]["sbcl"][0]["status"],
            "disabled",
        )
        for run in report["runs"]:
            self.assertEqual(len(run["benchmarks"]), 65)
            self.assertEqual(
                {record["status"] for record in run["benchmarks"].values()} - {"passed", "disabled"},
                set(),
            )
            self.assertTrue(all(isinstance(record["times"], list) for record in run["benchmarks"].values()))
            self.assertIn("status", run["process"])
            self.assertIn("stdout", run["process"])
            self.assertIn("stderr", run["process"])
            self.assertEqual(run["process"]["wall_seconds"], 0.125)
        self.assertTrue(all(call[-1] == scoreboard.DEFAULT_COMMAND_TIMEOUT for call in runner.calls))

    def test_failed_run_is_kept_and_machine_details_are_redacted(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            report, runner = self._run(root, fail_one_benchmark=True)
            rendered = json.dumps(report)
            output_path = root / "private-output.json"
            with patch.object(baseline, "run_baseline", return_value=report):
                exit_status = baseline.main([
                    "--ncl", str(root / "ncl"),
                    "--sbcl", "sbcl",
                    "--cache-dir", str(root / "cache"),
                    "--output", str(output_path),
                ])
            output_json = output_path.read_text(encoding="utf-8")

        self.assertEqual(exit_status, 0)
        self.assertNotIn(str(output_path), output_json)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["scores"]["common_benchmark_count"], 63)
        failed_run = next(
            run for run in report["runs"]
            if run.get("mode") == "off"
            and run.get("pair") == 1
            and run["phase"] == "measurement"
        )
        self.assertEqual(failed_run["process"]["status"], "failed")
        self.assertEqual(failed_run["process"]["return_code"], 23)
        self.assertEqual(failed_run["driver_status"], "failed")
        self.assertEqual(failed_run["benchmarks"]["ACKERMANN"]["status"], "failed")
        self.assertEqual(failed_run["benchmarks"]["ACKERMANN"]["times"], [999.0])
        self.assertNotIn("driver_error", failed_run)
        self.assertNotIn(999.0, failed_run["driver_times"])
        self.assertIn("<absolute-path>", failed_run["benchmarks"]["ACKERMANN"]["error"])
        self.assertNotIn(runner.path_text, rendered)
        self.assertNotIn(str(Path.home()), rendered)
        self.assertNotIn(runner.hostname, rendered)
        self.assertNotIn(runner.username, rendered)
        self.assertEqual(len(failed_run["benchmarks"]), 65)

    def test_unknown_driver_shape_marks_all_benchmarks_failed(self):
        result = scoreboard.ProcessResult(
            "passed",
            0,
            json.dumps({
                "status": "passed",
                "times": [],
                "benchmarks": {},
                "unexpected": True,
            }),
        )
        driver_status, _, benchmarks, error, _ = baseline._driver_data(result)

        self.assertIsNone(driver_status)
        self.assertIsNotNone(error)
        self.assertEqual(len(benchmarks), 65)
        self.assertEqual({value["status"] for value in benchmarks.values()}, {"failed"})

    def test_revision_lookup_failure_is_reported_as_committed_source_error(self):
        cases = (
            scoreboard.ProcessResult("failed", 1, "", "revision unavailable"),
            scoreboard.ProcessResult("passed", 0, "not-a-revision", ""),
        )
        for result in cases:
            with self.subTest(result=result):
                with patch.object(
                    baseline,
                    "_timed_command",
                    return_value=(result, 0.25),
                ) as timed_command:
                    with self.assertRaisesRegex(RuntimeError, r"committed build revision at @-"):
                        baseline._revision_record()

                timed_command.assert_called_once_with(
                    ["jj", "log", "--ignore-working-copy", "-r", "@-", "-T", "commit_id"],
                    cwd=baseline.REPO_ROOT,
                    timeout=baseline.INFO_TIMEOUT,
                )


if __name__ == "__main__":
    unittest.main()
