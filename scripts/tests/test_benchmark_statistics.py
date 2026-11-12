from __future__ import annotations

from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))

from bench.corpus.report import aggregate_file_samples
from bench.statistics import compare_statistics, paired_statistics, sample_statistics
from micro_bench import _summarize_engine
from optimization_baseline_scoring import _score_runs
from performance_summary import (
    render_ab_comparison,
    render_cl_bench_table,
    render_compile_table,
    render_micro_table,
)
from compare_benchmarks import run_comparison


class BenchmarkStatisticsTests(unittest.TestCase):
    def test_sample_summary_reports_count_median_and_max_minus_min(self):
        self.assertEqual(
            sample_statistics([0.5, 1.5, 1.0]),
            {
                "sample_count": 3,
                "median_seconds": 1.0,
                "minimum_seconds": 0.5,
                "maximum_seconds": 1.5,
                "range_seconds": 1.0,
            },
        )

    def test_comparison_marks_an_effect_at_or_below_the_range_as_noise(self):
        within = compare_statistics(
            sample_statistics([1.0, 1.1, 1.2]),
            sample_statistics([1.0, 1.1, 1.2]),
        )
        outside = compare_statistics(
            sample_statistics([1.19, 1.2, 1.21]),
            sample_statistics([0.99, 1.0, 1.01]),
        )

        self.assertTrue(within["difference_within_noise"])
        self.assertTrue(within["ratio_within_noise"])
        self.assertTrue(within["noise_below"])
        self.assertFalse(outside["difference_within_noise"])
        self.assertFalse(outside["ratio_within_noise"])
        self.assertFalse(outside["noise_below"])

    def test_ratio_classification_uses_relative_ranges(self):
        first = sample_statistics([50.0, 100.0, 150.0])
        second = sample_statistics([1.0, 1.0, 1.0])

        self.assertTrue(compare_statistics(first, second)["noise_below"])
        ratio_comparison = compare_statistics(first, second, noise_basis="ratio")
        self.assertFalse(ratio_comparison["noise_below"])
        self.assertEqual(ratio_comparison["noise_basis"], "ratio")

    def test_ratio_classification_is_inconclusive_when_denominator_is_zero(self):
        comparison = compare_statistics(
            sample_statistics([1.0, 1.0]),
            sample_statistics([0.0, 0.0]),
            noise_basis="ratio",
        )

        self.assertIsNone(comparison["ratio"])
        self.assertIsNone(comparison["ratio_within_noise"])
        self.assertIsNone(comparison["noise_below"])

    def test_paired_summary_uses_pair_ids_and_reports_delta_median_and_wins(self):
        ncl = [
            {"pair": 1, "status": "passed", "seconds": 0.8},
            {"pair": 2, "status": "passed", "seconds": 1.2},
            {"pair": 3, "status": "passed", "seconds": 0.9},
            {"pair": 4, "status": "passed", "seconds": 1.1},
            {"pair": 5, "status": "passed", "seconds": 1.3},
        ]
        sbcl = [
            {"pair": 5, "status": "passed", "seconds": 1.0},
            {"pair": 4, "status": "passed", "seconds": 1.0},
            {"pair": 3, "status": "passed", "seconds": 1.0},
            {"pair": 2, "status": "passed", "seconds": 1.0},
            {"pair": 1, "status": "passed", "seconds": 1.0},
        ]

        summary = paired_statistics(ncl, sbcl, expected_sample_count=5)

        self.assertEqual(summary["sample_count"], 5)
        self.assertAlmostEqual(summary["median_delta_seconds"], 0.1)
        self.assertEqual(summary["ncl_wins"], 2)
        self.assertEqual(summary["sbcl_wins"], 3)
        self.assertEqual(summary["ties"], 0)
        self.assertTrue(summary["noise_below"])

    def test_incomplete_paired_summary_has_no_noise_classification(self):
        ncl = [{"pair": 1, "status": "passed", "seconds": 2.0}]
        sbcl = [{"pair": 1, "status": "passed", "seconds": 1.0}]

        summary = paired_statistics(ncl, sbcl, expected_sample_count=5)
        table = render_micro_table(
            {
                "cases": {
                    "fixture": {
                        "engines": {},
                        "paired_comparison": summary,
                    },
                },
            }
        )

        self.assertEqual(summary["sample_count"], 1)
        self.assertEqual(summary["expected_sample_count"], 5)
        self.assertEqual(summary["range_seconds"], 0.0)
        self.assertIsNone(summary["noise_below"])
        self.assertIn("1/5", table)
        self.assertIn("| n/a |", table)
        self.assertNotIn("exceeds noise", table)

    def test_micro_engine_summary_includes_timing_range(self):
        samples = [
            {
                "status": "passed",
                "seconds": seconds,
                "allocated_bytes": 100,
                "allocation_counter": "counter",
                "allocation_reason": None,
                "engine": "ncl",
            }
            for seconds in (1.0, 2.0, 3.0)
        ]

        summary = _summarize_engine(samples, runs=3)

        self.assertEqual(summary["sample_count"], 3)
        self.assertEqual(summary["median_seconds"], 2.0)
        self.assertEqual(summary["range_seconds"], 2.0)

    def test_cl_bench_scoring_reports_stats_per_implementation(self):
        runs = []
        for mode, values in (("off", (1.0, 1.1)), ("default", (0.8, 0.9)), (None, (1.0, 1.1))):
            for pair in range(1, 6):
                runs.append({
                    "phase": "measurement",
                    "mode": mode,
                    "benchmarks": {
                        "FIB": {"status": "passed", "times": list(values), "error": None},
                    },
                })

        scores = _score_runs(runs, pairs=5, sbcl_samples=5, benchmark_names=("FIB",))

        off = scores["benchmark_statistics_seconds"]["ncl_off"]["FIB"]
        self.assertEqual(off["sample_count"], 10)
        self.assertEqual(off["measurement_run_count"], 5)
        self.assertEqual(off["median_seconds"], 1.05)
        self.assertAlmostEqual(off["range_seconds"], 0.1)
        self.assertEqual(scores["benchmark_comparisons"]["FIB"]["off_over_sbcl"]["noise_basis"], "ratio")
        self.assertIn("below noise", render_cl_bench_table({"scores": scores}))

    def test_compile_file_aggregation_reports_successful_sample_range(self):
        rows = [
            {
                "suite": "synthetic",
                "version": "1",
                "file": "case.lisp",
                "line_count": 2,
                "status": "passed",
                "compile_seconds": seconds,
                "reason": None,
                "peak_rss_bytes": 128,
                "process_wall_seconds": seconds,
            }
            for seconds in (0.2, 0.4, 0.3)
        ]

        summary = aggregate_file_samples(rows)

        self.assertEqual(summary["sample_count"], 3)
        self.assertEqual(summary["expected_sample_count"], 3)
        self.assertEqual(summary["median_seconds"], 0.3)
        self.assertAlmostEqual(summary["range_seconds"], 0.2)

    def test_micro_table_marks_a_pair_effect_within_noise(self):
        table = render_micro_table({
            "cases": {
                "fixture": {
                    "engines": {
                        "ncl": {"sample_count": 5, "median_seconds": 1.0, "range_seconds": 0.2},
                        "sbcl": {"sample_count": 5, "median_seconds": 1.1, "range_seconds": 0.3},
                    },
                    "paired_comparison": {
                        "sample_count": 5,
                        "expected_sample_count": 5,
                        "median_delta_seconds": -0.02,
                        "ncl_wins": 3,
                        "sbcl_wins": 2,
                        "ties": 0,
                        "noise_below": True,
                    },
                },
            },
        })

        self.assertIn("below noise", table)
        self.assertIn("-0.02", table)
        self.assertIn("5/5", table)
        self.assertIn("3/2/0", table)

    def test_compile_table_prints_case_ranges_and_noise_classification(self):
        report = {
            "runtimes": {
                "ncl": {
                    "total_compile_sample_count": 5,
                    "total_compile_seconds": 1.0,
                    "total_compile_range_seconds": 0.2,
                    "compiled_file_count": 1,
                    "lines_per_second": 10.0,
                    "peak_rss_bytes": 1024,
                    "peak_rss_sample_count": 5,
                    "peak_rss_expected_sample_count": 5,
                    "binary_size_bytes": 2048,
                    "startup": {"sample_count": 5, "median_seconds": 0.1, "range_seconds": 0.02},
                    "files": [{
                        "suite": "synthetic", "file": "case.lisp", "sample_count": 5,
                        "median_seconds": 0.4, "range_seconds": 0.08,
                    }],
                },
                "sbcl": {"files": []},
            },
            "case_comparisons": {
                "synthetic/case.lisp": {"ratio": 1.0, "noise_below": True},
            },
        }

        table = render_compile_table(report)

        self.assertIn("synthetic/case.lisp", table)
        self.assertIn("0.08", table)
        self.assertIn("below noise", table)

    def test_ab_scoring_reports_per_file_spread_and_noise(self):
        samples = [10.0, 10.02, 9.98, 10.01, 9.99]
        indices = {"baseline": 0, "candidate": 0}

        def benchmark(ncl, _sbcl, _mode, **_kwargs):
            side = Path(ncl).name
            index = indices[side]
            indices[side] += 1
            offset = 0.01 if side == "candidate" else 0.0
            total = samples[index] + offset
            return {
                "status": "complete",
                "corpus_file_count": 2,
                "runtimes": {
                    "ncl": {
                        "file_count": 2,
                        "compiled_file_count": 2,
                        "failed_file_count": 0,
                        "files": [
                            {"suite": "synthetic", "file": "one.lisp", "status": "passed", "compile_seconds": total / 2},
                            {"suite": "synthetic", "file": "two.lisp", "status": "passed", "compile_seconds": total / 2},
                        ],
                    },
                },
            }

        report = run_comparison("/bin/baseline", "/bin/candidate", "sbcl", benchmark=benchmark)

        self.assertEqual(report["baseline"]["sample_count"], 5)
        self.assertAlmostEqual(report["baseline"]["range_seconds"], 0.04)
        self.assertAlmostEqual(report["comparison"]["median_difference_seconds"], 0.01)
        self.assertTrue(report["comparison"]["noise_below"])
        self.assertEqual(report["file_comparisons"]["synthetic/one.lisp"]["candidate"]["sample_count"], 5)
        self.assertTrue(report["file_comparisons"]["synthetic/one.lisp"]["comparison"]["noise_below"])
        self.assertIn("below noise", render_ab_comparison(report))


if __name__ == "__main__":
    unittest.main()
