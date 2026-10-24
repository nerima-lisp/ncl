#!/usr/bin/env python3
"""Validate split ownership tables for the retained public surface."""
from __future__ import annotations

import argparse
import csv
import sys
from collections.abc import Sequence
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
KINDS = {"function", "macro", "special-operator", "variable", "constant", "type", "class", "condition", "other"}
HEADER = ("package", "symbol", "kind", "crate", "phase", "direct-expansion", "notes")
SURFACE_HEADER = {"package", "symbol", "present", "external", "kind", "lambda-list"}
INTERNAL_REFERENCE_HEADER = (
    "package",
    "symbol",
    "present",
    "external",
    "kind",
    "lambda-list",
    "libraries",
    "example-file",
    "exported-in-sbcl-2.6.0",
)
INTERNAL_REFERENCE_COUNT = 58
LEGACY_CONTRIBS_HEADER = ("name", "package", "status", "external-count", "reason")
CONTRIBS_HEADER = (
    "name",
    "package",
    "macos-arm64-status",
    "macos-arm64-external-count",
    "macos-arm64-reason",
    "linux-x86_64-status",
    "linux-x86_64-external-count",
    "linux-x86_64-reason",
)
CONTRIB_PLATFORMS = {
    "macos-arm64": (
        "macos-arm64-status",
        "macos-arm64-external-count",
        "macos-arm64-reason",
    ),
    "linux-x86_64": (
        "linux-x86_64-status",
        "linux-x86_64-external-count",
        "linux-x86_64-reason",
    ),
}
PUBLIC_SURFACE_TABLES = {
    "common-lisp.tsv": "COMMON-LISP",
    "sb-ext.tsv": "SB-EXT",
    "sb-thread.tsv": "SB-THREAD",
    "sb-alien.tsv": "SB-ALIEN",
    "sb-sys.tsv": "SB-SYS",
    "sb-mop.tsv": "SB-MOP",
    "sb-gray.tsv": "SB-GRAY",
    "sb-unicode.tsv": "SB-UNICODE",
    "sb-debug.tsv": "SB-DEBUG",
    "internal-referenced.tsv": None,
}
EXPECTED_CONTRIBS = {
    "sb-posix": "SB-POSIX",
    "sb-bsd-sockets": "SB-BSD-SOCKETS",
    "sb-concurrency": "SB-CONCURRENCY",
    "sb-queue": "SB-QUEUE",
    "sb-md5": "SB-MD5",
    "sb-rotate-byte": "SB-ROTATE-BYTE",
    "sb-cltl2": "SB-CLTL2",
    "sb-gmp": "SB-GMP",
    "sb-mpfr": "SB-MPFR",
    "sb-simd": "SB-SIMD",
    "sb-capstone": "SB-CAPSTONE",
    "sb-aclrepl": "SB-ACLREPL",
    "sb-executable": "SB-EXECUTABLE",
    "sb-rt": "SB-RT",
    "sb-grovel": "SB-GROVEL",
    "sb-perf": "SB-PERF",
    "sb-sprof": "SB-SPROF",
    "sb-cover": "SB-COVER",
    "sb-introspect": "SB-INTROSPECT",
    "sb-simple-streams": "SB-SIMPLE-STREAMS",
    "asdf": "ASDF",
    "uiop": "UIOP",
}
OUT_OF_SCOPE_CONTRIBS = {"sb-simd", "sb-perf"}
SB_PUBLIC_PACKAGES = {
    "SB-EXT",
    "SB-THREAD",
    "SB-ALIEN",
    "SB-SYS",
    "SB-MOP",
    "SB-GRAY",
    "SB-UNICODE",
    "SB-DEBUG",
}
KNOWN_SB_PACKAGES = SB_PUBLIC_PACKAGES | {
    package
    for name, package in EXPECTED_CONTRIBS.items()
    if package.startswith("SB-") and name not in OUT_OF_SCOPE_CONTRIBS
}
PACKAGES = {
    "COMMON-LISP",
    "ASDF/INTERFACE",
    "UIOP/DRIVER",
    "NCL-THREADS",
    "NCL-FFI",
    "NCL-MOP",
    "NCL-GRAY",
    "NCL-GC",
    "NCL-IMAGE",
    "NCL-UNICODE",
    "NCL-OS",
    "NCL-EXT",
    "NCL-SYS",
}


def source_symbols() -> set[tuple[str, str]]:
    with (ROOT / "conformance" / "ownership" / "common-lisp.tsv").open(newline="") as stream:
        return {("COMMON-LISP", row["symbol"]) for row in csv.DictReader(stream, delimiter="\t")}


def contrib_metadata(
    *, require_complete_matrix: bool = False
) -> tuple[dict[str, dict[str, str]], list[str]]:
    path = ROOT / "conformance" / "sbcl" / "symbols" / "contribs.tsv"
    if not path.is_file():
        return {}, [f"{path}: contrib metadata is missing"]

    errors: list[str] = []
    metadata: dict[str, dict[str, str]] = {}
    with path.open(newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        header = tuple(reader.fieldnames or ())
        if header not in {LEGACY_CONTRIBS_HEADER, CONTRIBS_HEADER}:
            return {}, [f"{path}: invalid contrib metadata header"]

        for line, source_row in enumerate(reader, 2):
            if None in source_row or any(value is None for value in source_row.values()):
                errors.append(f"{path}:{line}: malformed contrib metadata row")
                continue
            if header == LEGACY_CONTRIBS_HEADER:
                row = {
                    "name": source_row["name"],
                    "package": source_row["package"],
                    "macos-arm64-status": source_row["status"],
                    "macos-arm64-external-count": source_row["external-count"],
                    "macos-arm64-reason": source_row["reason"],
                    "linux-x86_64-status": "",
                    "linux-x86_64-external-count": "",
                    "linux-x86_64-reason": "",
                }
            else:
                row = dict(source_row)

            name, package = row["name"], row["package"]
            if not name or not package:
                errors.append(f"{path}:{line}: empty required contrib metadata field")
                continue
            if name in metadata:
                errors.append(f"{path}:{line}: duplicate contrib metadata row {name}")
                continue
            metadata[name] = row
            if name not in EXPECTED_CONTRIBS:
                errors.append(f"{path}:{line}: unexpected contrib metadata row {name}")
            elif package != EXPECTED_CONTRIBS[name]:
                errors.append(
                    f"{path}:{line}: contrib {name} package is {package}, "
                    f"expected {EXPECTED_CONTRIBS[name]}"
                )

            table_path = path.parent / f"contrib-{name}.tsv"
            loaded_counts: list[tuple[str, int]] = []
            for platform, columns in CONTRIB_PLATFORMS.items():
                status_key, count_key, reason_key = columns
                status, count, reason = row[status_key], row[count_key], row[reason_key]
                if not any((status, count, reason)):
                    if require_complete_matrix:
                        errors.append(
                            f"{path}:{line}: missing {platform} metadata for contrib {name}"
                        )
                    continue
                if not all((status, count, reason)):
                    errors.append(
                        f"{path}:{line}: incomplete {platform} metadata for contrib {name}"
                    )
                    continue
                try:
                    declared_count = int(count)
                    if declared_count < 0:
                        raise ValueError
                except ValueError:
                    errors.append(
                        f"{path}:{line}: invalid {platform} external-count {count}"
                    )
                    declared_count = None

                if status == "loaded":
                    if reason != "-":
                        errors.append(
                            f"{path}:{line}: loaded contrib {name} has {platform} reason {reason}"
                        )
                    if declared_count is not None:
                        loaded_counts.append((platform, declared_count))
                elif status == "out-of-scope":
                    if platform != "macos-arm64" or name not in OUT_OF_SCOPE_CONTRIBS:
                        errors.append(
                            f"{path}:{line}: in-scope contrib {name} is out of scope on {platform}"
                        )
                    if reason != "platform-not-in-scope":
                        errors.append(
                            f"{path}:{line}: invalid out-of-scope reason for {name} on {platform}: {reason}"
                        )
                    if declared_count not in {None, 0}:
                        errors.append(
                            f"{path}:{line}: out-of-scope contrib {name} has {platform} external rows"
                        )
                elif status == "failed":
                    if reason not in {"load-failed", "package-not-found"}:
                        errors.append(
                            f"{path}:{line}: invalid failure reason for {name} on {platform}: {reason}"
                        )
                    if declared_count not in {None, 0}:
                        errors.append(
                            f"{path}:{line}: failed contrib {name} has {platform} external rows"
                        )
                    errors.append(
                        f"{path}:{line}: in-scope contrib {name} failed ({reason}) on {platform}"
                    )
                else:
                    errors.append(
                        f"{path}:{line}: invalid contrib status {status} for {name} on {platform}"
                    )

            if loaded_counts:
                if not table_path.is_file():
                    errors.append(
                        f"{path}:{line}: loaded contrib table is missing: {table_path.name}"
                    )
                else:
                    with table_path.open(newline="") as table_stream:
                        table_reader = csv.DictReader(table_stream, delimiter="\t")
                        fields = set(table_reader.fieldnames or ())
                        if not SURFACE_HEADER <= fields:
                            errors.append(
                                f"{table_path}: invalid surface header for contrib {name}"
                            )
                        else:
                            table_rows = list(table_reader)
                            for platform, declared_count in loaded_counts:
                                if len(table_rows) != declared_count:
                                    errors.append(
                                        f"{path}:{line}: contrib {name} {platform} declares "
                                        f"{declared_count} external rows, table has {len(table_rows)}"
                                    )
                            for table_line, table_row in enumerate(table_rows, 2):
                                if table_row["package"] != package:
                                    errors.append(
                                        f"{table_path}:{table_line}: package is "
                                        f"{table_row['package']}, expected {package}"
                                    )
                                if table_row["present"] != "yes" or table_row["external"] != "yes":
                                    errors.append(
                                        f"{table_path}:{table_line}: loaded contrib rows must be present and external"
                                    )
            elif table_path.exists():
                errors.append(f"{path}:{line}: no platform loads contrib {name}, but its table exists")

    for name in sorted(set(EXPECTED_CONTRIBS) - set(metadata)):
        errors.append(f"{path}: missing contrib metadata row {name}")
    return metadata, errors


def surface_rows(
    *, require_complete_matrix: bool = False
) -> tuple[dict[tuple[str, str], dict[str, str]], list[str]]:
    rows: dict[tuple[str, str], dict[str, str]] = {}
    metadata, errors = contrib_metadata(require_complete_matrix=require_complete_matrix)
    symbols_directory = ROOT / "conformance" / "sbcl" / "symbols"
    for name in sorted(PUBLIC_SURFACE_TABLES):
        if not (symbols_directory / name).is_file():
            errors.append(f"{symbols_directory / name}: required SBCL surface table is missing")
    paths = sorted(
        path
        for path in symbols_directory.glob("*.tsv")
        if path.name != "contribs.tsv"
    )
    expected_contrib_tables = {
        f"contrib-{name}.tsv"
        for name, row in metadata.items()
        if any(row[columns[0]] == "loaded" for columns in CONTRIB_PLATFORMS.values())
    }
    for path in paths:
        if path.name.startswith("contrib-") and path.name not in expected_contrib_tables:
            errors.append(f"{path}: contrib table has no matching loaded metadata row")

    for path in paths:
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            fields = set(reader.fieldnames or ())
            if path.name == "internal-referenced.tsv":
                if tuple(reader.fieldnames or ()) != INTERNAL_REFERENCE_HEADER:
                    errors.append(f"{path}: invalid internal reference header")
                    continue
            if not SURFACE_HEADER <= fields:
                missing = ", ".join(sorted(SURFACE_HEADER - fields))
                errors.append(f"{path}: invalid surface header, missing {missing}")
                continue
            expected_package = PUBLIC_SURFACE_TABLES.get(path.name)
            found_expected_package = False
            row_count = 0
            for line, row in enumerate(reader, 2):
                row_count += 1
                package, symbol = row["package"], row["symbol"]
                present, external, kind = row["present"], row["external"], row["kind"]
                key = (package, symbol)
                if not package or not symbol or not kind:
                    errors.append(f"{path}:{line}: empty required surface field")
                    continue
                if expected_package is not None:
                    if package != expected_package:
                        errors.append(
                            f"{path}:{line}: package is {package}, expected {expected_package}"
                        )
                    else:
                        found_expected_package = True
                    if present != "yes":
                        errors.append(f"{path}:{line}: public surface row must have present=yes")
                    if external != "yes":
                        errors.append(f"{path}:{line}: public surface row must have external=yes")
                if present not in {"yes", "no"}:
                    errors.append(f"{path}:{line}: invalid present value {present}")
                if external not in {"yes", "no"}:
                    errors.append(f"{path}:{line}: invalid external value {external}")
                if any(token not in KINDS for token in kind.split("+")):
                    errors.append(f"{path}:{line}: invalid surface kind {kind}")
                if key in rows:
                    errors.append(f"{path}:{line}: duplicate SBCL surface row {package}::{symbol}")
                    continue
                surface_row = dict(row)
                surface_row["_table"] = path.name
                rows[key] = surface_row
            if expected_package is not None and not found_expected_package:
                errors.append(f"{path}: no surface rows for expected package {expected_package}")
            if path.name == "internal-referenced.tsv" and row_count != INTERNAL_REFERENCE_COUNT:
                errors.append(
                    f"{path}: contains {row_count} rows, expected {INTERNAL_REFERENCE_COUNT}"
                )
    if not rows:
        errors.append("SBCL surface tables contain no symbol rows")
    return rows, errors


def ownership_rows(
    surface: dict[tuple[str, str], dict[str, str]],
) -> tuple[list[tuple[str, str]], list[str]]:
    keys: list[tuple[str, str]] = []
    errors: list[str] = []
    surface_packages = {package for package, _ in surface} | KNOWN_SB_PACKAGES
    for path in sorted((ROOT / "packages").glob("**/ownership.tsv")):
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            if tuple(reader.fieldnames or ()) != HEADER:
                errors.append(f"{path}: invalid header")
                continue
            for line, row in enumerate(reader, 2):
                package, symbol, kind = row["package"], row["symbol"], row["kind"]
                keys.append((package, symbol))
                if package not in PACKAGES and package not in surface_packages:
                    errors.append(f"{path}:{line}: invalid package {package}")
                if not package or not symbol or not row["crate"] or not row["notes"]:
                    errors.append(f"{path}:{line}: empty required field")
                if any(token not in KINDS for token in kind.split("+")):
                    errors.append(f"{path}:{line}: invalid kind {kind}")
                if row["phase"] not in {"1", "2", "3"}:
                    errors.append(f"{path}:{line}: invalid phase {row['phase']}")
                if row["direct-expansion"] not in {"yes", "no"}:
                    errors.append(f"{path}:{line}: invalid direct-expansion")
                if package.startswith("SB-"):
                    display = f"{package}::{symbol} ({kind})"
                    surface_row = surface.get((package, symbol))
                    if surface_row is None or surface_row["present"] != "yes":
                        errors.append(f"{display}: symbol is absent from the SBCL surface")
                    elif set(kind.split("+")) != set(surface_row["kind"].split("+")):
                        errors.append(
                            f"{display}: SBCL surface kind is {surface_row['kind']}"
                        )
    return keys, errors


def print_missing_surface_rows(
    surface: dict[tuple[str, str], dict[str, str]],
    owned: set[tuple[str, str]],
    selected_packages: set[str] | None,
) -> int:
    missing_by_package: dict[str, list[tuple[str, str]]] = {}
    for (package, symbol), row in surface.items():
        if not package.startswith("SB-") or row["present"] != "yes":
            continue
        if row["_table"] != "internal-referenced.tsv" and row["external"] != "yes":
            continue
        if selected_packages is not None and package not in selected_packages:
            continue
        if (package, symbol) not in owned:
            missing_by_package.setdefault(package, []).append((symbol, row["kind"]))

    package_names = sorted(
        package
        for package in ({package for package, _ in surface if package.startswith("SB-")} | KNOWN_SB_PACKAGES)
        if selected_packages is None or package in selected_packages
    )
    total = 0
    for package in package_names:
        missing = sorted(missing_by_package.get(package, []))
        total += len(missing)
        print(f"MISSING {package}: {len(missing)} surface symbols")
        for symbol, kind in missing:
            print(f"  {package}::{symbol} ({kind})")
    print(f"Missing SBCL surface rows: {total}")
    return total


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate ownership tables against retained surfaces.")
    parser.add_argument(
        "--package",
        action="append",
        dest="packages",
        metavar="PACKAGE",
        help="limit the missing SBCL surface report; repeat for multiple packages",
    )
    parser.add_argument(
        "--require-complete-contrib-matrix",
        action="store_true",
        help="require status, external count, and reason for both platforms on every contrib",
    )
    args = parser.parse_args(argv)

    expected = source_symbols()
    surface, surface_errors = surface_rows(
        require_complete_matrix=args.require_complete_contrib_matrix
    )
    actual, ownership_errors = ownership_rows(surface)
    errors = surface_errors + ownership_errors
    surface_packages = {package for package, _ in surface if package.startswith("SB-")} | KNOWN_SB_PACKAGES
    selected_packages = set(args.packages) if args.packages else None
    if selected_packages is not None:
        for package in sorted(selected_packages - surface_packages):
            errors.append(f"unknown SBCL surface package {package}")

    if not actual:
        errors.append("ownership tables are empty")
    if len(actual) != len(set(actual)):
        errors.append("ownership contains duplicate (package, symbol) rows")
    actual_cl = {key for key in actual if key[0] == "COMMON-LISP"}
    if missing := expected - actual_cl:
        errors.append(f"missing {len(missing)} COMMON-LISP symbols: {sorted(missing)[:5]}")
    if extra := actual_cl - expected:
        errors.append(f"extra COMMON-LISP symbols: {sorted(extra)[:5]}")
    expected_surface_cl = {symbol for package, symbol in expected if package == "COMMON-LISP"}
    actual_surface_cl = {symbol for package, symbol in surface if package == "COMMON-LISP"}
    if missing_surface_cl := expected_surface_cl - actual_surface_cl:
        errors.append(f"COMMON-LISP surface is missing symbols: {sorted(missing_surface_cl)}")
    if extra_surface_cl := actual_surface_cl - expected_surface_cl:
        errors.append(f"COMMON-LISP surface has extra symbols: {sorted(extra_surface_cl)}")
    missing_surface_count = print_missing_surface_rows(
        surface,
        set(actual),
        selected_packages,
    )
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print(
        f"OK: {len(actual)} ownership rows cover {len(expected)} retained COMMON-LISP symbols; "
        f"{missing_surface_count} SBCL surface symbols are unowned"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
