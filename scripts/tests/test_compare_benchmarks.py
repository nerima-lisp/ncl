from __future__ import annotations

from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))

from compare_benchmarks import ComparisonError, run_comparison


class FakeBenchmark:
    def __init__(self, values: dict[str, list[float]]) -> None:
        self.values = values
        self.calls: list[tuple[str, str, str]] = []
        self.sample_runs: list[int] = []
        self.indices = {name: 0 for name in values}

    def __call__(self, ncl, sbcl, mode, **_kwargs):
        side = Path(ncl).name
        self.calls.append((side, str(sbcl), mode))
        self.sample_runs.append(_kwargs["sample_runs"])
        index = self.indices[side]
        self.indices[side] += 1
        seconds = self.values[side][index]
        return {
            "status": "complete",
            "corpus_file_count": 2,
            "runtimes": {
                "ncl": {
                    "file_count": 2,
                    "compiled_file_count": 2,
                    "failed_file_count": 0,
                    "files": [
                        {"suite": "synthetic", "file": "one.lisp", "status": "passed", "compile_seconds": seconds},
                        {"suite": "synthetic", "file": "two.lisp", "status": "passed", "compile_seconds": seconds},
                    ]
                }
            },
        }


class CompareBenchmarksTests(unittest.TestCase):
    def test_identical_binary_results_are_interleaved_and_not_a_regression(self):
        runner = FakeBenchmark({"baseline": [10.0] * 5, "candidate": [10.0] * 5})
        result = run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=runner)
        self.assertEqual(result["status"], "no-regression")
        self.assertFalse(result["checks"]["regression"])
        self.assertEqual(result["runs_per_binary"], 5)
        self.assertEqual(result["sample_runs_per_compile_trial"], 1)
        self.assertEqual(runner.sample_runs, [1] * 10)
        self.assertEqual(result["compared_file_count"], 2)
        self.assertEqual(
            [call[0] for call in runner.calls],
            ["baseline", "candidate", "candidate", "baseline", "baseline", "candidate", "candidate", "baseline", "baseline", "candidate"],
        )

    def test_regression_requires_both_median_and_minimum_to_be_ten_percent_slower(self):
        runner = FakeBenchmark({"baseline": [10.0] * 5, "candidate": [11.1] * 5})
        result = run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=runner)
        self.assertEqual(result["status"], "regression")
        self.assertTrue(result["checks"]["median_at_least_threshold_slower"])
        self.assertTrue(result["checks"]["minimum_at_least_threshold_slower"])

    def test_a_fast_minimum_prevents_a_regression_result(self):
        runner = FakeBenchmark({
            "baseline": [10.0] * 5,
            "candidate": [11.2, 11.2, 11.2, 11.2, 10.5],
        })
        result = run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=runner)
        self.assertTrue(result["checks"]["median_at_least_threshold_slower"])
        self.assertFalse(result["checks"]["minimum_at_least_threshold_slower"])
        self.assertEqual(result["status"], "no-regression")

    def test_fewer_than_five_runs_are_rejected_before_launch(self):
        runner = FakeBenchmark({"baseline": [], "candidate": []})
        with self.assertRaisesRegex(ValueError, "at least five"):
            run_comparison("baseline", "candidate", "sbcl", runs=4, benchmark=runner)
        self.assertEqual(runner.calls, [])

    def test_changed_successful_file_set_is_incomparable_not_a_pass(self):
        def changed_files(ncl, _sbcl, _mode, **_kwargs):
            side = Path(ncl).name
            files = [
                {"suite": "synthetic", "file": "baseline.lisp", "status": "passed", "compile_seconds": 1.0}
            ]
            if side == "candidate":
                files = [
                    {"suite": "synthetic", "file": "candidate.lisp", "status": "passed", "compile_seconds": 1.0}
                ]
            return {
                "status": "complete",
                "corpus_file_count": 1,
                "runtimes": {
                    "ncl": {
                        "file_count": 1,
                        "compiled_file_count": 1,
                        "failed_file_count": 0,
                        "files": files,
                    }
                },
            }

        with self.assertRaisesRegex(ComparisonError, "different corpus"):
            run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=changed_files)

    def test_incomplete_benchmark_cannot_be_classified_as_no_regression(self):
        def incomplete(*_args, **_kwargs):
            return {"status": "error"}

        with self.assertRaisesRegex(ComparisonError, "incomplete"):
            run_comparison("baseline", "candidate", "sbcl", benchmark=incomplete)

    def test_failed_source_cannot_be_dropped_from_a_successful_comparison(self):
        def failed_source(ncl, _sbcl, _mode, **_kwargs):
            return {
                "status": "complete",
                "corpus_file_count": 1,
                "runtimes": {
                    "ncl": {
                        "file_count": 1,
                        "compiled_file_count": 0,
                        "failed_file_count": 1,
                        "files": [
                            {"suite": "synthetic", "file": "one.lisp", "status": "failed", "compile_seconds": None}
                        ]
                    }
                },
            }

        with self.assertRaisesRegex(ComparisonError, "failed source"):
            run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=failed_source)

    def test_missing_corpus_rows_cannot_be_classified_as_no_regression(self):
        def missing_source(ncl, _sbcl, _mode, **_kwargs):
            return {
                "status": "complete",
                "corpus_file_count": 3,
                "runtimes": {
                    "ncl": {
                        "file_count": 3,
                        "compiled_file_count": 2,
                        "failed_file_count": 0,
                        "files": [
                            {"suite": "synthetic", "file": "one.lisp", "status": "passed", "compile_seconds": 1.0},
                            {"suite": "synthetic", "file": "two.lisp", "status": "passed", "compile_seconds": 1.0},
                        ],
                    }
                },
            }

        with self.assertRaisesRegex(ComparisonError, "source denominator"):
            run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=missing_source)


if __name__ == "__main__":
    unittest.main()
