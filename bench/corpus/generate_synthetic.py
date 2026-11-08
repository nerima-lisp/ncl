#!/usr/bin/env python3
"""Generate the deterministic compile-file stress source."""

from __future__ import annotations

import argparse
from pathlib import Path


def generate_source(defuns: int = 2_000) -> str:
    if defuns < 1:
        raise ValueError("defuns must be positive")
    return "".join(
        f"(defun compile-bench-{index:04d} (value) (+ value 1))\n"
        for index in range(defuns)
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--defuns", type=int, default=2_000)
    args = parser.parse_args()
    try:
        source = generate_source(args.defuns)
    except ValueError as error:
        parser.error(str(error))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(source, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
