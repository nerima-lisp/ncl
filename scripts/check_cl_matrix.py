#!/usr/bin/env python3
"""Generate or validate the COMMON-LISP runtime status matrix."""
from __future__ import annotations

import argparse
import csv
import json
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
MATRIX_PATH = ROOT / "conformance/sbcl/symbols/common-lisp.tsv"
METADATA_PATH = ROOT / "conformance/sbcl/common-lisp-metadata.tsv"
OWNERSHIP_PATH = ROOT / "conformance/ownership/common-lisp.tsv"
ANSI_INDEX_PATH = ROOT / "conformance/baselines/ansi-test-sbcl-2.6.0-macos-arm64.tsv"
BASE_FIELDS = ("package", "symbol", "present", "external", "kind", "lambda-list")
METADATA_FIELDS = ("package", "symbol", "clhs_kinds", "clhs_chapters", "test_refs")
MATRIX_FIELDS = BASE_FIELDS + (
    "clhs_kinds", "clhs_chapters", "implementation_status", "status_notes", "test_refs", "ncl_kinds"
)
PROBE_FIELDS = (
    "present", "external", "fboundp", "macro_function", "special_operator_p", "boundp", "constantp",
    "find_class", "type_valid", "condition",
)
KINDS = {
    "function", "macro", "special-operator", "variable", "constant", "type", "class", "condition",
    "declaration", "lambda-list-keyword", "other",
}
KIND_ORDER = (
    "special-operator", "macro", "function", "variable", "constant", "class", "condition", "type",
    "declaration", "lambda-list-keyword", "other",
)
OBSERVABLE_KINDS = set(KIND_ORDER[:8])
STATUS_VALUES = {"implemented", "partial", "missing"}
TOKEN_RE = re.compile(r'"(?:\\.|[^"\\])*"|[()]|[^()\s]+')
UNIMPLEMENTED_RE = re.compile(r"unimplemented|not implemented|unsupported", re.IGNORECASE)
MAX_OUTPUT_BYTES = 2_000_000
MAX_SEXP_TOKENS = 100_000
MAX_SEXP_DEPTH = 32


class MatrixError(Exception):
    pass


def read_tsv(path: Path, required: tuple[str, ...], exact: bool = False) -> tuple[tuple[str, ...], list[dict[str, str]]]:
    try:
        with path.open(newline="", encoding="utf-8") as stream:
            reader = csv.DictReader(stream, delimiter="\t", strict=True)
            fields = tuple(reader.fieldnames or ())
            if exact and fields != required:
                raise MatrixError(f"{path}: expected header {required}, got {fields}")
            if len(fields) != len(set(fields)):
                raise MatrixError(f"{path}: duplicate header field")
            if not set(required) <= set(fields):
                raise MatrixError(f"{path}: missing columns {sorted(set(required) - set(fields))}")
            rows = []
            for line, row in enumerate(reader, 2):
                if None in row or any(value is None for value in row.values()):
                    raise MatrixError(f"{path}:{line}: malformed TSV row")
                rows.append({key: value for key, value in row.items() if key is not None})
    except (OSError, csv.Error) as error:
        raise MatrixError(f"{path}: {error}") from error
    return fields, rows


def key_for(row: dict[str, str]) -> tuple[str, str]:
    return row["package"], row["symbol"]


def unique_rows(rows: list[dict[str, str]], label: str) -> tuple[dict[tuple[str, str], dict[str, str]], list[str]]:
    by_key: dict[tuple[str, str], dict[str, str]] = {}
    errors = []
    for row in rows:
        key = key_for(row)
        if not all(key):
            errors.append(f"{label}: empty package or symbol")
        if key in by_key:
            errors.append(f"{label}: duplicate {key[0]}::{key[1]}")
        else:
            by_key[key] = row
    return by_key, errors


def split_list(value: str, separator: str, label: str) -> tuple[list[str], list[str]]:
    if not value:
        return [], [f"{label}: empty value"]
    if value == "-":
        return [], []
    parts = value.split(separator)
    errors = []
    if any(not part for part in parts):
        errors.append(f"{label}: empty item")
    if len(parts) != len(set(parts)):
        errors.append(f"{label}: duplicate item")
    return parts, errors


def read_ansi_names(path: Path) -> set[str]:
    _, rows = read_tsv(path, ("name", "chapter"))
    names = {row["name"] for row in rows}
    if not names:
        raise MatrixError(f"{path}: ANSI reference index is empty")
    return names


def validate_symbol_count(rows: list[dict[str, str]], ownership_path: Path) -> list[str]:
    _, ownership_rows = read_tsv(ownership_path, ("symbol",), exact=True)
    expected = [row["symbol"] for row in ownership_rows]
    actual = [row["symbol"] for row in rows]
    errors = []
    if len(rows) != 978:
        errors.append(f"expected 978 COMMON-LISP rows, got {len(rows)}")
    if len(expected) != len(set(expected)):
        errors.append("symbol inventory contains duplicate entries")
    if len(actual) != len(set(actual)):
        errors.append("matrix source contains duplicate symbols")
    if set(actual) != set(expected):
        errors.append(f"symbol set differs from inventory (missing={len(set(expected)-set(actual))}, extra={len(set(actual)-set(expected))})")
    for row in rows:
        if row["package"] != "COMMON-LISP" or row["present"] != "yes" or row["external"] != "yes":
            errors.append(f"{row['symbol']}: source is not an external COMMON-LISP symbol")
    return errors


def validate_metadata(
    base_rows: list[dict[str, str]], metadata_rows: list[dict[str, str]], ansi_names: set[str], root: Path = ROOT
) -> tuple[dict[tuple[str, str], dict[str, str]], list[str]]:
    base, errors = unique_rows(base_rows, "matrix source")
    metadata, meta_errors = unique_rows(metadata_rows, "metadata")
    errors.extend(meta_errors)
    if set(base) != set(metadata):
        errors.append(f"metadata symbol keys differ (missing={len(set(base)-set(metadata))}, extra={len(set(metadata)-set(base))})")
    for key, row in metadata.items():
        if key[0] != "COMMON-LISP":
            errors.append(f"metadata: {key[0]}::{key[1]} is outside COMMON-LISP")
        kinds, kind_errors = split_list(row["clhs_kinds"], "+", f"{key[1]} clhs_kinds")
        errors.extend(kind_errors)
        if not kinds:
            errors.append(f"{key[1]}: at least one CLHS kind is required")
        for kind in kinds:
            if kind not in KINDS:
                errors.append(f"{key[1]}: unknown CLHS kind {kind}")
        chapters, chapter_errors = split_list(row["clhs_chapters"], ";", f"{key[1]} clhs_chapters")
        errors.extend(chapter_errors)
        if not chapters:
            errors.append(f"{key[1]}: at least one CLHS chapter is required")
        for chapter in chapters:
            if not chapter.isdecimal() or not 1 <= int(chapter) <= 26:
                errors.append(f"{key[1]}: invalid CLHS chapter {chapter}")
        refs, ref_errors = split_list(row["test_refs"], ";", f"{key[1]} test_refs")
        errors.extend(ref_errors)
        for ref in refs:
            if ref.startswith("ANSI:"):
                if ref.removeprefix("ANSI:") not in ansi_names:
                    errors.append(f"{key[1]}: ANSI test reference does not exist: {ref}")
            else:
                path = PurePosixPath(ref.split("#", 1)[0])
                if path.is_absolute() or ".." in path.parts or not (root / path).is_file():
                    errors.append(f"{key[1]}: invalid test path reference: {ref}")
    return metadata, errors


def lisp_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def probe_source(metadata_rows: list[dict[str, str]]) -> str:
    names = [row["symbol"] for row in metadata_rows]
    typed = [row["symbol"] for row in metadata_rows if set(row["clhs_kinds"].split("+")) & {"type", "class", "condition"}]
    classes = [row["symbol"] for row in metadata_rows if set(row["clhs_kinds"].split("+")) & {"class", "condition"}]
    conditions = [row["symbol"] for row in metadata_rows if "condition" in row["clhs_kinds"].split("+")]
    quoted = lambda values: "(" + " ".join(lisp_string(value) for value in values) + ")"
    return f"""(let ((names '{quoted(names)}) (type-names '{quoted(typed)})
      (class-names '{quoted(classes)}) (condition-names '{quoted(conditions)}))
  (mapcar (lambda (name)
    (multiple-value-bind (symbol status) (find-symbol name \"COMMON-LISP\")
      (list name
        (if symbol :yes :no)
        (if (eq status :external) :yes :no)
        (if (and symbol (funcall 'fboundp symbol)) :yes :no)
        (if (not (funcall 'fboundp 'macro-function)) :unavailable
            (if (and symbol (funcall 'macro-function symbol)) :yes :no))
        (if (not (funcall 'fboundp 'special-operator-p)) :unavailable
            (if (and symbol (funcall 'special-operator-p symbol)) :yes :no))
        (if (and symbol (funcall 'boundp symbol)) :yes :no)
        (if (not (funcall 'fboundp 'constantp)) :unavailable
            (if (and symbol (funcall 'constantp symbol)) :yes :no))
        (if (not (member name class-names :test #'string=)) :not-applicable
            (if (not (funcall 'fboundp 'find-class)) :unavailable
                (multiple-value-bind (class error) (ignore-errors (funcall 'find-class symbol nil))
                  (cond (error :unavailable) (class :yes) (t :no)))))
        (if (not (member name type-names :test #'string=)) :not-applicable
            (if (not (funcall 'fboundp 'subtypep)) :unavailable
                (if symbol (multiple-value-bind (value valid) (funcall 'subtypep symbol 't)
                    (if valid :yes :no)) :no)))
        (if (not (member name condition-names :test #'string=)) :not-applicable
            (if (not (funcall 'fboundp 'subtypep)) :unavailable
                (if symbol (multiple-value-bind (value valid) (funcall 'subtypep symbol 'condition)
                    (if (and valid value) :yes :no)) :no)))))) names))"""


def parse_lisp_result(output: str) -> list[Any]:
    if len(output.encode("utf-8")) > MAX_OUTPUT_BYTES:
        raise MatrixError(f"NCL probe output exceeds {MAX_OUTPUT_BYTES} bytes")
    tokens = TOKEN_RE.findall(output)
    if len(tokens) > MAX_SEXP_TOKENS:
        raise MatrixError(f"NCL probe output exceeds {MAX_SEXP_TOKENS} tokens")
    if re.sub(r"\s+", "", output) != re.sub(r"\s+", "", "".join(tokens)):
        raise MatrixError("NCL probe output contains invalid Lisp syntax")
    index = 0

    def parse(depth: int = 0) -> Any:
        nonlocal index
        if depth > MAX_SEXP_DEPTH or index >= len(tokens):
            raise MatrixError("NCL probe output is too deep or incomplete")
        token = tokens[index]
        index += 1
        if token == "(":
            values = []
            while index < len(tokens) and tokens[index] != ")":
                values.append(parse(depth + 1))
            if index == len(tokens):
                raise MatrixError("NCL probe output has an unclosed list")
            index += 1
            return values
        if token == ")":
            raise MatrixError("NCL probe output has an unexpected close parenthesis")
        if token.startswith('"'):
            try:
                return json.loads(token)
            except json.JSONDecodeError as error:
                raise MatrixError(f"NCL probe output has an invalid string: {error}") from error
        return token[1:].upper() if token.startswith(":") else token.upper()

    result = parse()
    if index != len(tokens) or not isinstance(result, list):
        raise MatrixError("NCL probe output must contain one list")
    return result


def probe_ncl(ncl_path: Path, metadata_rows: list[dict[str, str]], timeout: int = 120) -> dict[str, dict[str, str]]:
    try:
        result = subprocess.run(
            [str(ncl_path), "--eval", probe_source(metadata_rows)], capture_output=True, text=True,
            encoding="utf-8", errors="strict", timeout=timeout, check=False,
        )
    except (OSError, subprocess.TimeoutExpired, UnicodeError) as error:
        raise MatrixError(f"could not run NCL probe: {error}") from error
    if result.returncode:
        detail = result.stderr.strip().splitlines()
        raise MatrixError(f"NCL probe exited {result.returncode}: {detail[-1][:300] if detail else 'no diagnostic'}")
    records = parse_lisp_result(result.stdout)
    if len(records) != len(metadata_rows):
        raise MatrixError(f"NCL probe returned {len(records)} rows, expected {len(metadata_rows)}")
    observations = {}
    allowed = {"YES", "NO", "UNAVAILABLE", "NOT-APPLICABLE"}
    for record in records:
        if not isinstance(record, list) or len(record) != len(PROBE_FIELDS) + 1 or not isinstance(record[0], str):
            raise MatrixError("NCL probe returned a malformed symbol record")
        name = record[0]
        if name in observations:
            raise MatrixError(f"NCL probe returned duplicate symbol {name}")
        observation = dict(zip(PROBE_FIELDS, record[1:]))
        if any(value not in allowed for value in observation.values()):
            raise MatrixError(f"NCL probe returned an invalid flag for {name}")
        if observation["external"] == "YES" and observation["present"] != "YES":
            raise MatrixError(f"NCL probe returned external-but-absent symbol {name}")
        if observation["present"] == "NO" and any(
            value == "YES" for field, value in observation.items() if field not in {"present", "external"}
        ):
            raise MatrixError(f"NCL probe returned a binding for absent symbol {name}")
        observations[name] = observation
    expected = {row["symbol"] for row in metadata_rows}
    if set(observations) != expected:
        raise MatrixError(f"NCL symbol set differs (missing={len(expected-set(observations))}, extra={len(set(observations)-expected)})")
    return observations


def observed_kinds(observation: dict[str, str]) -> set[str]:
    kinds = set()
    if observation["macro_function"] == "YES":
        kinds.add("macro")
    elif observation["fboundp"] == "YES":
        kinds.add("function")
    for field, kind in (("special_operator_p", "special-operator"), ("find_class", "class"),
                        ("type_valid", "type"), ("condition", "condition")):
        if observation[field] == "YES":
            kinds.add(kind)
    if observation["constantp"] == "YES":
        kinds.add("constant")
    elif observation["boundp"] == "YES":
        kinds.add("variable")
    return kinds


def classify_row(row: dict[str, str], observation: dict[str, str]) -> tuple[str, str, str]:
    expected = set(row["clhs_kinds"].split("+"))
    observed = observed_kinds(observation)
    expected_observable = expected & OBSERVABLE_KINDS
    missing = expected_observable - observed
    probes = {
        "function": "fboundp", "macro": "macro_function", "special-operator": "special_operator_p",
        "variable": "boundp", "constant": "constantp", "class": "find_class",
        "type": "type_valid", "condition": "condition",
    }
    unavailable = {kind for kind in expected_observable if observation[probes[kind]] == "UNAVAILABLE"}
    unprobed = expected - OBSERVABLE_KINDS
    if observation["external"] != "YES":
        status, notes = "missing", ["symbol-not-external"] + (["missing-test-reference"] if row["test_refs"] == "-" else [])
    elif missing or unavailable or unprobed or row["test_refs"] == "-":
        status = "partial" if observed & expected_observable or unavailable or unprobed or row["test_refs"] == "-" and not missing else "missing"
        notes = []
        if missing:
            notes.append("missing=" + "+".join(kind for kind in KIND_ORDER if kind in missing))
        if unavailable:
            notes.append("unavailable=" + "+".join(kind for kind in KIND_ORDER if kind in unavailable))
        if unprobed:
            notes.append("unprobed=" + "+".join(kind for kind in KIND_ORDER if kind in unprobed))
        if row["test_refs"] == "-":
            notes.append("missing-test-reference")
    else:
        status, notes = "implemented", []
    kinds = "+".join(kind for kind in KIND_ORDER if kind in observed) or "-"
    return status, ";".join(notes) or "-", kinds


def assemble_matrix(
    base_rows: list[dict[str, str]], metadata_rows: list[dict[str, str]], observations: dict[str, dict[str, str]]
) -> tuple[list[dict[str, str]], list[str]]:
    metadata, errors = unique_rows(metadata_rows, "metadata")
    base, base_errors = unique_rows(base_rows, "matrix source")
    errors.extend(base_errors)
    if set(base) != set(metadata):
        errors.append("matrix source and metadata symbol sets differ")
    output = []
    for source in base_rows:
        key = key_for(source)
        meta, observation = metadata.get(key), observations.get(key[1])
        if meta is None or observation is None:
            continue
        status, notes, kinds = classify_row(meta, observation)
        output.append({**{field: source[field] for field in BASE_FIELDS}, **{
            "clhs_kinds": meta["clhs_kinds"], "clhs_chapters": meta["clhs_chapters"],
            "implementation_status": status, "status_notes": notes, "test_refs": meta["test_refs"], "ncl_kinds": kinds,
        }})
    return output, errors


def empty_function_cells(metadata_rows: list[dict[str, str]], observations: dict[str, dict[str, str]]) -> list[str]:
    empty = []
    for row in metadata_rows:
        kinds = set(row["clhs_kinds"].split("+"))
        if kinds & {"function", "macro"} and "special-operator" not in kinds:
            if observations[row["symbol"]]["present"] == "YES" and observations[row["symbol"]]["fboundp"] != "YES":
                empty.append(row["symbol"])
    return empty


def validate_implemented_refs(rows: list[dict[str, str]]) -> list[str]:
    errors = []
    for row in rows:
        if row.get("implementation_status") not in STATUS_VALUES:
            errors.append(f"{row['symbol']}: invalid implementation status")
        refs, ref_errors = split_list(row.get("test_refs", ""), ";", f"{row['symbol']} test_refs")
        errors.extend(ref_errors)
        if row.get("implementation_status") == "implemented" and not refs:
            errors.append(f"{row['symbol']}: implemented symbol has no test reference")
    return errors


def check_matrix_rows(
    base_rows: list[dict[str, str]], metadata_rows: list[dict[str, str]], matrix_rows: list[dict[str, str]],
    observations: dict[str, dict[str, str]],
) -> list[str]:
    expected_rows, errors = assemble_matrix(base_rows, metadata_rows, observations)
    matrix, duplicate_errors = unique_rows(matrix_rows, "matrix")
    errors.extend(duplicate_errors)
    expected, expected_errors = unique_rows(expected_rows, "generated matrix")
    errors.extend(expected_errors)
    if set(matrix) != set(expected):
        errors.append("matrix symbol set differs from the 978-symbol source")
    for key, expected_row in expected.items():
        actual = matrix.get(key)
        if actual is None:
            continue
        for field in MATRIX_FIELDS:
            if actual.get(field) != expected_row[field]:
                errors.append(f"{key[1]}: matrix {field} is {actual.get(field)!r}, expected {expected_row[field]!r}")
        kinds = set(expected_row["clhs_kinds"].split("+"))
        if actual["implementation_status"] == "implemented" and kinds & {"function", "macro"}:
            if observations[key[1]]["fboundp"] != "YES":
                errors.append(f"{key[1]}: implemented row claims a callable cell while fboundp is NO")
    errors.extend(validate_implemented_refs(matrix_rows))
    return errors


def write_tsv(path: Path, rows: list[dict[str, str]]) -> None:
    with tempfile.NamedTemporaryFile("w", newline="", encoding="utf-8", dir=path.parent, delete=False) as stream:
        temporary = Path(stream.name)
        writer = csv.DictWriter(stream, fieldnames=MATRIX_FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
        stream.flush()
    temporary.replace(path)


def run_call_probe(ncl_path: Path, symbol: str, form: str, timeout: int = 10) -> tuple[bool, bool, str]:
    """Run one caller-supplied expression in isolation; it may have side effects."""
    try:
        result = subprocess.run(
            [str(ncl_path), "--eval", form], capture_output=True, text=True, encoding="utf-8", errors="strict",
            timeout=timeout, check=False,
        )
    except (OSError, subprocess.TimeoutExpired, UnicodeError) as error:
        return False, False, f"{symbol}: call probe failed to run: {error}"
    if result.returncode and UNIMPLEMENTED_RE.search(result.stderr + result.stdout):
        return False, True, f"{symbol}: explicit call returned an unimplemented/unsupported error"
    if result.returncode:
        return False, False, f"{symbol}: explicit call exited {result.returncode}"
    return True, False, f"{symbol}: explicit call completed"


def call_probe_arg(value: str) -> tuple[str, str]:
    symbol, separator, tail = value.partition("=(")
    form = "(" + tail if separator else ""
    if not separator or not form or not re.fullmatch(r"[A-Z0-9*+/<>=!?&%$-]+", symbol.upper()):
        raise argparse.ArgumentTypeError("expected SYMBOL=LISP_FORM with a COMMON-LISP symbol name")
    match = re.match(r"\s*\(\s*([^\s()]+)(?:\s|\))", form)
    if not match or match[1].upper() != symbol.upper():
        raise argparse.ArgumentTypeError("LISP_FORM must directly call SYMBOL")
    return symbol.upper(), form


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate or validate the COMMON-LISP implementation matrix.")
    parser.add_argument("--ncl", required=True, type=Path, help="path to an NCL binary to query")
    parser.add_argument("--matrix", type=Path, default=MATRIX_PATH)
    parser.add_argument("--metadata", type=Path, default=METADATA_PATH)
    parser.add_argument("--ownership", type=Path, default=OWNERSHIP_PATH)
    parser.add_argument("--ansi-index", type=Path, default=ANSI_INDEX_PATH)
    parser.add_argument("--write", action="store_true", help="regenerate the enriched matrix")
    parser.add_argument("--probe-call", action="append", type=call_probe_arg, default=[], metavar="SYMBOL=LISP_FORM",
                        help="direct caller-supplied call expression; may have side effects")
    parser.add_argument("--timeout", type=int, default=120, help="NCL introspection timeout in seconds")
    args = parser.parse_args(argv)
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    try:
        fields, source_rows = read_tsv(args.matrix, BASE_FIELDS)
        if args.write and fields not in (BASE_FIELDS, MATRIX_FIELDS):
            raise MatrixError(f"{args.matrix}: --write accepts only base or enriched matrix columns")
        if not args.write and fields != MATRIX_FIELDS:
            raise MatrixError(f"{args.matrix}: check mode requires enriched matrix columns")
        _, metadata_rows = read_tsv(args.metadata, METADATA_FIELDS, exact=True)
        errors = validate_symbol_count(source_rows, args.ownership)
        ansi_names = read_ansi_names(args.ansi_index)
        _, metadata_errors = validate_metadata(source_rows, metadata_rows, ansi_names)
        errors.extend(metadata_errors)
        if errors:
            raise MatrixError("\n".join(errors[:40]))
        observations = probe_ncl(args.ncl, metadata_rows, args.timeout)
        generated, errors = assemble_matrix(source_rows, metadata_rows, observations)
        errors.extend(validate_implemented_refs(generated))
        if errors:
            raise MatrixError("\n".join(errors[:40]))
        checked = generated if args.write else source_rows
        if not args.write:
            errors = check_matrix_rows(source_rows, metadata_rows, source_rows, observations)
            if errors:
                raise MatrixError("\n".join(errors[:40]))
        for symbol, form in args.probe_call:
            if symbol not in observations or observations[symbol]["fboundp"] != "YES":
                raise MatrixError(f"call probe requires an fbound COMMON-LISP symbol: {symbol}")
            passed, unimplemented, message = run_call_probe(args.ncl, symbol, form)
            print(message, file=sys.stderr)
            if unimplemented or not passed:
                raise MatrixError(message)
        if args.write:
            write_tsv(args.matrix, generated)
        totals = Counter(row["implementation_status"] for row in checked)
        print(f"OK: {'generated' if args.write else 'checked'} {len(checked)} COMMON-LISP symbols "
              f"(implemented={totals['implemented']}, partial={totals['partial']}, missing={totals['missing']})")
        empty = empty_function_cells(metadata_rows, observations)
        if empty:
            print("EMPTY FUNCTION CELLS (fboundp=no): " + ", ".join(empty), file=sys.stderr)
        print("CALL-PROBE GAP: no general fbound-call sweep is safe; --probe-call tests only a caller-supplied expression.",
              file=sys.stderr)
        return 0
    except MatrixError as error:
        print(f"check_cl_matrix.py: {error}", file=sys.stderr)
        return 1
if __name__ == "__main__":
    raise SystemExit(main())
