#!/usr/bin/env python3
"""Verify that coverage exclusions are explicit and match the CI gate."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / ".github/workflows/ci.yml"
EXPECTED = {
    "src/cli/repl/interactive.rs",
    "packages/core/runtime/src/builtins/registry/builtin_definitions.rs",
}


def main() -> int:
    ci = CI.read_text(encoding="utf-8")
    patterns = re.findall(r"--ignore-filename-regex '([^']+)'", ci)
    if not patterns:
        print("coverage exclusions: no ignore-filename-regex found", file=sys.stderr)
        return 1
    if len(patterns) != 2 or len(set(patterns)) != 1:
        print("coverage exclusions: expected two identical gate regexes in CI", file=sys.stderr)
        return 1

    regex = patterns[0]
    paths = {
        part.replace(r"\.", ".")
        for part in regex.split("|")
        if part
    }
    if paths != EXPECTED:
        print(f"coverage exclusions: expected {sorted(EXPECTED)}, found {sorted(paths)}", file=sys.stderr)
        return 1
    missing = [path for path in EXPECTED if not (ROOT / path).is_file()]
    if missing:
        print(f"coverage exclusions: missing files: {', '.join(sorted(missing))}", file=sys.stderr)
        return 1

    print(f"coverage exclusions: verified {len(EXPECTED)} explicit files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
