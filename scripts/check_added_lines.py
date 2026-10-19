#!/usr/bin/env python3
"""Check standards-sensitive content on newly added Rust lines."""
from __future__ import annotations
import argparse, re, subprocess, sys
import ast
from functools import lru_cache
from pathlib import Path
from typing import Optional

ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = re.compile(
    r"\.(?:unwrap|expect|unwrap_or_else)\s*\(|"
    r"\b(?:panic|assert|assert_eq|assert_ne|unreachable)!\s*\("
)
UNSAFE = re.compile(r"\bunsafe\b")
TODO = re.compile(r"\b(?:todo|unimplemented)!\s*\(")
UNBOUND = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*::UNBOUND\b")
UNSUPPORTED = re.compile(r"\bUnsupported\b")
WORD_TABLE = re.compile(r"\b(?:HashMap|BTreeMap|HashSet)\s*<[^\n>]*\bWord\b")
ALLOW = re.compile(r"check-added-lines:\s*allow\(([^)]+)\)(.*)$")
TEST_MODULE_NAME = re.compile(r"(?:^tests\.rs$|^tests_|_test\.rs$|_tests\.rs$)")
MODULE_TOKEN = re.compile(r"\bmod\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*(?P<delimiter>[;{])")
INCLUDE_CALL = re.compile(
    r"\binclude!\s*\((?P<arguments>(?:[^()]|\([^()]*\))*)\)", re.S
)
STRING_LITERAL = r'"(?:\\.|[^"\\])*"'
CONCAT_ARGUMENTS = re.compile(
    rf"\s*{STRING_LITERAL}(?:\s*,\s*{STRING_LITERAL})*\s*,?\s*", re.S
)
MACRO_DEFINITION = re.compile(r"\bmacro_rules!\s*[A-Za-z_][A-Za-z0-9_]*\s*(?P<open>[{[(])")
MACRO_INVOCATION = re.compile(
    r"\b(?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*!\s*(?P<open>[{[(])"
)
INDEX = re.compile(
    r"(?<![#!'])"
    r"(?:\b(?!let\b|mut\b|ref\b)[A-Za-z_][A-Za-z0-9_]*|\)|\])\s*\[[^]]+\]"
)

def diff_text(base: Optional[str]) -> str:
    if (ROOT / ".jj").exists():
        revision = base or "main@origin"
        if revision == "origin/main":
            revision = "main@origin"
        return subprocess.run(
            [
                "jj", "--no-pager", "--color=never", "diff", "--git", "--context", "0",
                "--from", revision, "--to", "@", "--", 'root-glob:"**/*.rs"',
            ],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout

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
    masked = _mask_rust_comments_and_strings(source)
    for match in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", masked):
        opening = masked.find("{", match.end())
        semicolon = masked.find(";", match.end())
        if opening < 0 or (semicolon >= 0 and semicolon < opening):
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

def _mask_rust_comments_and_strings(source: str) -> str:
    masked = list(source)

    def blank(start: int, end: int) -> None:
        for position in range(start, end):
            if source[position] != "\n":
                masked[position] = " "

    index = 0
    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end < 0 else end
            blank(index, end)
            index = end
        elif source.startswith("/*", index):
            start, depth = index, 1
            index += 2
            while index < len(source) and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            blank(start, index)
        else:
            raw = re.match(r"(?:br|r)(?P<hashes>#{0,})\"", source[index:])
            if raw and (index == 0 or not (source[index - 1].isalnum() or source[index - 1] == "_")):
                delimiter = '"' + raw.group("hashes")
                end = source.find(delimiter, index + raw.end())
                end = len(source) if end < 0 else end + len(delimiter)
                blank(index, end)
                index = end
            elif source[index] == '"':
                start = index
                index += 1
                while index < len(source):
                    if source[index] == "\\":
                        index += 2
                    elif source[index] == '"':
                        index += 1
                        break
                    else:
                        index += 1
                blank(start, min(index, len(source)))
            elif source[index] == "'":
                character = re.match(r"'(?:\\.|[^'\\])'", source[index:])
                if character:
                    blank(index, index + character.end())
                    index += character.end()
                else:
                    index += 1
            else:
                index += 1
    return "".join(masked)

def _mask_macro_syntax(source: str) -> str:
    pairs = {"{": "}", "[": "]", "(": ")"}
    closing = {character: opening for opening, character in pairs.items()}
    stack, group_ends = [], {}
    for position, character in enumerate(source):
        if character in pairs:
            stack.append((character, position))
        elif character in closing and stack and stack[-1][0] == closing[character]:
            _, opening = stack.pop()
            group_ends[opening] = position + 1

    intervals = []
    for pattern in (MACRO_DEFINITION, MACRO_INVOCATION):
        for match in pattern.finditer(source):
            if pattern is MACRO_INVOCATION and match.group(0).lstrip().startswith("include!"):
                continue
            end = group_ends.get(match.end("open") - 1)
            if end is not None:
                intervals.append((match.start(), end))

    masked = list(source)
    end_of_interval = -1
    for start, end in sorted(intervals):
        if start >= end_of_interval:
            end_of_interval = end
            for position in range(start, end):
                if source[position] != "\n":
                    masked[position] = " "
        elif end > end_of_interval:
            for position in range(end_of_interval, end):
                if source[position] != "\n":
                    masked[position] = " "
            end_of_interval = end
    return "".join(masked)

def _module_attributes(source: str, masked: str, start: int) -> str:
    cursor = start

    def skip_whitespace() -> None:
        nonlocal cursor
        while cursor and masked[cursor - 1].isspace():
            cursor -= 1

    skip_whitespace()
    for modifier in ("unsafe",):
        if masked[:cursor].rstrip().endswith(modifier):
            cursor = len(masked[:cursor].rstrip()) - len(modifier)
            skip_whitespace()
    if cursor and masked[cursor - 1] == ")":
        depth, opening = 1, cursor - 2
        while opening >= 0 and depth:
            if masked[opening] == ")": depth += 1
            elif masked[opening] == "(": depth -= 1
            opening -= 1
        prefix = masked[:opening + 1].rstrip()
        if depth == 0 and re.search(r"\bpub$", prefix):
            cursor = len(prefix) - 3
    elif re.search(r"\bpub$", masked[:cursor].rstrip()):
        cursor = len(masked[:cursor].rstrip()) - 3

    attributes = []
    while True:
        skip_whitespace()
        if cursor == 0 or masked[cursor - 1] != "]":
            break
        depth, opening = 1, cursor - 2
        while opening >= 0 and depth:
            if masked[opening] == "]": depth += 1
            elif masked[opening] == "[": depth -= 1
            opening -= 1
        opening += 1
        if depth or opening == 0 or masked[opening - 1] != "#":
            break
        attribute_start = opening - 1
        attributes.append(source[attribute_start:cursor])
        cursor = attribute_start
    return "".join(reversed(attributes))

def _package_root(source_path: Path) -> Optional[Path]:
    for directory in (source_path.parent, *source_path.parent.parents):
        if (directory / "Cargo.toml").is_file():
            return directory
    return None

def _cargo_integration_test(path: Path) -> bool:
    source_path = path if path.is_absolute() else ROOT / path
    package_tests = next((
        directory for directory in source_path.parents
        if directory.name == "tests" and (directory.parent / "Cargo.toml").is_file()
    ), None)
    if package_tests is None:
        return False

    # Cargo's package-root tests/ convention is test-only unless workspace
    # production source explicitly pulls one of those files into its module graph.
    package_root = package_tests.parent.resolve()
    parents = _workspace_module_parent_graph(_workspace_root(package_root))
    incoming = parents.get(source_path.resolve(), [])
    return not any(not test_only for _, test_only in incoming)

def _source_module_dir(source_path: Path) -> Path:
    if source_path.name in {"lib.rs", "main.rs", "mod.rs"}:
        return source_path.parent
    return source_path.parent / source_path.stem

def _source_declarations(
    source_path: Path, module_dir: Path, source: str
) -> tuple[list[tuple[tuple[Path, ...], bool, Path]], list[tuple[Path, bool]]]:
    masked = _mask_macro_syntax(_mask_rust_comments_and_strings(source))
    brace_stack, brace_pairs = [], {}
    for position, character in enumerate(masked):
        if character == "{":
            brace_stack.append(position)
        elif character == "}" and brace_stack:
            brace_pairs[brace_stack.pop()] = position

    items = list(MODULE_TOKEN.finditer(masked))
    inline_modules = []
    for match in items:
        if match.group("delimiter") != "{":
            continue
        opening = match.end() - 1
        closing = brace_pairs.get(opening)
        if closing is None:
            continue
        attributes = _module_attributes(source, masked, match.start())
        test_only = bool(re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", attributes))
        inline_modules.append((opening, closing, match.group("name"), test_only))

    modules, includes = [], []
    for match in items:
        if match.group("delimiter") != ";":
            continue
        attributes = _module_attributes(source, masked, match.start())
        test_only = bool(re.search(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", attributes))
        path = re.search(r"#\s*\[\s*path\s*=\s*\"([^\"]+)\"\s*\]", attributes)
        child_base = module_dir
        path_dir = source_path.parent
        for opening, closing, name, inline_test_only in sorted(inline_modules):
            if opening < match.start() < closing:
                child_base /= name
                path_dir = child_base
                test_only = test_only or inline_test_only
        name = match.group("name")
        if path:
            targets = ((path_dir / path.group(1)).resolve(),)
        else:
            targets = (
                (child_base / f"{name}.rs").resolve(),
                (child_base / name / "mod.rs").resolve(),
            )
        modules.append((targets, test_only, child_base / name))

    for match in INCLUDE_CALL.finditer(masked):
        arguments = source[match.start("arguments"):match.end("arguments")]
        path = _include_path_argument(arguments)
        if path is None:
            continue
        inline_test_only = any(
            opening < match.start() < closing and test_only
            for opening, closing, _, test_only in inline_modules
        )
        includes.append((
            (source_path.parent / path).resolve(),
            inline_test_only,
        ))
    return modules, includes

@lru_cache(maxsize=32)
def _module_parent_graph(package_root: Path) -> dict[Path, list[tuple[Path, bool]]]:
    parents: dict[Path, list[tuple[Path, bool]]] = {}

    def visit(parent: Path, module_dir: Path, test_context: bool, visited: set) -> None:
        parent = parent.resolve()
        key = (parent, module_dir.resolve(), test_context)
        if key in visited:
            return
        visited.add(key)
        try:
            source = parent.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            return
        modules, includes = _source_declarations(parent, module_dir, source)
        for targets, declared_test_only, child_module_dir in modules:
            test_only = test_context or declared_test_only
            for target in targets:
                parents.setdefault(target, []).append((parent.resolve(), test_only))
                if target.is_file():
                    visit(target, child_module_dir, test_only, visited)
        for target, inline_test_only in includes:
            test_only = test_context or inline_test_only
            parents.setdefault(target, []).append((parent.resolve(), test_only))
            if target.is_file():
                visit(target, module_dir, test_only, visited)

    roots = [
        path for path in (
            package_root / "src/lib.rs",
            package_root / "src/main.rs",
            package_root / "build.rs",
        ) if path.is_file()
    ]
    roots.extend(
        path for path in (package_root / "src/bin").glob("*.rs") if path.is_file()
    )
    roots.extend(
        path for path in (package_root / "src/bin").glob("*/main.rs") if path.is_file()
    )
    roots.extend(
        path for directory in ("examples", "benches")
        for path in (package_root / directory).glob("*.rs") if path.is_file()
    )
    visited = set()
    for root in roots:
        visit(root, root.parent, False, visited)
    return parents

def _include_path_argument(arguments: str) -> Optional[str]:
    literal = re.fullmatch(rf"\s*({STRING_LITERAL})\s*", arguments, re.S)
    if literal is not None:
        try:
            value = ast.literal_eval(literal.group(1))
        except (SyntaxError, ValueError):
            return None
        return value if isinstance(value, str) else None

    concat = re.fullmatch(r"\s*concat!\s*\((?P<parts>.*)\)\s*", arguments, re.S)
    if concat is None or not CONCAT_ARGUMENTS.fullmatch(concat.group("parts")):
        return None
    try:
        pieces = re.findall(STRING_LITERAL, concat.group("parts"))
        return "".join(ast.literal_eval(piece) for piece in pieces)
    except (SyntaxError, ValueError):
        return None

def _workspace_root(package_root: Path) -> Path:
    for directory in (package_root, *package_root.parents):
        manifest = directory / "Cargo.toml"
        if not manifest.is_file():
            continue
        source = manifest.read_text(encoding="utf-8")
        if re.search(r"(?m)^\[workspace\]\s*$", source):
            return directory.resolve()
    return package_root.resolve()

@lru_cache(maxsize=16)
def _workspace_package_roots(workspace_root: Path) -> tuple[Path, ...]:
    manifest = workspace_root / "Cargo.toml"
    if not manifest.is_file():
        return (workspace_root,)
    source = manifest.read_text(encoding="utf-8")
    workspace = re.search(r"(?ms)^\[workspace\]\s*(.*?)(?=^\[|\Z)", source)
    members = re.search(r"(?ms)^\s*members\s*=\s*\[(.*?)\]", workspace.group(1)) if workspace else None
    patterns = re.findall(r'"((?:\\.|[^"\\])*)"', members.group(1)) if members else ["."]
    candidates = {workspace_root.resolve()}
    for pattern in patterns:
        if pattern == ".":
            continue
        candidates.update(
            candidate.resolve()
            for candidate in workspace_root.glob(pattern)
            if candidate.is_dir()
        )
    return tuple(sorted(
        package_root for package_root in candidates
        if (package_root / "Cargo.toml").is_file()
        and re.search(r"(?m)^\[package\]\s*$", (package_root / "Cargo.toml").read_text(encoding="utf-8"))
    ))

@lru_cache(maxsize=16)
def _workspace_module_parent_graph(workspace_root: Path) -> dict[Path, list[tuple[Path, bool]]]:
    parents: dict[Path, list[tuple[Path, bool]]] = {}
    for package_root in _workspace_package_roots(workspace_root):
        for target, incoming in _module_parent_graph(package_root).items():
            parents.setdefault(target, []).extend(incoming)
    return parents

def _is_cfg_test_module(path: Path) -> bool:
    source_path = path if path.is_absolute() else ROOT / path
    package_root = _package_root(source_path)
    if package_root is None:
        return False
    parents = _workspace_module_parent_graph(
        _workspace_root(package_root.resolve())
    )

    memo: dict[Path, bool] = {}

    def test_context(module: Path, visiting: set[Path]) -> bool:
        if module in memo:
            return memo[module]
        if module in visiting:
            return False
        declarations = parents.get(module, [])
        if not declarations:
            memo[module] = False
            return False
        visiting.add(module)
        result = all(test_only or test_context(parent, visiting) for parent, test_only in declarations)
        visiting.remove(module)
        memo[module] = result
        return result

    return test_context(source_path.resolve(), set())

def test_only_path(path: Path) -> bool:
    if _cargo_integration_test(path):
        return True
    has_test_convention = bool(TEST_MODULE_NAME.search(path.name) or "tests" in path.parts)
    return has_test_convention and _is_cfg_test_module(path)

def violations(lines: list[tuple[Path, int, str]]) -> list[str]:
    failures, excluded = [], cfg_test_lines(lines)
    test_only_paths = {}
    for index, (path, number, text) in enumerate(lines):
        if path not in test_only_paths:
            test_only_paths[path] = test_only_path(path)
        test_only = test_only_paths[path]
        if path.suffix != ".rs" or test_only or (path, number) in excluded:
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
        if "index" not in allowed and INDEX.search(code):
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
        if "unbound" not in allowed and UNBOUND.search(code):
            failures.append(f"{location}: [unbound] unbound sentinel")
        if "unsupported" not in allowed and UNSUPPORTED.search(code):
            failures.append(f"{location}: [unsupported] unsupported placeholder")
        if "word-table" not in allowed and WORD_TABLE.search(code):
            failures.append(f"{location}: [word-table] unrooted Word table")
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
    print(f"Rust added lines checked: {len(lines)}")
    if failures:
        print("violations:\n" + "\n".join(f"- {failure}" for failure in failures)); return 1
    print("violations: none"); return 0

if __name__ == "__main__":
    sys.exit(main())
