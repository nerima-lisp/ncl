import importlib.util
import json
import shlex
import signal
import socket
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


MODULE_PATH = Path(__file__).parents[1] / "conformance_scoreboard.py"
spec = importlib.util.spec_from_file_location("conformance_scoreboard", MODULE_PATH)
scoreboard = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = scoreboard
spec.loader.exec_module(scoreboard)


class ConformanceScoreboardDiagnosticsTests(unittest.TestCase):
    def test_json_records_real_child_failure_and_sanitized_stderr(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            ansi_source = root / "ansi-source"
            bench_source = root / "bench-source"
            ansi_source.mkdir()
            bench_source.mkdir()
            ansi_script = root / "ansi.py"
            bench_script = root / "bench.py"
            artifact = root / "scoreboard.json"
            ansi_stdout = json.dumps(
                {"passed": 1, "failed": 0, "unexecuted": 0}
            ) + "\n"
            ansi_script.write_text(
                "import sys\n"
                f"sys.stdout.write({ansi_stdout!r})\n",
                encoding="utf-8",
            )
            absolute_path = root / "private" / "artifact.log"
            quoted_path = root / "dir with spaces" / "private.log"
            unquoted_spaced_path = root / "unquoted dir with spaces" / "private.log"
            hostname = socket.gethostname()
            stderr = (
                "x" * 4500
                + f"\nunquoted-path={absolute_path}"
                + f"\nquoted-path='{quoted_path}'"
                + f"\nunquoted-spaced-path={unquoted_spaced_path}"
                + f"\nhome-path={Path.home()}/private.log"
                + f"\nfile-uri={absolute_path.as_uri()}"
                + f"\nhost={hostname}\nstderr-tail-marker\n"
            )
            bench_stdout = json.dumps({
                "status": "failed",
                "times": [],
                "benchmarks": {
                    "CLOS/defclass": {
                        "status": "failed",
                        "stage": "load",
                        "times": [],
                        "condition": "TypeError: class-0-0 is not a type specifier",
                    },
                },
            }) + "\n"
            bench_script.write_text(
                "import sys\n"
                f"sys.stdout.write({bench_stdout!r})\n"
                f"sys.stderr.write({stderr!r})\n"
                "sys.stderr.flush()\n"
                "sys.exit(23)\n",
                encoding="utf-8",
            )
            source_dirs = {"ansi-test": ansi_source, "cl-bench": bench_source}

            def local_source(name, _cache_dir, **_kwargs):
                return source_dirs[name]

            with patch.object(scoreboard, "checkout_source", side_effect=local_source):
                exit_status = scoreboard.main(
                    [
                        "--cache-dir", str(root / "cache"),
                        "--ansi-command",
                        f"{shlex.quote(sys.executable)} {shlex.quote(str(ansi_script))}",
                        "--cl-bench-command",
                        f"{shlex.quote(sys.executable)} {shlex.quote(str(bench_script))}",
                        "--format", "json",
                        "--output", str(artifact),
                    ]
                )

            self.assertEqual(exit_status, 0)
            report = json.loads(artifact.read_text(encoding="utf-8"))
            bench = report["cl-bench"]
            self.assertEqual(report["ansi-test"]["status"], "passed")
            self.assertEqual(bench["status"], "failed")
            self.assertEqual(bench["driver_status"], "failed")
            self.assertEqual(
                bench["benchmarks"]["CLOS/defclass"]["stage"],
                "load",
            )
            self.assertEqual(
                bench["benchmarks"]["CLOS/defclass"]["condition"],
                "TypeError: class-0-0 is not a type specifier",
            )
            self.assertEqual(bench["return_code"], 23)
            self.assertNotIn("signal_number", bench)
            self.assertEqual(bench["samples"], 0)
            self.assertNotIn("geometric_mean", bench)
            self.assertTrue(bench["stderr_truncated"])
            self.assertLessEqual(len(bench["stderr_tail"]), 4000)
            self.assertIn("stderr-tail-marker", bench["stderr_tail"])
            self.assertIn("unquoted-path=<absolute-path>", bench["stderr_tail"])
            self.assertIn("quoted-path='<absolute-path>'", bench["stderr_tail"])
            self.assertIn("unquoted-spaced-path=<absolute-path>", bench["stderr_tail"])
            self.assertIn("home-path=<absolute-path>", bench["stderr_tail"])
            self.assertIn("file-uri=<absolute-path>", bench["stderr_tail"])
            self.assertIn("host=<hostname>", bench["stderr_tail"])
            self.assertNotIn(str(root), bench["stderr_tail"])
            self.assertNotIn(str(Path.home()), bench["stderr_tail"])
            self.assertNotIn(hostname, bench["stderr_tail"])

    def test_json_exposes_direct_or_reported_signal_and_keeps_missing_code_null(self):
        cases = (
            (-signal.SIGTERM, signal.SIGTERM, "diagnostic"),
            (None, None, "diagnostic"),
            (128 + signal.SIGTERM, None, "diagnostic"),
            (
                128 + signal.SIGSEGV,
                signal.SIGSEGV,
                "Segmentation fault (core dumped)\n",
            ),
            (128 + signal.SIGSEGV, None, "diagnostic"),
        )
        for returncode, expected_signal, stderr in cases:
            with self.subTest(returncode=returncode, stderr=stderr):
                bench = scoreboard.ProcessResult("crash", returncode, stderr=stderr)
                report = scoreboard.make_scoreboard(
                    scoreboard.ProcessResult("passed", 0),
                    {"passed": 1, "failed": 0, "unexecuted": 0},
                    bench,
                    None,
                )
                cl_bench = json.loads(json.dumps(report))["cl-bench"]
                self.assertEqual(cl_bench["return_code"], returncode)
                if expected_signal is None:
                    self.assertNotIn("signal_number", cl_bench)
                else:
                    self.assertEqual(cl_bench["signal_number"], expected_signal)


if __name__ == "__main__":
    unittest.main()
