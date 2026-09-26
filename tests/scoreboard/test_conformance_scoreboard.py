import importlib.util
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


class ScoreboardTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
