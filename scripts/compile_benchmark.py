#!/usr/bin/env python3
"""Measure compile-file performance over the pinned Common Lisp corpus."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import sys
import tempfile
from typing import Sequence

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from bench.corpus.compile import BenchmarkError, run_benchmark, _safe_diagnostic
from bench.corpus.fetch import CorpusError


def _write_report(path: Path, report: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", newline="\n", dir=path.parent,
            prefix=f".{path.name}.", suffix=".tmp", delete=False,
        ) as stream:
            temporary = Path(stream.name)
            json.dump(report, stream, indent=2, sort_keys=True, allow_nan=False)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ncl", required=True, help="NCL executable")
    parser.add_argument("--ncl-profiler", help="optional profiler-enabled NCL executable")
    parser.add_argument("--sbcl", default="sbcl", help="SBCL 2.6.10 executable")
    parser.add_argument("--mode", choices=("light", "full"), required=True)
    parser.add_argument("--output", type=Path, help="write the JSON report to this path")
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=REPO_ROOT / "target/bench-corpus-cache",
        help="cache verified upstream archives here",
    )
    parser.add_argument("--timeout", type=float, default=60.0, help="seconds allowed per Lisp process")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = _parser()
    args = parser.parse_args(argv)
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("--timeout must be a finite positive number")
    try:
        report = run_benchmark(
            args.ncl,
            args.sbcl,
            args.mode,
            ncl_profiler=args.ncl_profiler,
            cache_dir=args.cache_dir,
            timeout=args.timeout,
        )
    except (BenchmarkError, CorpusError, OSError, ValueError) as error:
        report = {"schema_version": 1, "status": "error", "error": _safe_diagnostic(str(error))}
        rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
        if args.output is None:
            sys.stdout.write(rendered)
        else:
            _write_report(args.output, report)
        print(f"compile benchmark: {report['error']}", file=sys.stderr)
        return 1
    rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    if args.output is None:
        sys.stdout.write(rendered)
    else:
        _write_report(args.output, report)
    return 0 if report["status"] in ("complete", "partial") else 1


if __name__ == "__main__":
    raise SystemExit(main())
