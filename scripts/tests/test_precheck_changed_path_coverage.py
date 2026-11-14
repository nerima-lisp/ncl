import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))

import find_parent_coverage
import precheck_changed_path_coverage


class ChangedCratesTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def add_package(self, path, name):
        package = self.root / path
        package.mkdir(parents=True)
        (package / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "0.1.0"\n',
            encoding="utf-8",
        )

    def test_maps_changed_paths_to_sorted_unique_package_names(self):
        self.add_package("packages/zeta", "ncl-zeta")
        self.add_package("packages/alpha", "ncl-alpha")

        crates = precheck_changed_path_coverage.changed_crates(
            [
                "packages/zeta/src/lib.rs",
                "packages/alpha/tests/api.rs",
                "packages/zeta/tests/reader.rs",
            ],
            self.root,
        )

        self.assertEqual(crates, ["ncl-alpha", "ncl-zeta"])

    def test_uses_nearest_package_and_respects_path_component_boundaries(self):
        self.add_package("packages/compiler", "ncl-compiler")
        self.add_package("packages/compiler/front", "ncl-compiler-front")
        self.add_package("packages/compiler-extra", "ncl-compiler-extra")

        crates = precheck_changed_path_coverage.changed_crates(
            [
                "packages/compiler/front/src/lib.rs",
                "packages/compiler-extra/src/lib.rs",
            ],
            self.root,
        )

        self.assertEqual(crates, ["ncl-compiler-extra", "ncl-compiler-front"])

    def test_ignores_paths_outside_any_package(self):
        self.add_package("packages/core", "ncl-core")

        crates = precheck_changed_path_coverage.changed_crates(
            ["docs/development.md", "scripts/precheck_changed_path_coverage.py"],
            self.root,
        )

        self.assertEqual(crates, [])

    def test_root_package_only_includes_rust_source_paths(self):
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "ncl"\nversion = "0.1.0"\n',
            encoding="utf-8",
        )

        source_crates = precheck_changed_path_coverage.changed_crates(
            ["src/main.rs"], self.root
        )
        non_source_crates = precheck_changed_path_coverage.changed_crates(
            ["README.md", "Cargo.toml", "docs/project.md", "scripts/check.py"],
            self.root,
        )

        self.assertEqual(source_crates, ["ncl"])
        self.assertEqual(non_source_crates, [])


class ParentArtifactSelectionTests(unittest.TestCase):
    @staticmethod
    def artifact(run_id, sha, *, branch="main", name="coverage-summary", expired=False):
        return {
            "id": run_id + 1000,
            "name": name,
            "expired": expired,
            "created_at": f"2026-11-09T00:{run_id % 60:02d}:00Z",
            "workflow_run": {
                "id": run_id,
                "head_sha": sha,
                "head_branch": branch,
            },
        }

    def test_prefers_exact_base_and_ignores_nonmatching_artifacts(self):
        base, ancestor = "a" * 40, "b" * 40
        payload = [
            {
                "artifacts": [
                    self.artifact(11, ancestor),
                    self.artifact(12, base),
                    self.artifact(13, "c" * 40, branch="feature"),
                    self.artifact(14, "d" * 40, name="coverage-lcov"),
                    self.artifact(15, "e" * 40, expired=True),
                ]
            }
        ]

        selected = find_parent_coverage.select_parent_artifact(
            payload, "main", [base, ancestor]
        )

        self.assertEqual(selected, {"run_id": "12", "sha": base, "fallback": False})


class ParentSummaryNormalizationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.changed_path = "packages/core/src/lib.rs"

    def tearDown(self):
        self.temporary.cleanup()

    @staticmethod
    def summary(*filenames):
        return {"data": [{"files": [{"filename": name} for name in filenames]}]}

    def normalize(self, payload):
        return precheck_changed_path_coverage.normalize_parent_summary(
            payload, {self.changed_path}, self.root
        )

    def test_preserves_relative_and_repository_absolute_filenames(self):
        local_absolute = str(self.root / self.changed_path)
        payload = self.summary(self.changed_path, local_absolute)

        normalized = self.normalize(payload)

        filenames = [entry["filename"] for entry in normalized["data"][0]["files"]]
        self.assertEqual(filenames, [self.changed_path, local_absolute])

    def test_converts_matching_ci_absolute_filename_to_relative_path(self):
        ci_absolute = f"/home/runner/work/ncl/ncl/{self.changed_path}"
        payload = self.summary(ci_absolute)

        normalized = self.normalize(payload)

        self.assertEqual(
            normalized["data"][0]["files"][0]["filename"], self.changed_path
        )
        self.assertEqual(payload["data"][0]["files"][0]["filename"], ci_absolute)

    def test_leaves_nonmatching_absolute_filename_unchanged(self):
        unrelated = "/home/runner/work/ncl/ncl/packages/core/src/lib.rs.backup"
        payload = self.summary(unrelated)

        normalized = self.normalize(payload)

        self.assertEqual(normalized["data"][0]["files"][0]["filename"], unrelated)


if __name__ == "__main__":
    unittest.main()
