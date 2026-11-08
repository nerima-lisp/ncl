import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch


SCRIPTS = Path(__file__).parents[1]


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = load_module("check_changed_path_coverage", SCRIPTS / "check_changed_path_coverage.py")
parent_finder = load_module("find_parent_coverage", SCRIPTS / "find_parent_coverage.py")


def region_stats(count, missed):
    return {
        "count": count,
        "covered": count - missed,
        "notcovered": missed,
        "percent": 100.0 if count == 0 else (count - missed) * 100.0 / count,
    }


def summary(files):
    entries = []
    total_count = total_missed = 0
    for path, (count, missed) in files.items():
        entries.append(
            {
                "filename": path,
                "summary": {"regions": region_stats(count, missed)},
            }
        )
        total_count += count
        total_missed += missed
    return {
        "data": [
            {
                "files": entries,
                "totals": {"regions": region_stats(total_count, total_missed)},
            }
        ]
    }


def evaluate(files, parent_files, changed):
    root = Path("/repo")
    candidate = checker.parse_summary(summary(files), root)
    parent = checker.parse_summary(summary(parent_files), root)
    paths = checker.normalize_changed_paths(changed, root)
    return checker.evaluate(candidate, parent, paths)


class CheckChangedPathCoverageTests(unittest.TestCase):
    def test_path_output_escapes_control_and_markdown_delimiters(self):
        displayed = checker._display_path("bad\n::warning title=x:: forged\n`<tag>|column")
        self.assertNotIn("\n", displayed)
        self.assertNotIn("`", displayed)
        self.assertNotIn("<tag>", displayed)
        self.assertNotIn("|", displayed)

    def test_aggregate_passes_when_changed_path_missed_regions_do_not_increase(self):
        aggregate, per_path, current, previous, _ = evaluate(
            {"src/a.rs": (100, 9)},
            {"src/a.rs": (100, 10)},
            ["src/a.rs"],
        )
        self.assertTrue(aggregate)
        self.assertTrue(per_path)
        self.assertEqual(current.missed, 9)
        self.assertEqual(previous.missed, 10)

    def test_aggregate_passes_at_95_percent_even_when_missed_regions_increase(self):
        aggregate, per_path, _, _, _ = evaluate(
            {"src/a.rs": (100, 5)},
            {"src/a.rs": (100, 0)},
            ["src/a.rs"],
        )
        self.assertTrue(aggregate)
        self.assertTrue(per_path)

    def test_aggregate_fails_below_95_when_missed_regions_increase(self):
        aggregate, per_path, _, _, _ = evaluate(
            {"src/a.rs": (100, 6)},
            {"src/a.rs": (100, 0)},
            ["src/a.rs"],
        )
        self.assertFalse(aggregate)
        self.assertFalse(per_path)

    def test_failed_ratchet_exits_one_and_names_the_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            candidate = root / "candidate.json"
            parent = root / "parent.json"
            changed = root / "changed-paths"
            candidate.write_text(json.dumps(summary({"src/a.rs": (100, 6)})), encoding="utf-8")
            parent.write_text(json.dumps(summary({"src/a.rs": (100, 0)})), encoding="utf-8")
            changed.write_bytes(b"src/a.rs\0")
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                status = checker.main(
                    [
                        "--candidate",
                        str(candidate),
                        "--parent",
                        str(parent),
                        "--changed-paths",
                        str(changed),
                        "--root",
                        str(root),
                    ]
                )
        self.assertEqual(status, 1)
        self.assertIn("src/a.rs", stdout.getvalue())
        self.assertIn("| Path | Candidate | Parent | Per-path ratchet |", stdout.getvalue())
        self.assertIn("(a) aggregate changed-path ratchet: FAIL", stdout.getvalue())

    def test_each_parent_path_is_checked_independently(self):
        aggregate, per_path, current, previous, rows = evaluate(
            {"src/a.rs": (10, 1), "src/b.rs": (10, 9)},
            {"src/a.rs": (10, 0), "src/b.rs": (10, 10)},
            ["src/a.rs", "src/b.rs"],
        )
        self.assertTrue(aggregate)
        self.assertFalse(per_path)
        self.assertEqual(current.missed, previous.missed)
        self.assertEqual([row[3] for row in rows], [False, True])

    def test_changed_path_missing_from_both_summaries_has_zero_regions(self):
        aggregate, per_path, current, previous, rows = evaluate(
            {}, {}, ["docs/guide.md"]
        )
        self.assertTrue(aggregate)
        self.assertTrue(per_path)
        self.assertEqual(current, checker.RegionStats())
        self.assertEqual(previous, checker.RegionStats())
        self.assertIsNone(rows[0][3])

    def test_new_file_is_in_aggregate_but_has_no_parent_path_rule(self):
        aggregate, per_path, current, previous, rows = evaluate(
            {"src/new.rs": (100, 5)}, {}, ["src/new.rs"]
        )
        self.assertTrue(aggregate)
        self.assertTrue(per_path)
        self.assertEqual(current.count, 100)
        self.assertEqual(previous.count, 0)
        self.assertIsNone(rows[0][3])

    def test_parent_summary_missing_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            candidate = root / "candidate.json"
            changed = root / "changed-paths"
            missing_parent = root / "missing-parent.json"
            candidate.write_text(json.dumps(summary({})), encoding="utf-8")
            changed.write_bytes(b"")
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                status = checker.main(
                    [
                        "--candidate",
                        str(candidate),
                        "--parent",
                        str(missing_parent),
                        "--changed-paths",
                        str(changed),
                        "--root",
                        str(root),
                    ]
                )
        self.assertEqual(status, 2)
        self.assertIn("cannot read coverage summary", stderr.getvalue())


class FindParentCoverageTests(unittest.TestCase):
    def artifact(self, run_id, sha, *, branch="main", expired=False, name="coverage-summary"):
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

    def test_selects_exact_merge_base_artifact(self):
        base, parent = "a" * 40, "b" * 40
        payload = [
            {
                "artifacts": [
                    self.artifact(11, base),
                    self.artifact(12, parent),
                ]
            }
        ]
        selected = parent_finder.select_parent_artifact(payload, "main", [base, parent])
        self.assertEqual(selected, {"run_id": "11", "sha": base, "fallback": False})

    def test_falls_back_to_nearest_ancestor_artifact(self):
        base, parent, older = "a" * 40, "b" * 40, "c" * 40
        payload = [
            {
                "artifacts": [
                    self.artifact(11, older),
                    self.artifact(12, parent),
                    self.artifact(13, "d" * 40, branch="feature"),
                    self.artifact(14, "e" * 40, expired=True),
                ]
            }
        ]
        selected = parent_finder.select_parent_artifact(
            payload, "main", [base, parent, older]
        )
        self.assertEqual(selected, {"run_id": "12", "sha": parent, "fallback": True})

    def test_no_parent_artifact_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact_list = root / "artifacts.json"
            output = root / "github-output"
            artifact_list.write_text(json.dumps([{"artifacts": []}]), encoding="utf-8")
            stderr = io.StringIO()
            with patch.object(
                parent_finder.subprocess,
                "run",
                return_value=CompletedProcess([], 0, stdout="base\n", stderr=""),
            ), contextlib.redirect_stderr(stderr):
                status = parent_finder.main(
                    [
                        "--merge-base",
                        "base",
                        "--base-branch",
                        "main",
                        "--artifacts",
                        str(artifact_list),
                        "--github-output",
                        str(output),
                    ]
                )
        self.assertEqual(status, 2)
        self.assertIn("no non-expired coverage-summary artifact", stderr.getvalue())
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
