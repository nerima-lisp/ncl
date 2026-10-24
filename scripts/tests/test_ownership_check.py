import contextlib
import csv
import importlib.util
import io
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


PROJECT_ROOT = Path(__file__).resolve().parents[2]
CHECKER_PATH = PROJECT_ROOT / "conformance" / "ownership" / "check.py"
spec = importlib.util.spec_from_file_location("ownership_check", CHECKER_PATH)
checker = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)


def copy_gate_inputs(destination: Path) -> Path:
    root = destination / "repo"
    ownership_root = root / "packages"
    for source in (PROJECT_ROOT / "packages").glob("**/ownership.tsv"):
        target = ownership_root / source.relative_to(PROJECT_ROOT / "packages")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)

    common_lisp = root / "conformance" / "ownership" / "common-lisp.tsv"
    common_lisp.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(
        PROJECT_ROOT / "conformance" / "ownership" / "common-lisp.tsv",
        common_lisp,
    )
    shutil.copytree(
        PROJECT_ROOT / "conformance" / "sbcl" / "symbols",
        root / "conformance" / "sbcl" / "symbols",
    )
    return root


def write_ownership_row(root: Path, row: dict[str, str]) -> None:
    for path in sorted((root / "packages").glob("**/ownership.tsv")):
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            fields = tuple(reader.fieldnames or ())
            rows = list(reader)
        for existing in rows:
            if (existing["package"], existing["symbol"]) == (row["package"], row["symbol"]):
                existing["kind"] = row["kind"]
                with path.open("w", newline="") as stream:
                    writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                    writer.writeheader()
                    writer.writerows(rows)
                return

    path = root / "packages" / "asdf" / "ownership.tsv"
    contents = path.read_text()
    with path.open("a") as stream:
        if contents and not contents.endswith("\n"):
            stream.write("\n")
        stream.write(
            "\t".join(
                row.get(field, "")
                for field in checker.HEADER
            )
            + "\n"
        )


def remove_ownership_row(root: Path, key: tuple[str, str]) -> None:
    for path in sorted((root / "packages").glob("**/ownership.tsv")):
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            fields = tuple(reader.fieldnames or ())
            rows = list(reader)
        retained = [row for row in rows if (row["package"], row["symbol"]) != key]
        if len(retained) != len(rows):
            with path.open("w", newline="") as stream:
                writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                writer.writeheader()
                writer.writerows(retained)


def update_contrib_metadata(
    root: Path, name: str, platform: str = "macos-arm64", **changes: str
) -> dict[str, str]:
    path = root / "conformance" / "sbcl" / "symbols" / "contribs.tsv"
    with path.open(newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        fields = tuple(reader.fieldnames or ())
        rows = list(reader)
    target = next(row for row in rows if row["name"] == name)
    original = dict(target)
    for field, value in changes.items():
        column = field if field in target else f"{platform}-{field}"
        target[column] = value
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    return original


def write_legacy_contrib_metadata(root: Path) -> None:
    path = root / "conformance" / "sbcl" / "symbols" / "contribs.tsv"
    with path.open(newline="") as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(
            stream,
            fieldnames=checker.LEGACY_CONTRIBS_HEADER,
            delimiter="\t",
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(
            {
                "name": row["name"],
                "package": row["package"],
                "status": row["macos-arm64-status"],
                "external-count": row["macos-arm64-external-count"],
                "reason": row["macos-arm64-reason"],
            }
            for row in rows
        )
    shutil.rmtree(path.parent / checker.LINUX_PLATFORM, ignore_errors=True)


def read_surface_table(path: Path) -> tuple[tuple[str, ...], list[dict[str, str]]]:
    with path.open(newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        return tuple(reader.fieldnames or ()), list(reader)


def write_surface_table(path: Path, rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(
            stream,
            fieldnames=("package", "symbol", "present", "external", "kind", "lambda-list"),
            delimiter="\t",
            lineterminator="\n",
        )
        writer.writeheader()
        writer.writerows(rows)


def sb_ext_surface_row() -> tuple[tuple[str, str], dict[str, str]]:
    rows, errors = checker.surface_rows()
    if errors:
        raise AssertionError(errors)
    for key, row in sorted(rows.items()):
        if key[0] == "SB-EXT" and row["present"] == "yes" and row["external"] == "yes":
            return key, row
    raise AssertionError("the checked-in surface has no public SB-EXT symbol")


class OwnershipCheckTests(unittest.TestCase):
    def test_package_filter_selects_requested_packages(self):
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = checker.main(["--package", "SB-EXT", "--package", "SB-THREAD"])

        self.assertEqual(status, 0, stderr.getvalue())
        self.assertIn("MISSING SB-EXT:", stdout.getvalue())
        self.assertIn("MISSING SB-THREAD:", stdout.getvalue())
        self.assertNotIn("MISSING SB-SYS:", stdout.getvalue())

    def test_common_lisp_surface_must_match_retained_symbols(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            table = root / "conformance" / "sbcl" / "symbols" / "common-lisp.tsv"
            with table.open(newline="") as stream:
                reader = csv.DictReader(stream, delimiter="\t")
                fields = tuple(reader.fieldnames or ())
                rows = list(reader)

            missing_symbol = rows[0]["symbol"]
            with table.open("w", newline="") as stream:
                writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                writer.writeheader()
                writer.writerows(rows[1:])
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main([])

            self.assertEqual(status, 1)
            self.assertIn(
                f"COMMON-LISP surface is missing symbols: ['{missing_symbol}']",
                stderr.getvalue(),
            )

            extra_symbol = "__NCL_EXTRA_CL_SURFACE_TEST__"
            extra_row = dict(rows[0], symbol=extra_symbol)
            with table.open("w", newline="") as stream:
                writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                writer.writeheader()
                writer.writerows([*rows, extra_row])
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main([])

        self.assertEqual(status, 1)
        self.assertIn(
            f"COMMON-LISP surface has extra symbols: ['{extra_symbol}']",
            stderr.getvalue(),
        )

    def test_absent_sb_symbol_fails_with_the_ownership_row(self):
        key, _ = sb_ext_surface_row()
        package = key[0]
        symbol = "__NCL_ABSENT_SURFACE_TEST__"
        self.assertNotIn((package, symbol), checker.surface_rows()[0])

        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            write_ownership_row(
                root,
                {
                    "package": package,
                    "symbol": symbol,
                    "kind": "function",
                    "crate": "ncl-ext",
                    "phase": "1",
                    "direct-expansion": "no",
                    "notes": "adversarial coverage test",
                },
            )
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main(["--package", package])

        self.assertEqual(status, 1)
        self.assertIn(
            f"{package}::{symbol} (function): symbol is absent from the SBCL surface",
            stderr.getvalue(),
        )
        self.assertIn(f"MISSING {package}:", stdout.getvalue())
        self.assertNotIn("MISSING SB-THREAD:", stdout.getvalue())

    def test_unowned_internal_reference_is_reported_without_failing(self):
        rows, errors = checker.surface_rows()
        self.assertEqual(errors, [], errors)
        key, surface_row = next(
            (key, row)
            for key, row in sorted(rows.items())
            if row["_table"] == "internal-referenced.tsv"
            and row["present"] == "yes"
            and row["external"] == "no"
        )

        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            remove_ownership_row(root, key)
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main(["--package", key[0]])

        self.assertEqual(status, 0, stderr.getvalue())
        self.assertIn(f"{key[0]}::{key[1]} ({surface_row['kind']})", stdout.getvalue())

    def test_surface_kind_mismatch_fails_with_the_ownership_row(self):
        (package, symbol), surface_row = sb_ext_surface_row()
        surface_kinds = set(surface_row["kind"].split("+"))
        wrong_kind = next(kind for kind in sorted(checker.KINDS) if kind not in surface_kinds)

        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            write_ownership_row(
                root,
                {
                    "package": package,
                    "symbol": symbol,
                    "kind": wrong_kind,
                    "crate": "ncl-ext",
                    "phase": "1",
                    "direct-expansion": "no",
                    "notes": "adversarial coverage test",
                },
            )
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main(["--package", package])

        self.assertEqual(status, 1)
        self.assertIn(
            f"{package}::{symbol} ({wrong_kind}): SBCL surface kinds are {surface_row['kind']}",
            stderr.getvalue(),
        )

    def test_duplicate_ownership_rows_still_fail(self):
        source = PROJECT_ROOT / "packages" / "asdf" / "ownership.tsv"
        with source.open(newline="") as stream:
            first = next(csv.DictReader(stream, delimiter="\t"))

        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            duplicate_path = root / "packages" / "asdf" / "ownership.tsv"
            with duplicate_path.open("a") as stream:
                if not duplicate_path.read_text().endswith("\n"):
                    stream.write("\n")
                stream.write("\t".join(first[field] for field in checker.HEADER) + "\n")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main([])

        self.assertEqual(status, 1)
        self.assertIn("ownership contains duplicate (package, symbol) rows", stderr.getvalue())

    def test_missing_required_surface_table_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            (root / "conformance" / "sbcl" / "symbols" / "sb-debug.tsv").unlink()
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main([])

        self.assertEqual(status, 1)
        self.assertIn("sb-debug.tsv: required SBCL surface table is missing", stderr.getvalue())

    def test_public_surface_filename_must_match_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            table = root / "conformance" / "sbcl" / "symbols" / "sb-ext.tsv"
            with table.open(newline="") as stream:
                reader = csv.DictReader(stream, delimiter="\t")
                fields = tuple(reader.fieldnames or ())
                rows = list(reader)
            rows[0]["package"] = "SB-WRONG"
            with table.open("w", newline="") as stream:
                writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                writer.writeheader()
                writer.writerows(rows)

            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main([])

        self.assertEqual(status, 1)
        self.assertIn("sb-ext.tsv:2: package is SB-WRONG, expected SB-EXT", stderr.getvalue())

    def test_fixed_public_rows_must_be_present_and_external(self):
        for field in ("present", "external"):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                root = copy_gate_inputs(Path(directory))
                table = root / "conformance" / "sbcl" / "symbols" / "sb-ext.tsv"
                with table.open(newline="") as stream:
                    reader = csv.DictReader(stream, delimiter="\t")
                    fields = tuple(reader.fieldnames or ())
                    rows = list(reader)
                rows[0][field] = "no"
                with table.open("w", newline="") as stream:
                    writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                    writer.writeheader()
                    writer.writerows(rows)

                stdout = io.StringIO()
                stderr = io.StringIO()
                with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                    status = checker.main([])

            self.assertEqual(status, 1)
            self.assertIn(
                f"sb-ext.tsv:2: public surface row must have {field}=yes",
                stderr.getvalue(),
            )

    def test_internal_reference_header_and_row_count_are_fixed(self):
        for mutation in ("header", "row-count"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = copy_gate_inputs(Path(directory))
                table = root / "conformance" / "sbcl" / "symbols" / "internal-referenced.tsv"
                with table.open(newline="") as stream:
                    reader = csv.DictReader(stream, delimiter="\t")
                    fields = tuple(reader.fieldnames or ())
                    rows = list(reader)
                if mutation == "header":
                    fields = fields[:-1]
                    rows = [{field: row[field] for field in fields} for row in rows]
                else:
                    rows = rows[:-1]
                with table.open("w", newline="") as stream:
                    writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
                    writer.writeheader()
                    writer.writerows(rows)

                stdout = io.StringIO()
                stderr = io.StringIO()
                with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                    status = checker.main([])

            self.assertEqual(status, 1)
            if mutation == "header":
                self.assertIn("internal-referenced.tsv: invalid internal reference header", stderr.getvalue())
            else:
                self.assertIn("internal-referenced.tsv: contains 57 rows, expected 58", stderr.getvalue())

    def test_failed_in_scope_contrib_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            original = update_contrib_metadata(
                root,
                "sb-posix",
                status="failed",
                **{"external-count": "0", "reason": "load-failed"},
            )
            self.assertEqual(original["macos-arm64-status"], "loaded")
            (root / "conformance" / "sbcl" / "symbols" / "contrib-sb-posix.tsv").unlink()
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main(["--package", "SB-POSIX"])

        self.assertEqual(status, 1)
        self.assertIn("in-scope contrib sb-posix failed (load-failed)", stderr.getvalue())

    def test_contrib_declared_external_count_must_match_table(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            original = update_contrib_metadata(root, "sb-posix")
            declared_count = int(original["macos-arm64-external-count"]) + 1
            update_contrib_metadata(root, "sb-posix", **{"external-count": str(declared_count)})
            stdout = io.StringIO()
            stderr = io.StringIO()
            with patch.object(checker, "ROOT", root), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = checker.main(["--package", "SB-POSIX"])

        self.assertEqual(status, 1)
        self.assertIn(
            f"contrib sb-posix macos-arm64 declares {declared_count} external rows, "
            f"table has {original['macos-arm64-external-count']}",
            stderr.getvalue(),
        )

    def test_legacy_contrib_metadata_is_read_as_macos_arm64(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            write_legacy_contrib_metadata(root)
            with patch.object(checker, "ROOT", root):
                metadata, errors = checker.contrib_metadata()

        self.assertEqual(errors, [])
        self.assertEqual(metadata["sb-posix"]["macos-arm64-status"], "loaded")
        self.assertEqual(metadata["sb-posix"]["macos-arm64-external-count"], "547")
        self.assertEqual(metadata["sb-posix"]["linux-x86_64-status"], "")

    def test_complete_matrix_rejects_a_blank_platform_triplet(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            update_contrib_metadata(
                root,
                "sb-posix",
                platform="linux-x86_64",
                status="",
                **{"external-count": "", "reason": ""},
            )
            with patch.object(checker, "ROOT", root):
                _, errors = checker.contrib_metadata(require_complete_matrix=True)

        self.assertTrue(
            any("missing linux-x86_64 metadata for contrib sb-posix" in error for error in errors),
            errors,
        )

    def test_linux_uses_shared_table_when_no_override_exists(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            symbols_directory = root / "conformance" / "sbcl" / "symbols"
            table_name = "contrib-sb-posix.tsv"
            override = symbols_directory / checker.LINUX_PLATFORM / table_name
            override.unlink(missing_ok=True)
            _, baseline_rows = read_surface_table(symbols_directory / table_name)
            update_contrib_metadata(
                root,
                "sb-posix",
                platform="linux-x86_64",
                status="loaded",
                **{"external-count": str(len(baseline_rows)), "reason": "-"},
            )
            with patch.object(checker, "ROOT", root):
                _, errors = checker.contrib_metadata()
                resolved = checker.platform_table_path(
                    symbols_directory, table_name, checker.LINUX_PLATFORM
                )

        self.assertEqual(errors, [])
        self.assertEqual(resolved, symbols_directory / table_name)

    def test_same_count_linux_override_tracks_added_and_removed_symbols(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            symbols_directory = root / "conformance" / "sbcl" / "symbols"
            table_name = "contrib-sb-posix.tsv"
            baseline_path = symbols_directory / table_name
            _, baseline_rows = read_surface_table(baseline_path)
            removed_symbol = baseline_rows[0]["symbol"]
            added_symbol = "__NCL_LINUX_ONLY_POSIX_TEST__"
            linux_rows = [dict(row) for row in baseline_rows]
            linux_rows[0]["symbol"] = added_symbol
            override = symbols_directory / checker.LINUX_PLATFORM / table_name
            write_surface_table(override, linux_rows)
            update_contrib_metadata(
                root,
                "sb-posix",
                platform="linux-x86_64",
                status="loaded",
                **{"external-count": str(len(linux_rows)), "reason": "-"},
            )
            with patch.object(checker, "ROOT", root):
                rows, errors = checker.surface_rows()

        self.assertEqual(errors, [])
        self.assertEqual(len(linux_rows), len(baseline_rows))
        self.assertEqual(rows[("SB-POSIX", added_symbol)]["_platforms"], {checker.LINUX_PLATFORM})
        self.assertEqual(rows[("SB-POSIX", removed_symbol)]["_platforms"], {"macos-arm64"})

    def test_linux_only_contrib_uses_an_override_with_macos_out_of_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            symbols_directory = root / "conformance" / "sbcl" / "symbols"
            table_name = "contrib-sb-simd.tsv"
            (symbols_directory / table_name).unlink(missing_ok=True)
            override = symbols_directory / checker.LINUX_PLATFORM / table_name
            row = {
                "package": "SB-SIMD",
                "symbol": "SIMD-TEST",
                "present": "yes",
                "external": "yes",
                "kind": "function",
                "lambda-list": "()",
            }
            write_surface_table(override, [row])
            update_contrib_metadata(
                root,
                "sb-simd",
                status="out-of-scope",
                **{"external-count": "0", "reason": "platform-not-in-scope"},
            )
            update_contrib_metadata(
                root,
                "sb-simd",
                platform=checker.LINUX_PLATFORM,
                status="loaded",
                **{"external-count": "1", "reason": "-"},
            )
            with patch.object(checker, "ROOT", root):
                rows, errors = checker.surface_rows()

        self.assertEqual(errors, [])
        self.assertEqual(rows[("SB-SIMD", "SIMD-TEST")]["_platforms"], {checker.LINUX_PLATFORM})

    def test_linux_contrib_count_mismatch_is_reported_by_platform(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            symbols_directory = root / "conformance" / "sbcl" / "symbols"
            table_path = checker.platform_table_path(
                symbols_directory, "contrib-sb-posix.tsv", checker.LINUX_PLATFORM
            )
            _, table_rows = read_surface_table(table_path)
            wrong_count = len(table_rows) + 1
            update_contrib_metadata(
                root,
                "sb-posix",
                platform="linux-x86_64",
                status="loaded",
                **{"external-count": str(wrong_count), "reason": "-"},
            )
            with patch.object(checker, "ROOT", root):
                _, errors = checker.contrib_metadata()

        self.assertTrue(
            any(
                f"contrib sb-posix linux-x86_64 declares {wrong_count} external rows, "
                f"table has {len(table_rows)}" in error
                for error in errors
            ),
            errors,
        )

    def test_linux_posix_count_mismatch_uses_the_platform_override(self):
        with tempfile.TemporaryDirectory() as directory:
            root = copy_gate_inputs(Path(directory))
            symbols_directory = root / "conformance" / "sbcl" / "symbols"
            baseline_path = symbols_directory / "contrib-sb-posix.tsv"
            _, baseline_rows = read_surface_table(baseline_path)
            linux_rows = [dict(row) for row in baseline_rows]
            for symbol in ("GETRESGID", "GETRESUID"):
                linux_rows.append(
                    {
                        "package": "SB-POSIX",
                        "symbol": symbol,
                        "present": "yes",
                        "external": "yes",
                        "kind": "function",
                        "lambda-list": "()",
                    }
                )
            write_surface_table(
                symbols_directory / checker.LINUX_PLATFORM / "contrib-sb-posix.tsv",
                linux_rows,
            )
            update_contrib_metadata(
                root,
                "sb-posix",
                platform=checker.LINUX_PLATFORM,
                status="loaded",
                **{"external-count": str(len(linux_rows) - 1), "reason": "-"},
            )
            with patch.object(checker, "ROOT", root):
                _, errors = checker.contrib_metadata()

        self.assertTrue(
            any(
                f"contrib sb-posix linux-x86_64 declares {len(linux_rows) - 1} external rows, "
                f"table has {len(linux_rows)}" in error
                for error in errors
            ),
            errors,
        )


if __name__ == "__main__":
    unittest.main()
