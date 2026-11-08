import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).parents[1] / "check_crate_dependencies.py"
spec = importlib.util.spec_from_file_location("check_crate_dependencies", MODULE_PATH)
checker = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)


class CheckCrateDependenciesTests(unittest.TestCase):
    def make_workspace(self, root: Path, edge_rows: str, exception_rows: str = "") -> None:
        (root / "packages/a").mkdir(parents=True)
        (root / "packages/b").mkdir(parents=True)
        (root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["packages/*"]\n'
            '[workspace.dependencies]\nncl-b = { path = "packages/b" }\n'
        )
        (root / "packages/a/Cargo.toml").write_text(
            '[package]\nname = "ncl-a"\n'
            '[dependencies]\nncl-b = { workspace = true }\nserde = "1"\n'
            '[dev-dependencies]\nncl-b = { path = "../b" }\n'
        )
        (root / "packages/b/Cargo.toml").write_text('[package]\nname = "ncl-b"\n')
        edge_path = root / checker.EDGE_TABLE
        edge_path.parent.mkdir(parents=True)
        edge_path.write_text("from\tto\tkind\ttarget\n" + edge_rows)
        exception_path = root / checker.EXCEPTION_TABLE
        exception_path.write_text(
            "from\tto\tkind\ttarget\towner_lane\n" + exception_rows
        )

    def test_accepts_allowed_workspace_edge_and_ignores_external_and_dev_edges(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.make_workspace(root, "ncl-a\tncl-b\tnormal\t\n")
            packages, edges, errors = checker.check_dependencies(root)

        self.assertEqual(packages, 2)
        self.assertEqual(edges, 1)
        self.assertEqual(errors, [])

    def test_reports_unlisted_local_dependency(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.make_workspace(root, "")
            _, edges, errors = checker.check_dependencies(root)

        self.assertEqual(edges, 1)
        self.assertIn("ncl-a -> ncl-b", " ".join(errors))
        self.assertIn("not allowed by the edge table or exception table", " ".join(errors))

    def test_accepts_current_dependency_with_lane_owned_exception(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.make_workspace(
                root,
                "",
                "ncl-a\tncl-b\tnormal\t\tL9-1\n",
            )
            packages, edges, errors = checker.check_dependencies(root)

        self.assertEqual(packages, 2)
        self.assertEqual(edges, 1)
        self.assertEqual(errors, [])

    def test_reports_exception_when_dependency_is_removed(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.make_workspace(
                root,
                "",
                "ncl-a\tncl-b\tnormal\t\tL9-1\n",
            )
            manifest = root / "packages/a/Cargo.toml"
            manifest.write_text('[package]\nname = "ncl-a"\n')
            _, _, errors = checker.check_dependencies(root)

        self.assertIn("exception ncl-a -> ncl-b", " ".join(errors))
        self.assertIn("dependency is no longer declared", " ".join(errors))

    def test_reports_exception_when_edge_moves_into_allowlist(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.make_workspace(
                root,
                "ncl-a\tncl-b\tnormal\t\n",
                "ncl-a\tncl-b\tnormal\t\tL9-1\n",
            )
            _, _, errors = checker.check_dependencies(root)

        self.assertIn("exception ncl-a -> ncl-b", " ".join(errors))
        self.assertIn("now allowed by the edge table", " ".join(errors))

    def test_checks_target_specific_normal_dependencies(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            self.make_workspace(
                root,
                "ncl-a\tncl-b\tnormal\t\n"
                "ncl-a\tncl-b\tnormal\tcfg(unix)\n",
            )
            a_manifest = root / "packages/a/Cargo.toml"
            a_manifest.write_text(
                a_manifest.read_text()
                + '[target."cfg(unix)".dependencies]\n'
                + 'ncl-b = { path = "../b" }\n'
            )
            _, edges, errors = checker.check_dependencies(root)

        self.assertEqual(edges, 2)
        self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
