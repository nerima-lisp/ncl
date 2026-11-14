from __future__ import annotations

from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))

from bench.corpus.compile import _timed_result, _write_compile_driver
from bench.timing import MINIMUM_SAMPLE_SECONDS, measurement_metadata


class BenchmarkTimingTests(unittest.TestCase):
    def test_compile_driver_repeats_until_the_minimum_and_reports_count(self):
        with tempfile.TemporaryDirectory() as temporary:
            driver = Path(temporary) / "compile.lisp"
            _write_compile_driver(driver, Path(temporary) / "source.lisp")
            source = driver.read_text(encoding="utf-8")

        self.assertIn(f"{MINIMUM_SAMPLE_SECONDS}d0", source)
        self.assertIn("(loop", source)
        self.assertIn("(when (>= ticks minimum-ticks)", source)
        self.assertIn("(incf repetitions)", source)
        self.assertIn("__NCL_BENCH_RESULT__|2|~D|~D|~D", source)
        depth = 0
        in_string = False
        escaped = False
        for character in source:
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

    def test_compile_timing_is_normalized_and_rejects_short_samples(self):
        timing = _timed_result("__NCL_BENCH_RESULT__|2|100|50|2\n")

        self.assertEqual(
            timing,
            {"seconds": 0.25, "elapsed_seconds": 0.5, "repetitions": 2},
        )
        self.assertIsNone(_timed_result("__NCL_BENCH_RESULT__|2|100|5|1\n"))
        self.assertIsNone(_timed_result("__NCL_BENCH_RESULT__|2|100|50|0\n"))

    def test_metadata_records_revision_sbcl_platform_and_load_average(self):
        with (
            patch.dict(
                "os.environ",
                {"NCL_BENCH_REVISION": "a" * 40, "GITHUB_SHA": "b" * 40},
                clear=True,
            ),
            patch("bench.timing.platform.system", return_value="Linux"),
            patch("bench.timing.platform.machine", return_value="x86_64"),
            patch("bench.timing.os.getloadavg", return_value=(1.0, 2.0, 3.0)),
        ):
            metadata = measurement_metadata("2.6.10")

        self.assertEqual(metadata["ncl_sha"], "a" * 40)
        self.assertEqual(metadata["sbcl_version"], "2.6.10")
        self.assertEqual(metadata["host"], "Linux x86_64")
        self.assertEqual(metadata["load_average"], [1.0, 2.0, 3.0])
        self.assertEqual(metadata["minimum_sample_seconds"], MINIMUM_SAMPLE_SECONDS)

    def test_metadata_uses_ci_revision_fallback_and_allows_missing_load_average(self):
        with (
            patch.dict("os.environ", {"GITHUB_SHA": "c" * 40}, clear=True),
            patch("bench.timing.platform.system", return_value="Darwin"),
            patch("bench.timing.platform.machine", return_value="arm64"),
            patch("bench.timing.os.getloadavg", side_effect=OSError("unavailable")),
        ):
            metadata = measurement_metadata("2.6.10")

        self.assertEqual(metadata["ncl_sha"], "c" * 40)
        self.assertEqual(metadata["host"], "Darwin arm64")
        self.assertIsNone(metadata["load_average"])


if __name__ == "__main__":
    unittest.main()
