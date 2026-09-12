#!/usr/bin/env python3
"""Keep symbol lookup paths free of the allocating canonicalizer."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
HOT_PATHS = (
    ROOT / "packages/core/runtime/src/evaluator/evaluator_special_forms/evaluator_function_calls.rs",
    ROOT / "packages/core/runtime/src/evaluator/variable_lookup.rs",
)


def main() -> int:
    violations = [
        str(path.relative_to(ROOT))
        for path in HOT_PATHS
        if "normalize_name" in path.read_text(encoding="utf-8")
    ]
    if violations:
        print("allocating normalize_name found in symbol lookup path:")
        print("\n".join(violations))
        return 1
    print("symbol lookup allocation audit: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
