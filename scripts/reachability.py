#!/usr/bin/env python3
"""Reachability scanner for Rust crates using mod / #[path] / include!.

Starting from each crate's src/lib.rs (or src/main.rs), follows `mod`,
`#[path = "..."] mod`, and `include!("...")` declarations to find every
.rs file that is actually compiled. Reports any .rs file under a crate's
src/ that is never reached (orphaned by a #[path] shadow, or simply
never declared).

Exit status is non-zero when any crate has an orphan, so this doubles as
a CI gate (FR-016).
"""
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

CRATES = [
    REPO_ROOT / "packages/core/syntax",
    REPO_ROOT / "packages/core/compiler",
    REPO_ROOT / "packages/core/runtime",
    REPO_ROOT,  # root package "ncl"
]

MOD_RE = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;', re.M)
PATH_ATTR_RE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]\s*\n?\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;', re.M)
INCLUDE_RE = re.compile(r'include!\s*\(\s*"([^"]+)"\s*\)\s*;', re.M)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def strip_line_comments(src: str) -> str:
    # Cheap and good enough: drop `//` line comments. Doesn't handle
    # `//` inside string literals, but none of our mod/path/include
    # lines contain string literals with `//`.
    out = []
    for line in src.splitlines(keepends=True):
        idx = line.find("//")
        out.append(line if idx == -1 else line[:idx] + "\n")
    return "".join(out)


def scan_crate(src_root: Path):
    """Returns (reached: set[Path], all_rs: set[Path])."""
    # Cargo's default target discovery treats src/lib.rs AND src/main.rs
    # as separate compiled entry points when both exist and neither is
    # overridden by an explicit [lib]/[[bin]] path in Cargo.toml.
    entries = [p for p in (src_root / "lib.rs", src_root / "main.rs") if p.exists()]
    if not entries:
        return set(), set()

    all_rs = {p for p in src_root.rglob("*.rs")}

    reached = set()
    # (file_to_parse, dir_for_relative_resolution, reached_via_include)
    stack = [(entry, entry.parent, False) for entry in entries]
    # Track path-attr targets per (declaring_file) so plain `mod NAME;`
    # in the same file doesn't double-resolve a name already handled
    # by a `#[path]` attribute on that same declaration.
    while stack:
        current, resolve_dir, via_include = stack.pop()
        if current in reached:
            continue
        if not current.exists():
            print(f"WARN: {current} referenced but missing", file=sys.stderr)
            continue
        reached.add(current)
        src = strip_line_comments(read(current))

        path_attr_mods = set()
        for m in PATH_ATTR_RE.finditer(src):
            rel, _name = m.group(1), m.group(2)
            target = (resolve_dir / rel).resolve()
            path_attr_mods.add(m.end())
            stack.append((target, target.parent, False))

        # For plain `mod NAME;` not preceded by a #[path] attribute,
        # resolve via standard Rust module resolution: NAME.rs or
        # NAME/mod.rs under the *module directory* for this file.
        # The module directory for lib.rs/main.rs/mod.rs is its own
        # parent; for any other file `x.rs` it's `x/`. But a `mod`
        # line reached only because it was textually spliced in by
        # `include!` does NOT get its own nesting level -- rustc
        # resolves it relative to the includer's directory, verified
        # empirically against a scratch crate (see commit message).
        if via_include:
            mod_dir = resolve_dir
        elif current.name in ("lib.rs", "main.rs", "mod.rs"):
            mod_dir = current.parent
        else:
            mod_dir = current.parent / current.stem

        # Determine which `mod NAME;` occurrences are immediately
        # preceded by a #[path] attribute (already handled above) by
        # re-scanning with combined regex and checking overlap.
        path_attr_spans = [(m.start(), m.end()) for m in PATH_ATTR_RE.finditer(src)]

        def covered(pos):
            return any(s <= pos <= e for s, e in path_attr_spans)

        for m in MOD_RE.finditer(src):
            if covered(m.start()):
                continue
            name = m.group(1)
            # standard resolution: <mod_dir>/<name>.rs or <mod_dir>/<name>/mod.rs
            flat = mod_dir / f"{name}.rs"
            nested = mod_dir / name / "mod.rs"
            if flat.exists():
                stack.append((flat, flat.parent, False))
            elif nested.exists():
                stack.append((nested, nested.parent, False))
            else:
                print(f"WARN: cannot resolve `mod {name};` declared in {current}", file=sys.stderr)

        for m in INCLUDE_RE.finditer(src):
            rel = m.group(1)
            target = (resolve_dir / rel).resolve()
            # include! splices textually: treat the included file's own
            # mod/path/include declarations as resolved relative to the
            # INCLUDING file's directory, per rustc's include! semantics
            # (verified empirically -- see scan_crate's via_include note).
            stack.append((target, resolve_dir, True))

    return reached, all_rs


def main():
    any_orphan = False
    for crate_root in CRATES:
        src_root = crate_root / "src"
        if not src_root.exists():
            continue
        reached, all_rs = scan_crate(src_root)
        orphans = sorted(all_rs - reached)
        rel_crate = crate_root.relative_to(REPO_ROOT)
        print(f"== {rel_crate} == reached {len(reached)} / total {len(all_rs)}")
        for o in orphans:
            any_orphan = True
            lines = len(read(o).splitlines())
            print(f"  ORPHAN: {o.relative_to(REPO_ROOT)} ({lines} lines)")
    sys.exit(1 if any_orphan else 0)


if __name__ == "__main__":
    main()
