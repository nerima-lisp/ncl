#!/usr/bin/env python3
"""Run pinned ansi-test chapter entry points and report deftest-level state."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any


LANES = {
    "arrays": "numbers",
    "characters": "sequences-strings",
    "conditions": "conditions-types",
    "cons": "other",
    "data-and-control-flow": "other",
    "environment": "other",
    "eval-and-compile": "other",
    "files": "reader-pathnames-streams",
    "hash-tables": "other",
    "iteration": "loop-macros",
    "misc": "other",
    "numbers": "numbers",
    "objects": "clos",
    "packages": "other",
    "pathnames": "reader-pathnames-streams",
    "printer": "format-printer",
    "random": "numbers",
    "reader": "reader-pathnames-streams",
    "sequences": "sequences-strings",
    "streams": "reader-pathnames-streams",
    "strings": "sequences-strings",
    "structures": "other",
    "symbols": "other",
    "system-construction": "other",
    "types-and-classes": "conditions-types",
}


def cluster(output: str) -> str:
    for line in output.splitlines():
        line = line.strip()
        if line:
            line = re.sub(r"^ncl:\s*", "", line)
            return line[:240]
    return "no diagnostic output"


def deftest_count(chapter: Path) -> int:
    pattern = re.compile(r"\(deftest(?:\s|$)")
    return sum(
        len(pattern.findall(path.read_text(errors="replace")))
        for path in chapter.rglob("*.lsp")
    )


def run_chapter(ncl: str, chapter: Path, timeout: float) -> dict[str, Any]:
    try:
        result = subprocess.run(
            [ncl, "--load", "load.lsp"],
            cwd=chapter,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        output = "\n".join(
            value.decode(errors="replace") if isinstance(value, bytes) else value or ""
            for value in (error.stderr, error.stdout)
        )
        return {
            "chapter": chapter.name,
            "lane": LANES.get(chapter.name, "other"),
            "deftests": deftest_count(chapter),
            "status": "timeout",
            "diagnostic": cluster(output),
        }
    status = "passed" if result.returncode == 0 else "failed"
    if result.returncode < 0 or result.returncode >= 128:
        status = "crash"
    return {
        "chapter": chapter.name,
        "lane": LANES.get(chapter.name, "other"),
        "deftests": deftest_count(chapter),
        "status": status,
        "returncode": result.returncode,
        "diagnostic": cluster(result.stderr + "\n" + result.stdout),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ncl", required=True)
    parser.add_argument("--ansi-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=60.0)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    ncl = str(Path(args.ncl).resolve())

    chapters = sorted(
        path.parent for path in args.ansi_dir.glob("*/load.lsp")
        if path.parent.name not in {"auxiliary", "beyond-ansi"}
    )
    with ThreadPoolExecutor(max_workers=max(1, args.workers)) as executor:
        results = list(executor.map(
            lambda chapter: run_chapter(ncl, chapter, args.timeout), chapters,
        ))
    counts = Counter(result["status"] for result in results)
    diagnostics = Counter(
        (result["diagnostic"], result["lane"]) for result in results
    )
    unexecuted_reasons = Counter()
    for result in results:
        if result["status"] != "passed":
            unexecuted_reasons[(result["diagnostic"], result["lane"])] += result["deftests"]
    lane_counts: dict[str, dict[str, int]] = {}
    for result in results:
        values = lane_counts.setdefault(
            result["lane"], {"passed": 0, "failed": 0, "unexecuted": 0, "timeout": 0, "crash": 0},
        )
        if result["status"] == "passed":
            values["passed"] += result["deftests"]
        else:
            values["unexecuted"] += result["deftests"]
            values[result["status"]] += 1
    commit = subprocess.run(
        ["git", "-C", str(args.ansi_dir), "rev-parse", "HEAD"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    total_deftests = sum(result["deftests"] for result in results)
    chapter_passed = counts["passed"]
    print(json.dumps({
        "unit": "deftest",
        "commit": commit,
        "total": total_deftests,
        "passed": 0,
        "failed": 0,
        "unexecuted": total_deftests,
        "categories": lane_counts,
        "failure_clusters": [
            {"diagnostic": message, "count": count, "lane": lane}
            for (message, lane), count in diagnostics.most_common(20)
        ],
        "unexecuted_reasons": [
            {"reason": message, "count": count, "lane": lane}
            for (message, lane), count in unexecuted_reasons.most_common(20)
            if count > 0
        ],
        "execution": {
            "chapter_load_passed": chapter_passed,
            "chapter_load_failed": counts["failed"] + counts["crash"] + counts["timeout"],
            "deftests_executed": chapter_passed == len(results),
            "blocked_by": "rt.lsp LOOP for ... = ... TypeError" if chapter_passed != len(results) else None,
        },
        "results": results,
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
