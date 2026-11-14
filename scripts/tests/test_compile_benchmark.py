from __future__ import annotations

import io
import stat
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "scripts"))

from compile_benchmark import main as compile_benchmark_main
from bench.corpus.compile import (
    BenchmarkError,
    _pass_profile,
    _sample_file,
    _timed_seconds,
    _write_compile_driver,
    run_benchmark,
    select_sources,
)
from bench.corpus.extract_tar import _download, _validate_source_url
from bench.corpus.fetch import CorpusError, SourceFile, _extract_tar, _manifest, _member_list_sha256
from bench.corpus.generate_synthetic import generate_source
from bench.corpus.supervise import MAX_OUTPUT_BYTES, run as supervise


def _archive(entries: list[tuple[str, bytes | None, bytes | None]]) -> bytes:
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for name, kind, payload in entries:
            info = tarfile.TarInfo(name)
            if kind is None:
                info.type = tarfile.DIRTYPE
                info.mode = 0o755
                info.size = 0
                archive.addfile(info)
            elif kind == b"symlink":
                info.type = tarfile.SYMTYPE
                info.linkname = "../outside"
                archive.addfile(info)
            elif kind == b"hardlink":
                info.type = tarfile.LNKTYPE
                info.linkname = "root/source.lisp"
                archive.addfile(info)
            elif kind == b"device":
                info.type = tarfile.CHRTYPE
                archive.addfile(info)
            elif kind == b"fifo":
                info.type = tarfile.FIFOTYPE
                archive.addfile(info)
            else:
                assert payload is not None
                info.type = tarfile.REGTYPE
                info.mode = 0o644
                info.size = len(payload)
                archive.addfile(info, io.BytesIO(payload))
    return output.getvalue()


def _member_hash(payload: bytes) -> str:
    with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
        return _member_list_sha256(archive.getmembers())


class FakeHarness:
    def __init__(
        self,
        *,
        include_profile_diagnostics: bool = True,
        peak_rss_bytes: int | None = 4096,
        seconds_per_compile: float = 0.5,
        zero_first_sample: bool = False,
    ) -> None:
        self.commands: list[list[str]] = []
        self.drivers: list[str] = []
        self.include_profile_diagnostics = include_profile_diagnostics
        self.peak_rss_bytes = peak_rss_bytes
        self.seconds_per_compile = seconds_per_compile
        self.zero_first_sample = zero_first_sample

    def __call__(self, command, timeout):
        args = list(command)
        self.commands.append(args)
        executable = Path(args[0]).name
        repetitions = 1
        if "--script" in args:
            driver = Path(args[-1]).read_text(encoding="utf-8")
            self.drivers.append(driver)
            repetitions = driver.count("(compile-file #P")
        base = {
            "status": "passed",
            "return_code": 0,
            "timed_out": False,
            "wall_seconds": 0.25,
            "peak_rss_bytes": self.peak_rss_bytes,
            "stdout": "",
            "stderr": "",
            "output_truncated": False,
        }
        if args[-1:] == ["--version"]:
            base["stdout"] = "ncl 0.1.0\n" if executable in {"ncl", "ncl-profiler"} else "SBCL 2.6.10\n"
        elif "--script" in args:
            ticks = (
                0
                if self.zero_first_sample and len(self.drivers) == 1
                else round(self.seconds_per_compile * 1_000 * repetitions)
            )
            base["stdout"] = f"__NCL_BENCH_RESULT__|2|1000|{ticks}|{repetitions}\n"
            if "--opt-diagnostics" in args and self.include_profile_diagnostics:
                base["stderr"] = (
                    'ncl-opt iteration=1 function="entry" pass="sccp" '
                    "changed=true elapsed_ns=120 verification=passed allocated_bytes=256\n"
                )
        return base


class CorpusExtractionTests(unittest.TestCase):
    def test_sbcl_download_accepts_only_https_sourceforge_hosts(self):
        _validate_source_url("https://downloads.sourceforge.net/project/file.tar.bz2")
        _validate_source_url("https://mirror.dl.sourceforge.net/project/file.tar.bz2")
        for url in (
            "http://downloads.sourceforge.net/project/file.tar.bz2",
            "https://downloads.sourceforge.net.attacker.invalid/project/file.tar.bz2",
            "https://user@downloads.sourceforge.net/project/file.tar.bz2",
            "https://downloads.sourceforge.net:444/project/file.tar.bz2",
            "https://downloads.sourceforge.net:invalid/project/file.tar.bz2",
        ):
            with self.subTest(url=url), self.assertRaises(CorpusError):
                _validate_source_url(url)

    def test_sbcl_download_enforces_compressed_limit_during_read(self):
        class FakeResponse:
            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return None

            def geturl(self):
                return "https://downloads.sourceforge.net/file.tar.bz2"

            def read(self, _size):
                return b"12345"

        class FakeOpener:
            def open(self, _request, timeout):
                self.timeout = timeout
                return FakeResponse()

        with patch("bench.corpus.extract_tar.MAX_ARCHIVE_BYTES", 4):
            with patch("bench.corpus.extract_tar.build_opener", return_value=FakeOpener()):
                with self.assertRaisesRegex(CorpusError, "compressed-size limit"):
                    _download(
                        "https://downloads.sourceforge.net/file.tar.bz2",
                        "0" * 64,
                    )

    def test_manifest_pins_every_external_source_with_sha256(self):
        sources = _manifest()
        self.assertEqual(set(sources), {"ansi-test", "alexandria", "cl-ppcre", "asdf"})
        expected = {
            "ansi-test": ("ca06bd919661af162c67407c9d994e881870bdb3", "e445359ea3879797dac9f3d7c057564c4bba11917f6b9bd377be57b32ba15151"),
            "alexandria": ("20241012-git", "dcdfc5bac229a952358dfe661f0860e8a02480e406d0d95dee4300cdc3d45b31"),
            "cl-ppcre": ("20250622-git", "a9d6a9ca9eb862283f6079856629c8b19254f108fcde065d61f280e7c2c6ec39"),
            "asdf": ("3.3.7", "874e8f235ea633a090c7ab2665b1f624d1910eb7f6fc10349a596085c1cbb189"),
        }
        expected_member_lists = {
            "ansi-test": "a103764020a9acbb3c6f60ad197769892b371d208738198d49c561e9618f7235",
            "alexandria": "2348f3a5d8bc87be3c9aa7063a56cf8ec64d25799c5cc74446f5d8121d0d231f",
            "cl-ppcre": "34d908aac2d3ec114be01695be273c9854bfd38934869df685a2440c7c4be7b6",
        }
        for name, (version, digest) in expected.items():
            with self.subTest(source=name):
                self.assertEqual(sources[name]["version"], version)
                self.assertEqual(sources[name]["sha256"], digest)
                self.assertRegex(digest, r"^[0-9a-f]{64}$")
                if name in expected_member_lists:
                    self.assertEqual(sources[name]["member_list_sha256"], expected_member_lists[name])

    def test_safe_archive_extracts_only_its_pinned_root(self):
        payload = _archive([
            ("root", None, None),
            ("root/source.lisp", b"file", b"(defun corpus-source () t)\n"),
        ])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            extracted = _extract_tar(
                payload,
                destination,
                top_level="root",
                max_unpacked_bytes=1024,
                expected_member_list_sha256=_member_hash(payload),
            )
            self.assertEqual((extracted / "source.lisp").read_text(), "(defun corpus-source () t)\n")
            self.assertEqual(sorted(path.name for path in extracted.iterdir()), ["source.lisp"])
            self.assertEqual(stat.S_IMODE(extracted.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE((extracted / "source.lisp").stat().st_mode), 0o600)

    def test_safe_archive_accepts_an_implicit_pinned_root(self):
        payload = _archive([
            ("root/source.lisp", b"file", b"(defun corpus-source () t)\n"),
        ])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            extracted = _extract_tar(
                payload,
                destination,
                top_level="root",
                max_unpacked_bytes=1024,
                expected_member_list_sha256=_member_hash(payload),
            )
            self.assertEqual((extracted / "source.lisp").read_text(), "(defun corpus-source () t)\n")

    def test_archive_rejects_empty_archives(self):
        payload = _archive([])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            with self.assertRaisesRegex(CorpusError, "archive is empty"):
                _extract_tar(
                    payload,
                    destination,
                    top_level="root",
                    max_unpacked_bytes=1024,
                    expected_member_list_sha256=_member_hash(payload),
                )
            self.assertFalse(destination.exists())

    def test_archive_rejects_duplicated_root_directories(self):
        payload = _archive([
            ("root", None, None),
            ("root", None, None),
            ("root/source.lisp", b"file", b"safe"),
        ])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            with self.assertRaisesRegex(CorpusError, "root member is invalid or duplicated"):
                _extract_tar(
                    payload,
                    destination,
                    top_level="root",
                    max_unpacked_bytes=1024,
                    expected_member_list_sha256=_member_hash(payload),
                )
            self.assertFalse(destination.exists())

    def test_archive_rejects_unsafe_members_before_writing_any_file(self):
        invalid_members = [
            ("/absolute/source.lisp", b"file", b"bad"),
            ("outside/source.lisp", b"file", b"bad"),
            ("root/../escape.lisp", b"file", b"bad"),
            ("root/./source.lisp", b"file", b"bad"),
            ("root//empty-component.lisp", b"file", b"bad"),
            ("root/\\escape.lisp", b"file", b"bad"),
            ("C:/escape.lisp", b"file", b"bad"),
            ("root/valid.lisp", b"file", b"duplicate"),
            ("root/link", b"symlink", None),
            ("root/hardlink", b"hardlink", None),
            ("root/device", b"device", None),
            ("root/fifo", b"fifo", None),
        ]
        for invalid in invalid_members:
            with self.subTest(member=invalid[0]):
                payload = _archive([
                    ("root", None, None),
                    ("root/valid.lisp", b"file", b"safe"),
                    invalid,
                ])
                with tempfile.TemporaryDirectory() as temporary:
                    destination = Path(temporary) / "source"
                    with self.assertRaises(CorpusError):
                        _extract_tar(
                            payload,
                            destination,
                            top_level="root",
                            max_unpacked_bytes=1024,
                            expected_member_list_sha256=_member_hash(payload),
                        )
                    self.assertFalse((destination / "valid.lisp").exists())
                    self.assertFalse(destination.exists())
                    self.assertFalse((Path(temporary) / "escape.lisp").exists())

    def test_archive_rejects_unexpected_member_paths_before_writing(self):
        expected = _archive([
            ("root", None, None),
            ("root/source.lisp", b"file", b"safe"),
        ])
        payload = _archive([
            ("root", None, None),
            ("root/source.lisp", b"file", b"safe"),
            ("root/extra.txt", b"file", b"unexpected"),
        ])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            with self.assertRaisesRegex(CorpusError, "member list"):
                _extract_tar(
                    payload,
                    destination,
                    top_level="root",
                    max_unpacked_bytes=1024,
                    expected_member_list_sha256=_member_hash(expected),
                )
            self.assertFalse(destination.exists())

    def test_archive_removes_partial_private_tree_when_actual_member_bytes_exceed_header(self):
        payload = _archive([
            ("root", None, None),
            ("root/source.lisp", b"file", b"safe"),
        ])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            with patch("bench.corpus.fetch.tarfile.TarFile.extractfile", return_value=io.BytesIO(b"oversized")):
                with self.assertRaisesRegex(CorpusError, "unpacked-size limit"):
                    _extract_tar(
                        payload,
                        destination,
                        top_level="root",
                        max_unpacked_bytes=1024,
                        expected_member_list_sha256=_member_hash(payload),
                    )
            self.assertFalse(destination.exists())

    def test_archive_enforces_declared_unpacked_byte_budget_before_extraction(self):
        payload = _archive([
            ("root", None, None),
            ("root/source.lisp", b"file", b"123456"),
        ])
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "source"
            with self.assertRaisesRegex(CorpusError, "unpacked-size limit"):
                _extract_tar(
                    payload,
                    destination,
                    top_level="root",
                    max_unpacked_bytes=5,
                    expected_member_list_sha256=_member_hash(payload),
                )
            self.assertFalse((destination / "source.lisp").exists())
            self.assertFalse(destination.exists())


class CompileBenchmarkTests(unittest.TestCase):
    def test_synthetic_source_is_deterministic_and_has_requested_defun_count(self):
        default_source = generate_source()
        self.assertEqual(len(default_source.splitlines()), 2_000)
        self.assertEqual(default_source.count("(defun "), 2_000)
        source = generate_source(3)
        self.assertEqual(source, generate_source(3))
        self.assertEqual(source.count("(defun "), 3)
        self.assertIn("compile-bench-0002", source)
        with self.assertRaises(ValueError):
            generate_source(0)

    def test_light_mode_keeps_one_source_for_each_required_group(self):
        sources = [
            SourceFile(group, "1", "sample.lisp", Path(group) / "sample.lisp")
            for group in ("ansi-test", "alexandria", "cl-ppcre", "asdf", "synthetic")
        ]
        self.assertEqual([item.suite for item in select_sources(sources, "light")], ["alexandria", "ansi-test", "asdf", "cl-ppcre", "synthetic"])
        self.assertEqual(len(select_sources(sources, "full")), 5)

    def test_full_corpus_rejects_missing_groups_and_duplicate_paths(self):
        with self.assertRaisesRegex(BenchmarkError, "required set"):
            select_sources([], "full")
        source = SourceFile("synthetic", "test", "one.lisp", Path("one.lisp"))
        sources = [
            SourceFile(group, "1", "sample.lisp", Path(group) / "sample.lisp")
            for group in ("ansi-test", "alexandria", "cl-ppcre", "asdf")
        ] + [source, source]
        with self.assertRaisesRegex(BenchmarkError, "duplicate source"):
            select_sources(sources, "full")

    def test_compile_driver_and_internal_timer_output_are_specific(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'lisp"file.lisp'
            driver = root / "driver.lisp"
            driver_source = SourceFile("synthetic", "test", source.name, source)
            source.write_text("(defun f () 1)\n", encoding="utf-8")
            _write_compile_driver(driver, source)
            content = driver.read_text(encoding="utf-8")
            self.assertIn("get-internal-real-time", content)
            self.assertIn("compile-file #P", content)
            self.assertIn('lisp\\"file.lisp', content)
            self.assertEqual(_timed_seconds("__NCL_BENCH_RESULT__|2|100|50|2\n"), 0.25)
            self.assertIsNone(_timed_seconds("no timer marker"))
            self.assertIsNone(_timed_seconds("__NCL_BENCH_RESULT__|2|100|50|2\n__NCL_BENCH_RESULT__|2|100|50|2\n"))
            self.assertEqual(driver_source.label, "synthetic/lisp\"file.lisp")

    def test_compile_sample_retries_static_forms_until_minimum_duration(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_path = root / "source.lisp"
            source_path.write_text("(defun f () 1)\n", encoding="utf-8")
            source = SourceFile("synthetic", "test", source_path.name, source_path)
            driver = root / "driver.lisp"
            harness = FakeHarness(seconds_per_compile=0.025, zero_first_sample=True)

            sample = _sample_file(
                "ncl",
                "ncl",
                source,
                driver,
                1.0,
                supervisor=harness,
            )

        self.assertEqual(sample["status"], "passed")
        self.assertEqual(sample["compile_repetitions"], 4)
        self.assertAlmostEqual(sample["compile_elapsed_seconds"], 0.1)
        self.assertAlmostEqual(sample["compile_seconds"], 0.025)
        self.assertEqual(
            [driver.count("(compile-file #P") for driver in harness.drivers],
            [1, 2, 4],
        )
        self.assertTrue(all("(loop" not in driver for driver in harness.drivers))

    def test_pass_profile_summarizes_time_and_allocation_fields(self):
        profile = _pass_profile(
            'ncl-opt iteration=1 function="entry" pass="sccp" changed=true elapsed_ns=100 allocated_bytes=256\n'
            'ncl-opt iteration=1 function="entry" pass="dce" changed=false elapsed_ns=20 allocated_bytes=64\n',
            (),
        )
        self.assertEqual(profile["status"], "measured")
        self.assertTrue(profile["allocation_bytes_available"])
        self.assertEqual(len(profile["passes"]), 2)
        self.assertEqual(profile["top_10"][0]["pass"], "sccp")
        self.assertEqual(profile["top_10"][0]["allocated_bytes"], 256)

    def test_pass_profile_rejects_malformed_or_incomplete_records(self):
        profile = _pass_profile(
            'ncl-opt pass="sccp" elapsed_ns=10\n',
            (),
        )
        self.assertEqual(profile["status"], "unavailable")
        self.assertFalse(profile["allocation_bytes_available"])

    def test_runner_emits_compile_time_throughput_rss_and_pass_profile(self):
        harness = FakeHarness()
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.dict("os.environ", {"NCL_BENCH_REVISION": "a" * 40}),
        ):
            root = Path(temporary)
            executables = {}
            for name in ("ncl", "sbcl"):
                path = root / name
                path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
                path.chmod(0o755)
                executables[name] = path
            profiler = root / "ncl-profiler"
            profiler.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            profiler.chmod(0o755)

            def fetcher(cache_dir: Path, work_dir: Path) -> list[SourceFile]:
                groups = ("ansi-test", "alexandria", "cl-ppcre", "asdf", "synthetic")
                result = []
                for group in groups:
                    path = work_dir / f"{group}.lisp"
                    path.write_text("(defun f () 1)\n", encoding="utf-8")
                    result.append(SourceFile(group, "test", path.name, path))
                return result

            report = run_benchmark(
                executables["ncl"],
                executables["sbcl"],
                "light",
                ncl_profiler=profiler,
                cache_dir=root / "cache",
                timeout=1,
                fetcher=fetcher,
                supervisor=harness,
            )
            report_without_profiler = run_benchmark(
                executables["ncl"],
                executables["sbcl"],
                "light",
                sample_runs=1,
                cache_dir=root / "cache-without-profiler",
                timeout=1,
                fetcher=fetcher,
                supervisor=FakeHarness(),
            )
            report_with_missing_profile_data = run_benchmark(
                executables["ncl"],
                executables["sbcl"],
                "light",
                ncl_profiler=profiler,
                sample_runs=1,
                cache_dir=root / "cache-missing-profiler-data",
                timeout=1,
                fetcher=fetcher,
                supervisor=FakeHarness(include_profile_diagnostics=False),
            )
            report_with_missing_rss = run_benchmark(
                executables["ncl"],
                executables["sbcl"],
                "light",
                sample_runs=1,
                cache_dir=root / "cache-missing-rss",
                timeout=1,
                fetcher=fetcher,
                supervisor=FakeHarness(peak_rss_bytes=None),
            )

        self.assertEqual(report["status"], "complete")
        self.assertEqual(report["sample_runs"], 5)
        self.assertEqual(len(report["run_order"]), 30)
        self.assertEqual(sum(row["phase"] == "compile" for row in report["run_order"]), 25)
        self.assertEqual(sum(row["phase"] == "startup" for row in report["run_order"]), 5)
        self.assertEqual(report["corpus_file_count"], 5)
        self.assertEqual(report["measurement"]["ncl_sha"], "a" * 40)
        self.assertEqual(report["measurement"]["sbcl_version"], "2.6.10")
        ncl = report["runtimes"]["ncl"]
        self.assertEqual(ncl["compiled_file_count"], 5)
        self.assertAlmostEqual(ncl["total_compile_seconds"], 2.5)
        self.assertAlmostEqual(ncl["minimum_total_compile_seconds"], 2.5)
        self.assertEqual(len(ncl["total_compile_seconds_samples"]), 5)
        self.assertEqual(ncl["lines_per_second"], 2.0)
        self.assertEqual(ncl["peak_rss_bytes"], 4096)
        self.assertEqual(ncl["peak_rss_sample_count"], 25)
        self.assertEqual(ncl["peak_rss_expected_sample_count"], 25)
        self.assertTrue(ncl["peak_rss_available"])
        self.assertEqual(ncl["binary_size_bytes"], len("#!/bin/sh\nexit 0\n"))
        self.assertEqual(len(ncl["files"][0]["compile_seconds_samples"]), 5)
        self.assertEqual(ncl["files"][0]["sample_count"], 5)
        self.assertEqual(ncl["files"][0]["compile_repetitions_samples"], [1] * 5)
        self.assertEqual(len(ncl["startup"]["samples"]), 5)
        self.assertEqual(ncl["startup"]["minimum_seconds"], 0.25)
        first_file = ncl["files"][0]
        first_label = f"{first_file['suite']}/{first_file['file']}"
        pair_orders = [
            row["order"] for row in report["run_order"]
            if row["phase"] == "compile" and row["file"] == first_label
        ]
        self.assertEqual(pair_orders, [["ncl", "sbcl"], ["sbcl", "ncl"], ["ncl", "sbcl"], ["sbcl", "ncl"], ["ncl", "sbcl"]])
        self.assertEqual(report["pass_profile"]["status"], "measured")
        self.assertTrue(report["pass_profile"]["allocation_bytes_available"])
        compile_commands = [command for command in harness.commands if "--script" in command]
        self.assertEqual(len(compile_commands), 51)
        self.assertTrue(all("compile-file #P" in content for content in harness.drivers))
        self.assertIn("--opt-diagnostics", compile_commands[-1])
        self.assertEqual(Path(compile_commands[-1][0]).name, "ncl-profiler")
        self.assertEqual(report["allocation_metric"]["basis"], "requested Rust allocation bytes")
        self.assertEqual(report_without_profiler["status"], "complete")
        self.assertEqual(report_without_profiler["pass_profile"]["status"], "not-configured")
        self.assertEqual(report_with_missing_profile_data["status"], "partial")
        self.assertEqual(report_with_missing_rss["status"], "partial")

    def test_partial_report_is_written_and_runner_succeeds(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "compile.json"
            with patch("compile_benchmark.run_benchmark", return_value={"status": "partial"}):
                status = compile_benchmark_main([
                    "--ncl", "ncl", "--sbcl", "sbcl", "--mode", "light", "--output", str(output)
                ])
            self.assertEqual(status, 0)
            self.assertEqual(output.read_text(encoding="utf-8"), '{\n  "status": "partial"\n}\n')

    def test_error_report_is_written_and_runner_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "compile.json"
            with patch("compile_benchmark.run_benchmark", return_value={"status": "error"}):
                status = compile_benchmark_main([
                    "--ncl", "ncl", "--sbcl", "sbcl", "--mode", "light", "--output", str(output)
                ])
            self.assertEqual(status, 1)
            self.assertEqual(output.read_text(encoding="utf-8"), '{\n  "status": "error"\n}\n')


class SupervisorTests(unittest.TestCase):
    def test_capture_limit_does_not_limit_compiler_artifacts(self):
        artifact_size = MAX_OUTPUT_BYTES + 4096
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "compiled.fasl"
            script = (
                "import pathlib,sys; "
                "pathlib.Path(sys.argv[1]).write_bytes(b'x'*int(sys.argv[2])); "
                "sys.stdout.write('x'*int(sys.argv[2]))"
            )
            result = supervise(
                [sys.executable, "-c", script, str(artifact), str(artifact_size)],
                timeout=10,
            )
            produced_size = artifact.stat().st_size

        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["return_code"], 0)
        self.assertEqual(produced_size, artifact_size)
        self.assertEqual(len(result["stdout"].encode("utf-8")), MAX_OUTPUT_BYTES)
        self.assertTrue(result["output_truncated"])


if __name__ == "__main__":
    unittest.main()
