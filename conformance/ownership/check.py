#!/usr/bin/env python3
"""Validate split ownership tables for the retained public surface."""
from __future__ import annotations

import csv
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
KINDS = {"function", "macro", "special-operator", "variable", "constant", "type", "class", "condition", "other"}
HEADER = ("package", "symbol", "kind", "crate", "phase", "direct-expansion", "notes")
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
    "NCL-PROFILER",
}


def source_symbols() -> set[tuple[str, str]]:
    with (ROOT / "conformance" / "ownership" / "common-lisp.tsv").open(newline="") as stream:
        return {("COMMON-LISP", row["symbol"]) for row in csv.DictReader(stream, delimiter="\t")}


def ownership_rows() -> tuple[list[tuple[str, str]], list[str]]:
    keys: list[tuple[str, str]] = []
    errors: list[str] = []
    for path in sorted((ROOT / "packages").glob("**/ownership.tsv")):
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            if tuple(reader.fieldnames or ()) != HEADER:
                errors.append(f"{path}: invalid header")
            if tuple(reader.fieldnames or ()) != HEADER:
                continue
            for line, row in enumerate(reader, 2):
                package, symbol, kind = row["package"], row["symbol"], row["kind"]
                keys.append((package, symbol))
                if package not in PACKAGES:
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
                    errors.append(f"{path}:{line}: SB package is forbidden")
    return keys, errors


def main() -> int:
    expected = source_symbols()
    actual, errors = ownership_rows()
    if not actual:
        errors.append("ownership tables are empty")
    if len(actual) != len(set(actual)):
        errors.append("ownership contains duplicate (package, symbol) rows")
    actual_cl = {key for key in actual if key[0] == "COMMON-LISP"}
    if missing := expected - actual_cl:
        errors.append(f"missing {len(missing)} COMMON-LISP symbols: {sorted(missing)[:5]}")
    if extra := actual_cl - expected:
        errors.append(f"extra COMMON-LISP symbols: {sorted(extra)[:5]}")
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"OK: {len(actual)} ownership rows cover {len(expected)} retained symbols")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
