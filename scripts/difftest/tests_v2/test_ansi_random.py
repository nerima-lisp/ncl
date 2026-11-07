from __future__ import annotations

import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import ansi_random


class RunnerSourceTests(unittest.TestCase):
    def test_rtp_runner_selects_random_type_prop_tests_and_fixed_repetitions(self):
        runner = ansi_random._rtp_runner(seed_count=7, seed_start=3)

        self.assertIn('"RANDOM-TYPE-PROP."', runner)
        self.assertIn('set (find-symbol "*DEFAULT-REPS*" cl-test-package) 7', runner)
        self.assertIn("(loop repeat 3 do (random 2147483647))", runner)
        self.assertIn('"PHASE" "random-type-prop:start"', runner)
        self.assertIn('"RTP-SUMMARY" attempted passed failed errors', runner)

    def test_integer_runner_bounds_the_infinite_upstream_loop(self):
        runner = ansi_random._loop_runner(
            seed_count=11,
            seed_start=2,
            size=32,
            nvars=4,
        )

        self.assertIn('"TEST-RANDOM-INTEGER-FORMS" cl-test-package', runner)
        self.assertIn("(symbol-function test-symbol) 32 4 11", runner)
        self.assertIn(":index (* 2 11)", runner)
        self.assertIn('"PHASE" "loop-random-int-forms:start"', runner)


class OutputParsingTests(unittest.TestCase):
    def test_rtp_failures_are_counted_and_preserve_generated_case(self):
        stdout = "\n".join(
            (
                "NCL-ANSI-RANDOM|RTP|PASS|RANDOM-TYPE-PROP.TYPEP.1",
                "NCL-ANSI-RANDOM|RTP|FAIL|RANDOM-TYPE-PROP.EQL.1|(:form ...)",
                "NCL-ANSI-RANDOM|RTP-SUMMARY|2|1|1|0",
            )
        )

        result = ansi_random._parse_output("rtp", stdout, "", 0, False)

        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["summary"], {
            "attempted": 2,
            "passed": 1,
            "failed": 1,
            "errors": 0,
        })
        self.assertEqual(result["tests"][1]["detail"], "(:form ...)")

    def test_missing_random_feature_is_recorded_as_unsupported(self):
        stdout = "NCL-ANSI-RANDOM|BLOCKED|rtp|load-or-run|undefined COMPILE"

        result = ansi_random._parse_output("rtp", stdout, "", 0, False)

        self.assertEqual(result["status"], "unsupported")
        self.assertEqual(result["blocked"]["phase"], "load-or-run")
        self.assertEqual(result["blocked"]["reason"], "undefined COMPILE")

    def test_process_failure_without_summary_is_not_a_pass(self):
        result = ansi_random._parse_output("loop-random-int-forms", "", "crashed", 134, False)

        self.assertEqual(result["status"], "crash")

    def test_crash_after_blocked_marker_keeps_crash_artifacts(self):
        stdout = "NCL-ANSI-RANDOM|BLOCKED|rtp|load-or-run|FILE-ERROR condition"

        result = ansi_random._parse_output("rtp", stdout, "", 134, False)

        self.assertEqual(result["status"], "crash")
        with tempfile.TemporaryDirectory() as directory:
            artifacts = ansi_random._save_artifacts(
                Path(directory), 0, 1, "rtp", result, ansi_random._rtp_runner(1, 0)
            )

            self.assertEqual(len(artifacts), 2)
            record = json.loads((Path(directory) / artifacts[1]).read_text(encoding="utf-8"))
            self.assertEqual(record["status"], "crash")

    def test_timeout_preserves_progress_phases(self):
        stdout = "\n".join(
            (
                "NCL-ANSI-RANDOM|PHASE|runner-ready",
                "NCL-ANSI-RANDOM|PHASE|gclload1:start",
            )
        )

        result = ansi_random._parse_output("rtp", stdout, "", None, True)

        self.assertEqual(result["status"], "timeout")
        self.assertEqual(result["phases"], ["runner-ready", "gclload1:start"])


class SuiteExecutionTests(unittest.TestCase):
    def _fake_ncl(self, root: Path, *, mismatch: bool) -> Path:
        executable = root / "fake-ncl"
        rtp_status = "FAIL" if mismatch else "PASS"
        rtp_summary = "1|0|1|0" if mismatch else "1|1|0|0"
        executable.write_text(
            "#!/usr/bin/env python3\n"
            "from pathlib import Path\n"
            "import sys\n"
            "runner = Path(sys.argv[-1]).read_text(encoding='utf-8')\n"
            "if 'RANDOM-TYPE-PROP.' in runner:\n"
            f"    print('NCL-ANSI-RANDOM|RTP|{rtp_status}|RANDOM-TYPE-PROP.TEST')\n"
            f"    print('NCL-ANSI-RANDOM|RTP-SUMMARY|{rtp_summary}')\n"
            "else:\n"
            "    print('NCL-ANSI-RANDOM|LOOP-SUMMARY|5|0|NIL')\n",
            encoding="utf-8",
        )
        executable.chmod(0o755)
        return executable

    def test_main_reports_mismatch_and_saves_runner_and_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "ansi-test"
            source.mkdir()
            repro_dir = root / "repros"
            ncl = self._fake_ncl(root, mismatch=True)
            output = io.StringIO()

            with patch.object(ansi_random, "_source_root", return_value=source), \
                    patch.object(ansi_random, "_check_source"), \
                    contextlib.redirect_stdout(output):
                status = ansi_random.main([
                    "--ncl", str(ncl),
                    "--seed-count", "5",
                    "--seed-start", "9",
                    "--cache-dir", str(root / "cache"),
                    "--repro-dir", str(repro_dir),
                ])

            self.assertEqual(status, 1)
            report = json.loads(output.getvalue())
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["source_commit"], ansi_random.SOURCES["ansi-test"]["commit"])
            self.assertEqual(report["groups"][0]["summary"]["failed"], 1)
            artifacts = report["groups"][0]["artifacts"]
            self.assertEqual(len(artifacts), 2)
            self.assertTrue((repro_dir / artifacts[0]).is_file())
            failure = json.loads((repro_dir / artifacts[1]).read_text(encoding="utf-8"))
            self.assertEqual(failure["runner"], artifacts[0])

    def test_main_records_gclload_file_error_as_unsupported_nonzero(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "ansi-test"
            source.mkdir()
            executable = root / "unsupported-ncl"
            executable.write_text(
                "#!/usr/bin/env python3\n"
                "from pathlib import Path\n"
                "import sys\n"
                "runner = Path(sys.argv[-1]).read_text(encoding='utf-8')\n"
                "group = 'rtp' if 'RANDOM-TYPE-PROP.' in runner else 'loop-random-int-forms'\n"
                "print('NCL-ANSI-RANDOM|PHASE|runner-ready')\n"
                "print('NCL-ANSI-RANDOM|PHASE|gclload1:start')\n"
                "print('NCL-ANSI-RANDOM|BLOCKED|' + group + '|load-or-run|FILE-ERROR condition')\n",
                encoding="utf-8",
            )
            executable.chmod(0o755)
            output = io.StringIO()

            with patch.object(ansi_random, "_source_root", return_value=source), \
                    patch.object(ansi_random, "_check_source"), \
                    contextlib.redirect_stdout(output):
                status = ansi_random.main([
                    "--ncl", str(executable),
                    "--seed-count", "2",
                    "--seed-start", "0",
                    "--cache-dir", str(root / "cache"),
                    "--repro-dir", str(root / "repros"),
                ])

            self.assertEqual(status, 1)
            report = json.loads(output.getvalue())
            self.assertEqual(report["status"], "partial")
            self.assertTrue(all(group["status"] == "unsupported" for group in report["groups"]))
            for group in report["groups"]:
                self.assertEqual(group["blocked"]["reason"], "FILE-ERROR condition")
                self.assertEqual(group["blocked"]["last_progress"], "gclload1:start")
                self.assertIsNone(group["summary"])
                self.assertEqual(group["tests"], [])
            saved_report = json.loads(
                (root / "repros" / "ansi-random-report.json").read_text(encoding="utf-8")
            )
            self.assertEqual(saved_report["status"], "partial")


if __name__ == "__main__":
    unittest.main()
