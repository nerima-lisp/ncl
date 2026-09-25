#!/usr/bin/env python3
"""Validate split ownership tables for the retained public surface."""
from __future__ import annotations

import csv
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
KINDS = {"function", "macro", "special-operator", "variable", "constant", "type", "class", "condition", "other"}
HEADER = ("package", "symbol", "kind", "crate", "phase", "direct-expansion", "notes")


def source_symbols() -> set[tuple[str, str]]:
    symbols = set()
    source = ROOT / "conformance" / "sbcl" / "symbols"
    for filename in ("common-lisp.tsv", "contrib-asdf.tsv", "contrib-uiop.tsv"):
        with (source / filename).open(newline="") as stream:
            symbols.update((row["package"], row["symbol"]) for row in csv.DictReader(stream, delimiter="\t"))
    return symbols


def ownership_rows() -> tuple[list[tuple[str, str]], list[str]]:
    keys: list[tuple[str, str]] = []
    errors: list[str] = []
    for path in sorted((ROOT / "packages").glob("**/ownership.tsv")):
        with path.open(newline="") as stream:
            reader = csv.DictReader(stream, delimiter="\t")
            if tuple(reader.fieldnames or ()) != HEADER:
                errors.append(f"{path}: invalid header")
            for line, row in enumerate(reader, 2):
                package, symbol, kind = row["package"], row["symbol"], row["kind"]
                keys.append((package, symbol))
                if any(token not in KINDS for token in kind.split("+")):
                    errors.append(f"{path}:{line}: invalid kind {kind}")
                if row["phase"] not in {"1", "2", "3"}:
                    errors.append(f"{path}:{line}: invalid phase {row['phase']}")
                if row["direct-expansion"] not in {"yes", "no"}:
                    errors.append(f"{path}:{line}: invalid direct-expansion")
                if row["crate"] in {"ncl-contrib-asdf", "ncl-contrib-uiop"}:
                    errors.append(f"{path}:{line}: obsolete contrib crate")
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
    if missing := expected - set(actual):
        errors.append(f"missing {len(missing)} COMMON-LISP symbols: {sorted(missing)[:5]}")
    if extra := set(actual) - expected:
        errors.append(f"extra {len(extra)} symbols: {sorted(extra)[:5]}")
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"OK: {len(actual)} ownership rows cover {len(expected)} retained symbols")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
