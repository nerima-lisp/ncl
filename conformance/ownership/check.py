#!/usr/bin/env python3
"""Validate the complete SBCL symbol ownership surface."""
from __future__ import annotations
import csv
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "conformance" / "sbcl" / "symbols"
OWNERSHIP = ROOT / "conformance" / "ownership" / "symbols.tsv"
KINDS = {"function", "macro", "special-operator", "variable", "constant", "type", "class", "condition", "generic-function", "declaration", "other"}
BASE_CRATES = {"ncl-sys", "ncl-object", "ncl-ir", "ncl-types", "ncl-reader", "ncl-printer", "ncl-conditions", "ncl-clos", "ncl-compiler-front", "ncl-compiler-back", "ncl-threads", "ncl-ffi", "ncl-image", "ncl-runtime", "ncl-conformance", "ncl"}
LIB_CRATES = {"ncl-lib-numbers", "ncl-lib-sequences", "ncl-lib-strings", "ncl-lib-hash-arrays", "ncl-lib-streams", "ncl-lib-pathnames", "ncl-lib-format", "ncl-lib-loop", "ncl-lib-packages", "ncl-lib-macros"}

def source_keys():
    keys = set()
    for path in sorted(SOURCE.glob("*.tsv")):
        if path.name in {"contribs.tsv", "internal-referenced.tsv"}:
            continue
        with path.open(newline="") as stream:
            for row in csv.DictReader(stream, delimiter="\t"):
                keys.add((row["package"], row["symbol"]))
    with (SOURCE / "internal-referenced.tsv").open(newline="") as stream:
        for row in csv.DictReader(stream, delimiter="\t"):
            keys.add((row["package"], row["symbol"]))
    return keys

def main():
    errors = []
    expected = source_keys()
    actual = []
    with OWNERSHIP.open(newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        required = {"package", "symbol", "kind", "crate", "phase", "direct-expansion", "notes"}
        if set(reader.fieldnames or ()) != required:
            errors.append(f"header mismatch: {reader.fieldnames}")
        for line, row in enumerate(reader, 2):
            key = (row["package"], row["symbol"])
            actual.append(key)
            if row["kind"] and any(kind not in KINDS for kind in row["kind"].split("+")):
                errors.append(f"line {line}: invalid kind {row['kind']}")
            if row["phase"] not in {"1", "2", "3"}:
                errors.append(f"line {line}: invalid phase {row['phase']}")
            if row["direct-expansion"] not in {"yes", "no"}:
                errors.append(f"line {line}: invalid direct-expansion")
            crate_ok = row["crate"] in BASE_CRATES | LIB_CRATES or (row["phase"] == "3" and re.fullmatch(r"ncl-contrib-[a-z0-9-]+", row["crate"]))
            if not crate_ok:
                errors.append(f"line {line}: crate not in crates.md or Phase 3 reservation: {row['crate']}")
    if len(actual) != len(set(actual)):
        errors.append("ownership contains duplicate (package, symbol) rows")
    missing = expected - set(actual)
    extra = set(actual) - expected
    if missing:
        errors.append(f"missing {len(missing)} source symbols: {sorted(missing)[:5]}")
    if extra:
        errors.append(f"extra {len(extra)} symbols: {sorted(extra)[:5]}")
    if len(actual) == 0 or len(expected) == 0:
        errors.append("empty source or ownership surface")
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"OK: {len(actual)} ownership rows cover {len(expected)} source symbols")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
