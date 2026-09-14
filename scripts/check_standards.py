#!/usr/bin/env python3
"""Check the mechanical parts of NCL's Rust coding standards."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = re.compile(r"\b(?:unwrap|expect)\s*\(|\bpanic!\s*\(")
UNSAFE = re.compile(r"\bunsafe\b")
TODO = re.compile(r"\btodo!\s*\(")


def dependency_violations(path: Path) -> list[str]:
    section = ""
    violations = []
    for number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.split("#", 1)[0].strip()
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
            continue
        if section not in ("dependencies", "dev-dependencies") or not line or "=" not in line:
            continue
        name, value = (part.strip() for part in line.split("=", 1))
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            continue
        local = "workspace = true" in value or "path" in value
        if not local:
            violations.append(f"{path.relative_to(ROOT)}:{number}: external dependency {name}")
    return violations


def matching_brace(source: str, opening: int) -> int:
    depth = 0
    in_string = False
    escaped = False
    for index in range(opening, len(source)):
        char = source[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
        elif char == '"':
            in_string = True
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index + 1
    return len(source)


def non_test_source(path: Path, source: str) -> str:
    if "tests" in path.parts:
        return ""
    result = list(source)
    marker = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
    for match in marker.finditer(source):
        opening = source.find("{", match.end())
        if opening < 0:
            continue
        end = matching_brace(source, opening)
        result[match.start():end] = " " * (end - match.start())
    return "".join(result)


def main() -> int:
    failures = []
    todo_count = 0
    manifests = sorted((ROOT / "packages").rglob("Cargo.toml"))
    for manifest in manifests:
        failures.extend(dependency_violations(manifest))

    rust_files = sorted((ROOT / "packages").rglob("*.rs"))
    for path in rust_files:
        relative = path.relative_to(ROOT)
        source = path.read_text(encoding="utf-8", errors="replace")
        if len(source.splitlines()) > 500:
            failures.append(f"{relative}: file exceeds 500 lines")
        if path.name == "mod.rs":
            failures.append(f"{relative}: mod.rs is forbidden")
        if "sys" not in path.parts and UNSAFE.search(source):
            failures.append(f"{relative}: unsafe is outside packages/sys")
        checked = non_test_source(path, source)
        failures.extend(f"{relative}:{match.start()}: forbidden {match.group()}" for match in FORBIDDEN.finditer(checked))
        todo_count += len(TODO.findall(checked))

    print(f"manifests checked: {len(manifests)}")
    print(f"Rust files checked: {len(rust_files)}")
    print(f"todo!() outside tests: {todo_count}")
    if failures:
        print("violations:")
        print("\n".join(f"- {failure}" for failure in failures))
        return 1
    print("violations: none")
    return 0


if __name__ == "__main__":
    sys.exit(main())
