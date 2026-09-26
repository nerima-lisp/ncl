#!/usr/bin/env python3
"""Check standards-sensitive content on newly added Rust lines."""
from __future__ import annotations
import argparse, re, subprocess, sys
from pathlib import Path
from typing import Optional

ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = re.compile(
    r"\.(?:unwrap|expect|unwrap_or_else)\s*\(|"
    r"\b(?:panic|assert|assert_eq|assert_ne|unreachable)!\s*\("
)
UNSAFE = re.compile(r"\bunsafe\b")
TODO = re.compile(r"\b(?:todo|unimplemented)!\s*\(")
ALLOW = re.compile(r"check-added-lines:\s*allow\(([^)]+)\)(.*)$")

def diff_text(base: Optional[str]) -> str:
    command = ["git", "diff", "--no-ext-diff", "--unified=0"]
    if not base:
        base = subprocess.run(
            ["git", "merge-base", "FETCH_HEAD", "HEAD"],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout.strip()
    command.append(f"{base}...HEAD")
    command.extend(["--", "*.rs"])
    return subprocess.run(command, cwd=ROOT, check=True, capture_output=True, text=True).stdout

def added_lines(diff: str) -> list[tuple[Path, int, str]]:
    result, path, number, remaining = [], None, 0, 0
    for raw in diff.splitlines():
        if raw.startswith("+++ b/"):
            path = Path(raw[6:]); continue
        if raw.startswith("+++ "):
            path = None; continue
        if raw.startswith("@@"):
            match = re.search(r" \+(\d+)(?:,(\d+))? ", raw)
            if match:
                number, remaining = int(match.group(1)), int(match.group(2) or "1")
            else:
                path, remaining = None, 0
            continue
        if path is None or remaining == 0:
            continue
        if raw.startswith("+"):
            result.append((path, number, raw[1:])); number += 1; remaining -= 1
        elif raw.startswith("-"):
            continue
        else:
            number += 1; remaining -= 1
    return result

def added_rust_lines(diff: str) -> list[tuple[Path, int, str]]:
    return [line for line in added_lines(diff) if line[0].suffix == ".rs"]

def added(base: Optional[str] = None) -> list[tuple[Path, int, str]]:
    return added_rust_lines(diff_text(base))

def code_only(text: str) -> str:
    text = re.sub(r'"(?:\\.|[^"\\])*"', '""', text)
    return re.sub(r"//.*", "", text)

def test_ranges(source: str) -> list[tuple[int, int]]:
    ranges = []
    masked = re.sub(r'"(?:\\.|[^"\\])*"', lambda match: " " * len(match.group()), source)
    masked = re.sub(r"//[^\n]*", lambda match: " " * len(match.group()), masked)
    for match in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", masked):
        opening = masked.find("{", match.end())
        if opening < 0:
            continue
        depth = 0
        for index in range(opening, len(masked)):
            if masked[index] == "{": depth += 1
            elif masked[index] == "}":
                depth -= 1
                if depth == 0:
                    ranges.append((match.start(), index + 1)); break
    return ranges

def cfg_test_lines(lines: list[tuple[Path, int, str]]) -> set[tuple[Path, int]]:
    result = set()
    by_path = {}
    for path, number, _ in lines:
        by_path.setdefault(path, set()).add(number)
    for path, numbers in by_path.items():
        source_path = ROOT / path
        if source_path.is_file():
            source = source_path.read_text(encoding="utf-8")
            for start, end in test_ranges(source):
                first = source[:start].count("\n") + 1
                last = source[:end].count("\n") + 1
                result.update((path, number) for number in numbers if first <= number <= last)
            continue
        active = False
        depth = 0
        opened = False
        for current_path, number, text in lines:
            if current_path != path:
                continue
            code = code_only(text)
            if not active and re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", code):
                active = True
            if active:
                result.add((path, number))
                depth += code.count("{") - code.count("}")
                opened = opened or "{" in code
                if opened and depth <= 0:
                    active, depth, opened = False, 0, False
    return result

def violations(lines: list[tuple[Path, int, str]]) -> list[str]:
    failures, excluded = [], cfg_test_lines(lines)
    for index, (path, number, text) in enumerate(lines):
        if path.suffix != ".rs" or "tests" in path.parts or path.name.endswith("_test.rs") or (path, number) in excluded:
            continue
        location, code = f"{path}:{number}", code_only(text)
        allowed = set()
        candidates = [text]
        if index and lines[index - 1][0] == path and lines[index - 1][1] == number - 1:
            candidates.append(lines[index - 1][2])
        for candidate in candidates:
            directive = ALLOW.search(candidate)
            if directive and directive.group(2).strip():
                allowed.update(x.strip() for x in directive.group(1).split(","))
        if "index" not in allowed and re.search(r"(?<![#!])(?:\b[A-Za-z_][A-Za-z0-9_]*|\)|\])\s*\[[^]]+\]", code):
            failures.append(f"{location}: [index] unchecked indexing")
        if "as-cast" not in allowed and re.search(
            r"\bas\s+(?:u(?:8|16|32|64|128|size)|i(?:8|16|32|64|128|size)|f(?:32|64)|\*\s*(?:const|mut))\b",
            code,
        ) and not re.match(r"\s*use\b", code):
            failures.append(f"{location}: [as-cast] unchecked as-cast")
        match = FORBIDDEN.search(code)
        if match and "panic" not in allowed:
            if not (match.group().startswith(".unwrap_or_else") and
                    "PoisonError::into_inner" in code):
                failures.append(f"{location}: [panic] forbidden {match.group()}")
        if TODO.search(code) and "todo" not in allowed:
            failures.append(f"{location}: [todo] placeholder macro")
        if "wildcard" not in allowed and re.search(r"\b_\s*=>", code):
            failures.append(f"{location}: [wildcard] wildcard match arm")
        # UNBOUND is reported as a warning by main, because it is sometimes
        # intentional in registration metadata and is not a hard gate.
        if "sys" not in path.parts and UNSAFE.search(code):
            failures.append(f"{location}: [unsafe] unsafe outside packages/sys")
    return failures

def check_added_lines(diff: str) -> list[str]:
    failures = violations(added_lines(diff))
    for path, number, text in added_lines(diff):
        location = f"{path}:{number}"
        if "\u2014" in text:
            failures.append(f"{location}: [em-dash] em dash")
        if path.name == "Cargo.toml" and "packages" in path.parts:
            line = text.split("#", 1)[0].strip()
            if "=" in line:
                name, value = (part.strip() for part in line.split("=", 1))
                if re.fullmatch(r"[A-Za-z0-9_-]+", name) and "workspace = true" not in value and "path" not in value:
                    failures.append(f"{location}: [dependency] external dependency {name}")
    return failures

def unbound_warnings(lines: list[tuple[Path, int, str]]) -> list[str]:
    return [
        f"{path}:{number}: [unbound] Word::UNBOUND sentinel"
        for path, number, text in lines
        if path.suffix == ".rs" and re.search(r"\b[A-Za-z_][A-Za-z0-9_]*::UNBOUND\b", code_only(text))
    ]

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", nargs="?"); parser.add_argument("--base", dest="base_option")
    args = parser.parse_args()
    if args.base and args.base_option: parser.error("base revision was supplied twice")
    try:
        entries = added(args.base_option or args.base)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"error: unable to read diff: {getattr(error, 'stderr', '') or error}", file=sys.stderr); return 2
    lines = []
    lines.extend(entries)
    failures = violations(lines)
    warnings = unbound_warnings(lines)
    print(f"Rust added lines checked: {len(lines)}")
    if failures:
        print("violations:\n" + "\n".join(f"- {failure}" for failure in failures)); return 1
    if warnings:
        print("warnings:\n" + "\n".join(f"- {warning}" for warning in warnings))
    print("violations: none"); return 0

if __name__ == "__main__":
    sys.exit(main())
