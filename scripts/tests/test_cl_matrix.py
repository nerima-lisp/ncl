import csv
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check_cl_matrix as checker


def base_row(symbol):
    return {
        "package": "COMMON-LISP", "symbol": symbol, "present": "yes", "external": "yes",
        "kind": "function", "lambda-list": "-",
    }


def metadata_row(symbol="FOO", kinds="function", refs="ANSI:TEST-FOO"):
    return {
        "package": "COMMON-LISP", "symbol": symbol, "clhs_kinds": kinds,
        "clhs_chapters": "3", "test_refs": refs,
    }


def observation(**overrides):
    values = {
        "present": "YES", "external": "YES", "fboundp": "YES", "macro_function": "NO",
        "special_operator_p": "NO", "boundp": "NO", "constantp": "NO", "find_class": "NO",
        "type_valid": "NOT-APPLICABLE", "condition": "NOT-APPLICABLE",
    }
    values.update(overrides)
    return values


class CheckClMatrixTests(unittest.TestCase):
    def test_valid_matrix_matches_runtime(self):
        base = [base_row("FOO")]
        metadata = [metadata_row()]
        runtime = {"FOO": observation()}
        matrix, errors = checker.assemble_matrix(base, metadata, runtime)
        self.assertEqual(errors, [])
        self.assertEqual(matrix[0]["implementation_status"], "implemented")
        self.assertEqual(checker.validate_implemented_refs(matrix), [])
        self.assertEqual(checker.check_matrix_rows(base, metadata, matrix, runtime), [])

    def test_matrix_runtime_mismatch_fails(self):
        base = [base_row("FOO")]
        metadata = [metadata_row()]
        runtime = {"FOO": observation()}
        matrix, _ = checker.assemble_matrix(base, metadata, runtime)
        matrix[0]["ncl_kinds"] = "-"
        errors = checker.check_matrix_rows(base, metadata, matrix, runtime)
        self.assertTrue(any("matrix ncl_kinds" in error for error in errors), errors)

    def test_empty_function_cell_cannot_be_claimed_implemented(self):
        base = [base_row("FOO")]
        metadata = [metadata_row()]
        runtime = {"FOO": observation(fboundp="NO")}
        matrix, _ = checker.assemble_matrix(base, metadata, runtime)
        self.assertEqual(matrix[0]["implementation_status"], "missing")
        self.assertEqual(checker.empty_function_cells(metadata, runtime), ["FOO"])
        matrix[0]["implementation_status"] = "implemented"
        matrix[0]["ncl_kinds"] = "function"
        errors = checker.check_matrix_rows(base, metadata, matrix, runtime)
        self.assertTrue(any("callable cell while fboundp is NO" in error for error in errors), errors)
        self.assertEqual(checker.empty_function_cells(metadata, {"FOO": observation(present="NO", fboundp="NO")}), [])

    def test_implemented_row_requires_test_reference(self):
        rows, errors = checker.assemble_matrix([base_row("FOO")], [metadata_row(refs="-")], {"FOO": observation()})
        self.assertEqual(errors, [])
        self.assertEqual(rows[0]["implementation_status"], "partial")
        self.assertIn("missing-test-reference", rows[0]["status_notes"])
        rows[0]["implementation_status"] = "implemented"
        self.assertTrue(any("implemented symbol has no test reference" in error
                            for error in checker.validate_implemented_refs(rows)))

    def test_metadata_checks_chapters_and_test_references(self):
        base = [base_row("FOO")]
        metadata = [metadata_row()]
        checked, errors = checker.validate_metadata(base, metadata, {"TEST-FOO"})
        self.assertIn(("COMMON-LISP", "FOO"), checked)
        self.assertEqual(errors, [])
        invalid = [metadata_row() | {"clhs_chapters": "0", "test_refs": "ANSI:DOES-NOT-EXIST"}]
        _, errors = checker.validate_metadata(base, invalid, {"TEST-FOO"})
        self.assertTrue(any("invalid CLHS chapter" in error for error in errors), errors)
        self.assertTrue(any("ANSI test reference does not exist" in error for error in errors), errors)

    def test_978_symbol_coverage_is_checked_independently(self):
        symbols = [f"SYMBOL-{index:03}" for index in range(978)]
        rows = [base_row(symbol) for symbol in symbols]
        with tempfile.TemporaryDirectory() as temporary:
            inventory = Path(temporary) / "ownership.tsv"
            with inventory.open("w", newline="", encoding="utf-8") as stream:
                writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
                writer.writerow(["symbol"])
                writer.writerows([[symbol] for symbol in symbols])
            self.assertEqual(checker.validate_symbol_count(rows, inventory), [])
            rows[-1] = base_row("SYMBOL-000")
            errors = checker.validate_symbol_count(rows, inventory)
            self.assertTrue(any("duplicate symbols" in error for error in errors), errors)
            self.assertTrue(any("symbol set differs" in error for error in errors), errors)

    def test_ncl_probe_result_fields_are_parsed(self):
        metadata = [metadata_row()]
        result = CompletedProcess([], 0, '(("FOO" :YES :YES :YES :NO :NO :NO :NO :NO :NOT-APPLICABLE :NOT-APPLICABLE))', "")
        with patch.object(checker.subprocess, "run", return_value=result) as run:
            parsed = checker.probe_ncl(Path("/fake/ncl"), metadata)
        self.assertEqual(parsed["FOO"]["external"], "YES")
        self.assertEqual(parsed["FOO"]["fboundp"], "YES")
        self.assertEqual(parsed["FOO"]["macro_function"], "NO")
        self.assertIn("subtypep", run.call_args.args[0][2])
        self.assertEqual(checker.observed_kinds(parsed["FOO"]), {"function"})

    def test_ncl_probe_rejects_impossible_symbol_flags(self):
        metadata = [metadata_row()]
        outputs = (
            '(("FOO" :NO :YES :NO :NO :NO :NO :NO :NO :NOT-APPLICABLE :NOT-APPLICABLE))',
            '(("FOO" :NO :NO :YES :NO :NO :NO :NO :NO :NOT-APPLICABLE :NOT-APPLICABLE))',
        )
        for output in outputs:
            with self.subTest(output=output), patch.object(
                checker.subprocess, "run", return_value=CompletedProcess([], 0, output, "")
            ):
                with self.assertRaises(checker.MatrixError):
                    checker.probe_ncl(Path("/fake/ncl"), metadata)

    def test_explicit_call_probe_reports_unimplemented_errors(self):
        result = CompletedProcess([], 1, "", "Unsupported function")
        with patch.object(checker.subprocess, "run", return_value=result):
            passed, unimplemented, message = checker.run_call_probe(Path("/fake/ncl"), "FOO", "(foo)")
        self.assertFalse(passed)
        self.assertTrue(unimplemented)
        self.assertIn("unimplemented/unsupported", message)

    def test_failed_call_probe_does_not_write_matrix(self):
        base = [base_row("FOO")]
        metadata = [metadata_row()]
        runtime = {"FOO": observation()}
        generated, errors = checker.assemble_matrix(base, metadata, runtime)
        self.assertEqual(errors, [])
        with (
            patch.object(checker, "read_tsv", side_effect=[(checker.BASE_FIELDS, base), (checker.METADATA_FIELDS, metadata)]),
            patch.object(checker, "validate_symbol_count", return_value=[]),
            patch.object(checker, "read_ansi_names", return_value={"TEST-FOO"}),
            patch.object(checker, "probe_ncl", return_value=runtime),
            patch.object(checker, "run_call_probe", return_value=(False, True, "FOO: unsupported")),
            patch.object(checker, "write_tsv") as write_tsv,
            redirect_stderr(io.StringIO()),
        ):
            result = checker.main([
                "--ncl", "/fake/ncl", "--matrix", "matrix.tsv", "--metadata", "metadata.tsv",
                "--ownership", "ownership.tsv", "--ansi-index", "ansi.tsv", "--write",
                "--probe-call", "FOO=(foo)",
            ])
        self.assertEqual(result, 1)
        write_tsv.assert_not_called()

    def test_call_probe_must_directly_call_the_named_symbol(self):
        self.assertEqual(checker.call_probe_arg("FOO=(foo 1)"), ("FOO", "(foo 1)"))
        for symbol in ("=", "/=", "<=", ">="):
            form = f"({symbol} 1 1)"
            self.assertEqual(checker.call_probe_arg(f"{symbol}={form}"), (symbol, form))
        with self.assertRaises(checker.argparse.ArgumentTypeError):
            checker.call_probe_arg("FOO=(+ 1 2)")


if __name__ == "__main__":
    unittest.main()
