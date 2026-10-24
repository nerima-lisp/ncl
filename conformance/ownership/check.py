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
KIND_ORDER = (
    "special-operator",
    "macro",
    "function",
    "variable",
    "constant",
    "class",
    "condition",
    "type",
    "other",
)
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
LINUX_PLATFORM = "linux-x86_64"
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


def platform_table_path(symbols_directory: Path, filename: str, platform: str) -> Path:
    if platform == LINUX_PLATFORM:
        override = symbols_directory / LINUX_PLATFORM / filename
        if override.is_file():
            return override
    return symbols_directory / filename


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

            table_name = f"contrib-{name}.tsv"
            loaded_counts: dict[str, int] = {}
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
                        loaded_counts[platform] = declared_count
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

            symbols_directory = path.parent
            macos_table = symbols_directory / table_name
            linux_table = symbols_directory / LINUX_PLATFORM / table_name
            if row["macos-arm64-status"] != "loaded" and macos_table.exists():
                errors.append(
                    f"{path}:{line}: macOS arm64 does not load contrib {name}, "
                    f"but its baseline table exists"
                )
            if row["linux-x86_64-status"] != "loaded" and linux_table.exists():
                errors.append(
                    f"{path}:{line}: Linux x86_64 does not load contrib {name}, "
                    f"but its platform override exists"
                )
            for platform, declared_count in loaded_counts.items():
                table_path = platform_table_path(symbols_directory, table_name, platform)
                if not table_path.is_file():
                    errors.append(
                        f"{path}:{line}: loaded {platform} contrib table is missing: "
                        f"{table_path.name}"
                    )
                    continue
                if platform == LINUX_PLATFORM and table_path == macos_table:
                    if row["macos-arm64-status"] != "loaded":
                        errors.append(
                            f"{path}:{line}: Linux contrib {name} uses a shared table "
                            "without a loaded macOS baseline"
                        )
                with table_path.open(newline="") as table_stream:
                    table_reader = csv.DictReader(table_stream, delimiter="\t")
                    fields = set(table_reader.fieldnames or ())
                    if not SURFACE_HEADER <= fields:
                        errors.append(
                            f"{table_path}: invalid surface header for contrib {name}"
                        )
                        continue
                    table_rows = list(table_reader)
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

    for name in sorted(set(EXPECTED_CONTRIBS) - set(metadata)):
        errors.append(f"{path}: missing contrib metadata row {name}")
    return metadata, errors


def surface_rows(
    *, require_complete_matrix: bool = False
) -> tuple[dict[tuple[str, str], dict[str, str]], list[str]]:
    metadata, errors = contrib_metadata(require_complete_matrix=require_complete_matrix)
    symbols_directory = ROOT / "conformance" / "sbcl" / "symbols"
    platforms = ["macos-arm64"]
    if any(row["linux-x86_64-status"] for row in metadata.values()):
        platforms.append(LINUX_PLATFORM)

    expected_names = {
        filename
        for filename in PUBLIC_SURFACE_TABLES
    } | {"internal-referenced.tsv"}
    for name, row in metadata.items():
        for platform, (status_key, _, _) in CONTRIB_PLATFORMS.items():
            if row[status_key] == "loaded":
                expected_names.add(f"contrib-{name}.tsv")

    for path in sorted(symbols_directory.glob("*.tsv")):
        if path.name == "contribs.tsv":
            continue
        if path.name not in expected_names:
            errors.append(f"{path}: surface table has no matching loaded metadata row")
    linux_directory = symbols_directory / LINUX_PLATFORM
    if linux_directory.is_dir():
        for path in sorted(linux_directory.glob("*.tsv")):
            linux_contribs = {
                f"contrib-{name}.tsv"
                for name, row in metadata.items()
                if row["linux-x86_64-status"] == "loaded"
            }
            if path.name not in PUBLIC_SURFACE_TABLES and path.name != "internal-referenced.tsv" and path.name not in linux_contribs:
                errors.append(f"{path}: Linux platform override has no matching loaded surface")

    platform_rows: dict[str, dict[tuple[str, str], dict[str, str]]] = {}
    for platform in platforms:
        names = set(PUBLIC_SURFACE_TABLES) | {"internal-referenced.tsv"}
        names.update(
            f"contrib-{name}.tsv"
            for name, row in metadata.items()
            if row[CONTRIB_PLATFORMS[platform][0]] == "loaded"
        )
        rows: dict[tuple[str, str], dict[str, str]] = {}
        for name in sorted(names):
            path = platform_table_path(symbols_directory, name, platform)
            if not path.is_file():
                if name in PUBLIC_SURFACE_TABLES or name == "internal-referenced.tsv":
                    errors.append(f"{path}: required SBCL surface table is missing")
                continue
            with path.open(newline="") as stream:
                reader = csv.DictReader(stream, delimiter="\t")
                fields = set(reader.fieldnames or ())
                if name == "internal-referenced.tsv":
                    if tuple(reader.fieldnames or ()) != INTERNAL_REFERENCE_HEADER:
                        errors.append(f"{path}: invalid internal reference header")
                        continue
                if not SURFACE_HEADER <= fields:
                    missing = ", ".join(sorted(SURFACE_HEADER - fields))
                    errors.append(f"{path}: invalid surface header, missing {missing}")
                    continue
                expected_package = PUBLIC_SURFACE_TABLES.get(name)
                if name.startswith("contrib-"):
                    contrib_name = name.removeprefix("contrib-").removesuffix(".tsv")
                    expected_package = metadata.get(contrib_name, {}).get("package")
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
                        if name in PUBLIC_SURFACE_TABLES:
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
                        errors.append(f"{path}:{line}: duplicate {platform} SBCL surface row {package}::{symbol}")
                        continue
                    surface_row = dict(row)
                    surface_row["_table"] = name
                    surface_row["_platforms"] = {platform}
                    surface_row["_platform_kinds"] = {frozenset(kind.split("+"))}
                    rows[key] = surface_row
                if expected_package is not None and not found_expected_package:
                    errors.append(f"{path}: no surface rows for expected package {expected_package}")
                if name == "internal-referenced.tsv" and row_count != INTERNAL_REFERENCE_COUNT:
                    errors.append(
                        f"{path}: contains {row_count} rows, expected {INTERNAL_REFERENCE_COUNT}"
                    )
        platform_rows[platform] = rows

    rows = {}
    for platform in platforms:
        for key, row in platform_rows.get(platform, {}).items():
            current = rows.get(key)
            if current is None:
                rows[key] = row
                continue
            current["_platforms"].add(platform)
            current["_platform_kinds"].update(row["_platform_kinds"])
            all_kinds = set(current["kind"].split("+")) | set(row["kind"].split("+"))
            current["kind"] = "+".join(kind for kind in KIND_ORDER if kind in all_kinds)
            current["present"] = "yes" if current["present"] == "yes" or row["present"] == "yes" else "no"
            current["external"] = "yes" if current["external"] == "yes" or row["external"] == "yes" else "no"
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
                    elif frozenset(kind.split("+")) not in surface_row["_platform_kinds"]:
                        errors.append(
                            f"{display}: SBCL surface kinds are {surface_row['kind']}"
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
