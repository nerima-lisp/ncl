import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


MODULE_PATH = Path(__file__).parents[1] / "reachability.py"
spec = importlib.util.spec_from_file_location("reachability", MODULE_PATH)
reachability = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = reachability
spec.loader.exec_module(reachability)


class ReachabilityOutputTests(unittest.TestCase):
    def test_reports_distinct_counts_and_keeps_orphan_failure(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            repo_root = Path(temporary_directory)
            crate_root = repo_root / "pseudo-crate"
            src_root = crate_root / "src"
            outside_src = crate_root / "outside"
            src_root.mkdir(parents=True)
            outside_src.mkdir()

            (src_root / "lib.rs").write_text(
                'mod live;\n'
                '#[path = "../outside/path_target.rs"]\n'
                "mod path_target;\n"
                'include!("fragment.inc");\n',
                encoding="utf-8",
            )
            (src_root / "live.rs").write_text("", encoding="utf-8")
            (src_root / "fragment.inc").write_text("", encoding="utf-8")
            (outside_src / "path_target.rs").write_text("", encoding="utf-8")
            (src_root / "orphan.rs").write_text("", encoding="utf-8")

            stdout = io.StringIO()
            with patch.object(reachability, "workspace_members", return_value=[crate_root]), \
                    patch.object(reachability, "REPO_ROOT", repo_root), \
                    patch.object(sys, "argv", [str(MODULE_PATH)]), \
                    contextlib.redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as exit_error:
                    reachability.main()

            self.assertEqual(exit_error.exception.code, 1)
            output = stdout.getvalue()
            self.assertIn(
                "== pseudo-crate == reached Rust files under src: 2 / 3; "
                "other reached paths: 2",
                output,
            )
            self.assertIn("ORPHAN: pseudo-crate/src/orphan.rs", output)


if __name__ == "__main__":
    unittest.main()
