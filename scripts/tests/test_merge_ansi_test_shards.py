import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
import ansi_test_inclusion as inclusion
import merge_ansi_test_shards as merger


POLICIES = ("safety-3", "speed-3-safety-1")
SHARD_COUNT = 2
SOURCE_REVISION = "revision-a"


def _names_by_shard():
    names = [[] for _ in range(SHARD_COUNT)]
    candidate = 0
    while any(not shard_names for shard_names in names):
        name = f"SYNTHETIC.{candidate}"
        names[inclusion.shard_for_test(name, SHARD_COUNT)].append(name)
        candidate += 1
    return names


def _rows(names):
    return [{"name": name, "chapter": "test", "status": "passed"} for name in names]


def _counts(rows):
    return {
        "passed": sum(row["status"] == "passed" for row in rows),
        "failed": 0,
        "error": 0,
        "timeout": 0,
        "harness_error": 0,
        "unexecuted": 0,
    }


def _write_reports(artifacts_dir):
    names_by_shard = _names_by_shard()
    manifest = [
        {"name": name, "chapter": "test"}
        for shard_names in names_by_shard
        for name in shard_names
    ]
    active_total = len(manifest)
    removed = len(inclusion.REMOVED_TEST_NAMES)

    for policy in POLICIES:
        for shard_index, names in enumerate(names_by_shard):
            artifact = artifacts_dir / f"ansi-shard-{shard_index}-{policy}"
            artifact.mkdir(parents=True)
            shard_date = "2026-11-07" if shard_index == 0 else "2026-11-08"
            baseline_rows = _rows(names)
            baseline = {
                "schema_version": 3,
                "suite": "ansi-test",
                "implementation": "sbcl",
                "runtime_version": f"SBCL {inclusion.SBCL_VERSION}",
                "policy": policy,
                "platform": "Linux x86_64",
                "measured_on": shard_date,
                "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
                "source_revision": SOURCE_REVISION,
                "registered": active_total + removed,
                "active": len(baseline_rows),
                "active_total": active_total,
                "disabled": 0,
                "disabled_tests": [],
                "removed": removed,
                **_counts(baseline_rows),
                "status": "complete",
                "tests": baseline_rows,
                "manifest": manifest,
                "process_exit_status": 0,
                "shard_index": shard_index,
                "shard_count": SHARD_COUNT,
            }
            (artifact / "sbcl-report.json").write_text(
                json.dumps(baseline), encoding="utf-8",
            )

            comparison_rows = _rows(names)
            expected = {name: "test" for name in names}
            comparison = inclusion.compare_results(
                expected, {row["name"]: row for row in comparison_rows},
            )
            report = {
                "schema_version": 3,
                "suite": "ansi-test",
                "implementation": "ncl",
                "runtime_version": "ncl 0.1.1",
                "policy": policy,
                "platform": "Linux x86_64",
                "measured_on": "2026-11-08",
                "source_commit": inclusion.SOURCES["ansi-test"]["commit"],
                "source_revision": SOURCE_REVISION,
                "registered": len(comparison_rows),
                "active": len(comparison_rows),
                **_counts(comparison_rows),
                "status": "complete",
                "tests": comparison_rows,
                "comparison": comparison,
                "shard_index": shard_index,
                "shard_count": SHARD_COUNT,
            }
            (artifact / "ncl-report.json").write_text(
                json.dumps(report), encoding="utf-8",
            )
            (artifact / "ncl-exit-status").write_text("0\n", encoding="utf-8")


class AnsiShardMergeTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.root = Path(self.temporary_directory.name)
        self.artifacts_dir = self.root / "artifacts"
        self.artifacts_dir.mkdir()
        _write_reports(self.artifacts_dir)

    def _merge(self, policy):
        return merger.merge_shards(
            self.artifacts_dir,
            SHARD_COUNT,
            self.root / f"baseline-{policy}",
            self.root / f"baseline-report-{policy}.json",
            self.root / f"comparison-report-{policy}.json",
            policy,
        )

    def test_shards_from_different_dates_merge_and_share_the_earliest_run_date(self):
        merged = {policy: self._merge(policy) for policy in POLICIES}

        self.assertEqual(merged["safety-3"][0]["measured_on"], "2026-11-07")
        self.assertEqual(merged["safety-3"][1]["measured_on"], "2026-11-07")
        self.assertEqual(merged["speed-3-safety-1"][0]["measured_on"], "2026-11-07")
        self.assertEqual(merged["speed-3-safety-1"][1]["measured_on"], "2026-11-07")
        self.assertEqual(
            merged["safety-3"][1]["measured_on"],
            merged["speed-3-safety-1"][1]["measured_on"],
        )
        self.assertEqual(merged["safety-3"][0]["source_revision"], SOURCE_REVISION)
        self.assertEqual(merged["safety-3"][1]["source_revision"], SOURCE_REVISION)

    def _make_dates_same(self):
        for report_path in self.artifacts_dir.glob("ansi-shard-*/*.json"):
            report = json.loads(report_path.read_text(encoding="utf-8"))
            report["measured_on"] = "2026-11-08"
            report_path.write_text(json.dumps(report), encoding="utf-8")

    def test_platform_mismatch_is_rejected(self):
        self._make_dates_same()
        report_path = self.artifacts_dir / "ansi-shard-1-safety-3" / "sbcl-report.json"
        report = json.loads(report_path.read_text(encoding="utf-8"))
        report["platform"] = "macOS arm64"
        report_path.write_text(json.dumps(report), encoding="utf-8")

        with self.assertRaisesRegex(ValueError, "platform"):
            self._merge("safety-3")

    def test_runtime_version_mismatch_is_rejected(self):
        self._make_dates_same()
        report_path = self.artifacts_dir / "ansi-shard-1-safety-3" / "ncl-report.json"
        report = json.loads(report_path.read_text(encoding="utf-8"))
        report["runtime_version"] = "ncl 0.1.2"
        report_path.write_text(json.dumps(report), encoding="utf-8")

        with self.assertRaisesRegex(ValueError, "runtime version"):
            self._merge("safety-3")

    def test_source_revision_mismatch_is_rejected(self):
        self._make_dates_same()
        report_path = self.artifacts_dir / "ansi-shard-1-safety-3" / "ncl-report.json"
        report = json.loads(report_path.read_text(encoding="utf-8"))
        report["source_revision"] = "revision-b"
        report_path.write_text(json.dumps(report), encoding="utf-8")

        with self.assertRaisesRegex(ValueError, "source revision"):
            self._merge("safety-3")


class AnsiShardReportGenerationTests(unittest.TestCase):
    def test_report_records_the_source_revision(self):
        report = inclusion._measurement(
            "ncl", [sys.executable], Path(__file__), [], 0, "", "",
            source_revision=SOURCE_REVISION,
        )

        self.assertEqual(report["schema_version"], 3)
        self.assertEqual(report["source_revision"], SOURCE_REVISION)


if __name__ == "__main__":
    unittest.main()
