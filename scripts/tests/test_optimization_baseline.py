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
    def __init__(self, root, *, fail_one_benchmark=False, benchmark_names=None):
        self.source = root / "pinned-source"
        self.source.mkdir()
        cl_bench = self.source / "cl-bench"
        (cl_bench / "files").mkdir(parents=True)
        (cl_bench / "tests.lisp").write_text("", encoding="utf-8")
        self.fail_one_benchmark = fail_one_benchmark
        self.benchmark_names = benchmark_names or baseline.BENCHMARKS
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
                return scoreboard.ProcessResult("passed", 0, "SBCL 2.6.10\n")
            return scoreboard.ProcessResult("passed", 0, "ncl 0.1.0\n")

        if command[:2] == [sys.executable, str(baseline.DRIVER_SUPERVISOR)]:
            engine = command[command.index("--engine") + 1]
            if engine == "ncl":
                mode = command[command.index("--mode") + 1]
                call = self.ncl_counts[mode]
                self.ncl_counts[mode] += 1
                if call == 0:
                    value = 100.0
                else:
                    value = {
                        "off": [2.0, 4.0, 8.0, 4.0, 2.0],
                        "default": [1.0, 2.0, 3.0, 2.0, 1.0],
                    }[mode][call - 1]
                runtime = "ncl"
            else:
                call = self.sbcl_count
                self.sbcl_count += 1
                value = [100.0, 0.5, 1.0, 1.5, 1.0, 0.5][call]
                mode = None
                runtime = "sbcl"
        else:
            raise AssertionError(f"unexpected command: {command}")

        self.calls.append((runtime, mode, call, command, cwd, timeout))
        benchmarks = {}
        for name in self.benchmark_names:
            disabled = runtime == "sbcl" and name == baseline.DISABLED_BENCHMARK
            status = "disabled" if disabled else "passed"
            stage = "finish"
            times = [] if disabled else [value]
            condition = None
            failed = (
                self.fail_one_benchmark
                and runtime == "ncl"
                and mode == "off"
                and call == 1
                and name == "ACKERMANN"
            )
            if failed:
                status = "failed"
                stage = "execute"
                times = [999.0]
                condition = f"failed at {self.path_text} host={self.hostname} user={self.username}"
            benchmarks[name] = {
                "status": status,
                "stage": stage,
                "times": times,
                "sample_metadata": [] if disabled or failed else [{
                    "repetitions": 1,
                    "duration_seconds": value,
                    "minimum_duration_met": value >= baseline.MINIMUM_SAMPLE_SECONDS,
                }],
                "condition": condition,
                "wall_time_seconds": 0.125,
            }
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
    def _run(
        self,
        root,
        *,
        fail_one_benchmark=False,
        benchmarks=None,
        ncl_revision=None,
        ncl_revision_source=None,
        cl_bench_runs=None,
        child_timeout_seconds=baseline.DEFAULT_CL_BENCH_CHILD_TIMEOUT_SECONDS,
    ):
        benchmark_names = baseline._select_benchmarks(benchmarks)
        fake_runner = FakeRunner(
            root,
            fail_one_benchmark=fail_one_benchmark,
            benchmark_names=benchmark_names,
        )
        with patch.object(baseline, "_timed_command", side_effect=lambda command, **kwargs: (
            fake_runner(command, cwd=kwargs["cwd"], timeout=kwargs["timeout"]),
            0.125,
        )), patch.object(baseline.scoreboard, "checkout_source", return_value=fake_runner.source) as checkout, patch.object(
            baseline, "_load_average", return_value=[1.0, 2.0, 3.0]
        ):
            report = baseline.run_baseline(
                ncl=str(root / "ncl"),
                sbcl="sbcl",
                cache_dir=root / "cache",
                pairs=5,
                sbcl_samples=5,
                benchmarks=benchmarks,
                ncl_revision=ncl_revision,
                ncl_revision_source=ncl_revision_source,
                cl_bench_runs=cl_bench_runs,
                child_timeout_seconds=child_timeout_seconds,
            )
        checkout.assert_called_once_with("cl-bench", root / "cache")
        return report, fake_runner

    def test_default_sbcl_and_sample_counts_match_the_run_contract(self):
        arguments = baseline._parser().parse_args(["--ncl", "ncl"])

        self.assertEqual(baseline.SBCL_VERSION, "2.6.10")
        self.assertEqual(arguments.sbcl, "sbcl")
        self.assertIsNone(arguments.ncl_revision)
        self.assertIsNone(arguments.ncl_revision_source)
        self.assertEqual(arguments.pairs, 5)
        self.assertEqual(arguments.sbcl_samples, 5)
        self.assertIsNone(arguments.cl_bench_runs)
        self.assertEqual(arguments.child_timeout_seconds, 20.0)
        with self.assertRaises(SystemExit):
            baseline._parser().parse_args(["--ncl", "ncl", "--pairs", "4"])
        with self.assertRaises(SystemExit):
            baseline._parser().parse_args(["--ncl", "ncl", "--sbcl-samples", "4"])

    def test_supervisor_options_are_shared_by_both_runtime_commands(self):
        cwd = Path("/tmp/cl-bench")
        for engine, command in (
            ("ncl", baseline._ncl_command(
                Path("ncl"), "off", ("FIB",), cwd=cwd, runs=3,
                child_timeout_seconds=45.0,
            )),
            ("sbcl", baseline._sbcl_command(
                "sbcl", ("FIB",), cwd=cwd, runs=3,
                child_timeout_seconds=45.0,
            )),
        ):
            with self.subTest(engine=engine):
                self.assertEqual(command[:2], [sys.executable, str(baseline.DRIVER_SUPERVISOR)])
                self.assertEqual(command[command.index("--engine") + 1], engine)
                self.assertEqual(command[command.index("--runs") + 1], "3")
                self.assertEqual(
                    command[command.index("--minimum-sample-seconds") + 1],
                    str(baseline.MINIMUM_SAMPLE_SECONDS),
                )
                self.assertEqual(
                    command[command.index("--child-timeout-seconds") + 1], "45.0"
                )
                self.assertIn("--include-sample-metadata", command)
                self.assertEqual(command[-1], "FIB")

    def test_round_schedule_omits_only_the_exhausted_runtime(self):
        ncl_exhausted = baseline._round_schedule(5, 6)
        self.assertEqual(ncl_exhausted[-1], [{"engine": "sbcl", "sample": 6}])
        sbcl_exhausted = baseline._round_schedule(6, 5)
        self.assertEqual(sbcl_exhausted[-1], [
            {"engine": "ncl", "mode": "default", "pair": 6},
            {"engine": "ncl", "mode": "off", "pair": 6},
        ])

    def test_interleaved_round_order_65_statuses_and_geometric_means(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            report, runner = self._run(root)

        self.assertEqual(len(baseline.BENCHMARKS), 65)
        self.assertEqual(report["status"], "passed")
        self.assertEqual(runner.revision_commands, [[
            "jj", "log", "--ignore-working-copy", "--no-graph", "-r", "@-", "-T", "commit_id",
        ]])
        self.assertEqual(report["build_revision"]["commit"], "a" * 40)
        self.assertEqual(report["build_revision"]["source"], "committed source at @-")
        self.assertIsNone(report["protocol"]["cl_bench_runs_override"])
        self.assertEqual(
            report["protocol"]["minimum_sample_duration_policy"]["target_seconds"],
            baseline.MINIMUM_SAMPLE_SECONDS,
        )
        self.assertTrue(report["protocol"]["minimum_sample_duration_policy"]["met"])
        self.assertEqual(
            report["protocol"]["minimum_sample_duration_policy"]["sample_count"],
            report["protocol"]["minimum_sample_duration_policy"]["expected_sample_count"],
        )
        self.assertEqual(report["protocol"]["allocation_metric"]["status"], "unavailable")
        expected_cwd = (runner.source / "cl-bench").resolve()
        self.assertTrue(all(call[4] == expected_cwd for call in runner.calls))
        self.assertEqual(
            [(runtime, mode, call) for runtime, mode, call, *_ in runner.calls],
            [
                ("ncl", "off", 0),
                ("sbcl", None, 0),
                ("ncl", "default", 0),
                ("ncl", "off", 1),
                ("sbcl", None, 1),
                ("ncl", "default", 1),
                ("sbcl", None, 2),
                ("ncl", "default", 2),
                ("ncl", "off", 2),
                ("ncl", "default", 3),
                ("ncl", "off", 3),
                ("sbcl", None, 3),
                ("ncl", "off", 4),
                ("sbcl", None, 4),
                ("ncl", "default", 4),
                ("sbcl", None, 5),
                ("ncl", "default", 5),
                ("ncl", "off", 5),
            ],
        )
        measured_ncl_order = [
            run["mode"] for run in report["runs"]
            if run["phase"] == "measurement" and "mode" in run
        ]
        self.assertEqual(
            measured_ncl_order,
            ["off", "default", "default", "off", "default", "off", "off", "default", "default", "off"],
        )
        self.assertEqual(report["protocol"]["ncl_pair_order"], [
            ["off", "default"],
            ["default", "off"],
            ["default", "off"],
            ["off", "default"],
            ["default", "off"],
        ])
        observed_round_order = [
            [
                (
                    "sbcl" if run.get("mode") is None else "ncl",
                    run.get("mode"),
                    run.get("pair", run.get("sample")),
                )
                for run in report["runs"]
                if run["phase"] == "measurement"
                and run.get("pair", run.get("sample")) == round_number
            ]
            for round_number in range(1, 6)
        ]
        self.assertEqual(observed_round_order, [
            [("ncl", "off", 1), ("sbcl", None, 1), ("ncl", "default", 1)],
            [("sbcl", None, 2), ("ncl", "default", 2), ("ncl", "off", 2)],
            [("ncl", "default", 3), ("ncl", "off", 3), ("sbcl", None, 3)],
            [("ncl", "off", 4), ("sbcl", None, 4), ("ncl", "default", 4)],
            [("sbcl", None, 5), ("ncl", "default", 5), ("ncl", "off", 5)],
        ])
        self.assertEqual(
            report["protocol"]["measurement_round_order"],
            baseline._round_schedule(5, 5),
        )
        self.assertEqual(report["scores"]["common_benchmark_count"], 64)
        self.assertNotIn(baseline.DISABLED_BENCHMARK, report["scores"]["common_benchmarks"])
        self.assertNotIn(baseline.DISABLED_BENCHMARK, report["scores"]["missing_benchmarks"])
        self.assertEqual(report["scores"]["excluded_benchmark_count"], 1)
        self.assertEqual(report["scores"]["excluded_benchmarks"], [{
            "name": baseline.DISABLED_BENCHMARK,
            "reason": baseline.DISABLED_BENCHMARK_REASONS[baseline.DISABLED_BENCHMARK],
        }])
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
        self.assertEqual(
            report["scores"]["disabled_benchmarks"][baseline.DISABLED_BENCHMARK]["sbcl"][0]["reason"],
            baseline.DISABLED_BENCHMARK_REASONS[baseline.DISABLED_BENCHMARK],
        )

        for mode, expected_ratio in (("off_over_sbcl", 4.0), ("default_over_sbcl", 2.0)):
            ranking = report["scores"]["slowest_10_by_ncl_over_sbcl_ratio"][mode]
            self.assertEqual(len(ranking), 10)
            self.assertEqual(
                [item["name"] for item in ranking],
                report["scores"]["common_benchmarks"][:10],
            )
            self.assertTrue(all(
                item["ncl_over_sbcl_ratio"] == expected_ratio
                for item in ranking
            ))
        for run in report["runs"]:
            self.assertEqual(len(run["benchmarks"]), 65)
            self.assertEqual(
                {record["status"] for record in run["benchmarks"].values()} - {"passed", "disabled"},
                set(),
            )
            self.assertTrue(all(isinstance(record["times"], list) for record in run["benchmarks"].values()))
            self.assertTrue(all(
                sample["duration_seconds"]
                == run["benchmarks"][name]["times"][index]
                for name, record in run["benchmarks"].items()
                if record["status"] == "passed"
                for index, sample in enumerate(record["sample_metadata"])
            ))
            if run["phase"] == "measurement":
                self.assertTrue(all(
                    sample["load_average_process_start"] == [1.0, 2.0, 3.0]
                    and sample["load_average_process_end"] == [1.0, 2.0, 3.0]
                    for record in run["benchmarks"].values()
                    if record["status"] == "passed"
                    for sample in record["sample_metadata"]
                ))
            else:
                self.assertTrue(all(
                    sample["load_average_process_start"] is None
                    and sample["load_average_process_end"] is None
                    for record in run["benchmarks"].values()
                    for sample in record.get("sample_metadata", [])
                ))
            self.assertTrue(all(
                record["stage"] in scoreboard.CL_BENCH_STAGES
                for record in run["benchmarks"].values()
            ))
            disabled_record = run["benchmarks"][baseline.DISABLED_BENCHMARK]
            if disabled_record["status"] == "disabled":
                self.assertEqual(
                    disabled_record["reason"],
                    baseline.DISABLED_BENCHMARK_REASONS[baseline.DISABLED_BENCHMARK],
                )
            self.assertIn("status", run["process"])
            self.assertIn("stdout", run["process"])
            self.assertIn("stderr", run["process"])
            self.assertEqual(run["process"]["wall_seconds"], 0.125)
        self.assertTrue(all(call[-1] == scoreboard.DEFAULT_COMMAND_TIMEOUT for call in runner.calls))

    def test_partial_cl_bench_warning_makes_run_unclean(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            report, _ = self._run(Path(temporary_directory), benchmarks=["FIB"])

        run = report["runs"][0]
        self.assertTrue(baseline._run_is_clean(run))
        warned_run = {
            **run,
            "process": {
                **run["process"],
                "stderr": "cl-bench warning: FIB emitted a valid passing row\n",
            },
        }
        self.assertFalse(baseline._run_is_clean(warned_run))

    def test_selected_benchmarks_are_case_insensitive_and_passed_to_both_drivers(self):
        selected = ["frpoly/fixnum", "fib", "tak", "factorial", "ackermann"]
        expected = ["FRPOLY/FIXNUM", "FIB", "TAK", "FACTORIAL", "ACKERMANN"]
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            report, runner = self._run(root, benchmarks=selected)

        self.assertEqual(report["status"], "passed")
        self.assertEqual(report["protocol"]["benchmarks"], expected)
        self.assertEqual(report["scores"]["common_benchmarks"], expected)
        self.assertEqual(report["scores"]["common_benchmark_count"], 5)
        self.assertTrue(all(len(run["benchmarks"]) == 5 for run in report["runs"]))
        for call in runner.calls:
            self.assertEqual(call[3][:2], [sys.executable, str(baseline.DRIVER_SUPERVISOR)])
            self.assertEqual(call[3][call[3].index("--engine") + 1], call[0])
            self.assertEqual(call[3][call[3].index("--cwd") + 1], str(call[4]))
            self.assertEqual(
                call[3][call[3].index("--minimum-sample-seconds") + 1],
                str(baseline.MINIMUM_SAMPLE_SECONDS),
            )
            self.assertEqual(
                call[3][call[3].index("--child-timeout-seconds") + 1], "20.0"
            )
            self.assertIn("--include-sample-metadata", call[3])
            self.assertEqual(call[3][-len(expected):], expected)

    def test_runs_override_and_child_timeout_reach_both_runtime_workers(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            report, runner = self._run(
                root,
                benchmarks=["FIB"],
                cl_bench_runs=3,
                child_timeout_seconds=45.0,
            )

        self.assertEqual(report["protocol"]["cl_bench_runs_override"], 3)
        self.assertEqual(report["protocol"]["child_timeout_seconds"], 45.0)
        self.assertEqual(report["protocol"]["command_timeout_seconds"], 900.0)
        for call in runner.calls:
            self.assertEqual(call[3][call[3].index("--runs") + 1], "3")
            self.assertEqual(
                call[3][call[3].index("--child-timeout-seconds") + 1], "45.0"
            )

    def test_selected_benchmarks_reject_unknown_and_duplicate_names(self):
        for names, message in (
            (["not-a-benchmark"], "unknown benchmark"),
            (["FIB", "fib"], "duplicate benchmark"),
            ([], "at least one benchmark"),
        ):
            with self.subTest(names=names):
                with self.assertRaisesRegex(ValueError, message):
                    baseline._select_benchmarks(names)

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
        missing_ackermann = report["scores"]["missing_benchmarks"]["ACKERMANN"]["ncl_off"]
        self.assertEqual(missing_ackermann["reason"], "no complete positive sample set")
        self.assertEqual(len(missing_ackermann["errors"]), 1)
        self.assertIn("<absolute-path>", missing_ackermann["errors"][0])
        self.assertNotIn(runner.path_text, missing_ackermann["errors"][0])
        self.assertNotIn(runner.hostname, missing_ackermann["errors"][0])
        self.assertNotIn(runner.username, missing_ackermann["errors"][0])
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
        self.assertEqual(failed_run["benchmarks"]["ACKERMANN"]["stage"], "execute")
        self.assertEqual(
            failed_run["benchmarks"]["ACKERMANN"]["condition"],
            failed_run["benchmarks"]["ACKERMANN"]["error"],
        )
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

    def test_malformed_stage_record_is_kept_as_failed_benchmark(self):
        result = scoreboard.ProcessResult(
            "passed",
            0,
            json.dumps({
                "status": "passed",
                "times": [0.5],
                "benchmarks": {
                    "ACKERMANN": {
                        "status": "passed",
                        "stage": "load-file",
                        "times": [0.5],
                        "condition": None,
                    },
                },
            }),
        )

        driver_status, _, benchmarks, error, _ = baseline._driver_data(
            result,
            ("ACKERMANN",),
        )

        self.assertEqual(driver_status, "failed")
        self.assertIsNotNone(error)
        self.assertEqual(benchmarks["ACKERMANN"]["status"], "failed")
        self.assertIsNone(benchmarks["ACKERMANN"]["stage"])
        self.assertIn("stage", benchmarks["ACKERMANN"]["condition"])

    def test_load_failure_condition_and_stage_survive_driver_normalization(self):
        record = baseline._benchmark_record({
            "status": "failed",
            "stage": "load",
            "times": [],
            "condition": "TypeError: class-0-0 is not a type specifier",
        }, name="CLOS/defclass")

        self.assertEqual(record["status"], "failed")
        self.assertEqual(record["stage"], "load")
        self.assertEqual(record["condition"], "TypeError: class-0-0 is not a type specifier")
        self.assertEqual(record["error"], record["condition"])

    def test_sample_metadata_preserves_repetitions_and_requires_scalar_duration(self):
        value = {
            "status": "passed",
            "stage": "finish",
            "times": [0.05],
            "sample_metadata": [{
                "repetitions": 2,
                "duration_seconds": 0.1,
                "minimum_duration_met": True,
            }],
            "condition": None,
        }
        record = baseline._benchmark_record(value, name="FIB")

        self.assertEqual(record["status"], "passed")
        self.assertEqual(record["sample_metadata"][0]["repetitions"], 2)
        self.assertEqual(record["sample_metadata"][0]["duration_seconds"], 0.1)
        self.assertTrue(record["sample_metadata"][0]["minimum_duration_met"])
        malformed = {
            **value,
            "sample_metadata": [{
                **value["sample_metadata"][0],
                "duration_seconds": [0.1],
            }],
        }
        malformed_record = baseline._benchmark_record(malformed, name="FIB")
        self.assertEqual(malformed_record["status"], "failed")
        self.assertIn("duration_seconds", malformed_record["condition"])

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
                    ["jj", "log", "--ignore-working-copy", "--no-graph", "-r", "@-", "-T", "commit_id"],
                    cwd=baseline.REPO_ROOT,
                    timeout=baseline.INFO_TIMEOUT,
                )

    def test_explicit_ncl_revision_source_is_recorded_for_external_binary(self):
        revision = "a" * 40
        with patch.object(baseline, "_timed_command") as timed_command:
            record = baseline._revision_record(revision, "GITHUB_SHA")

        timed_command.assert_not_called()
        self.assertEqual(record, {
            "commit": revision,
            "status": "passed",
            "source": "GITHUB_SHA",
            "wall_seconds": 0.0,
        })
        with self.assertRaisesRegex(ValueError, "commit SHA"):
            baseline._revision_record("not-a-commit")

    def test_explicit_revision_source_is_present_in_the_report(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            revision = "b" * 40
            report, runner = self._run(
                root,
                benchmarks=["fib"],
                ncl_revision=revision,
                ncl_revision_source="GITHUB_SHA",
            )

        self.assertEqual(report["build_revision"]["commit"], revision)
        self.assertEqual(report["build_revision"]["source"], "GITHUB_SHA")
        self.assertEqual(runner.revision_commands, [])


if __name__ == "__main__":
    unittest.main()
