# Wave plan

This document records the current parallel implementation lanes. It is
deliberately short: implementation details belong in the owning crate and
measured results belong in `conformance/`.

## Current lanes

| Lane | Scope |
| --- | --- |
| ansi-runner | ANSI conformance runner and M2 reporting |
| cl-bench | M3 benchmark execution and recorded baselines |
| loop-macros | LOOP macro implementation |
| clos-mop | CLOS and MOP behavior |
| opt-core | Pass manager, inlining, GVN, and SCCP |
| regalloc | SSA register allocation |
| parallel-gc | Concurrent and parallel collection |
| threads | Thread runtime and synchronization |
| os-ffi | OS and foreign-function interfaces |
| image-startup | Image loading and startup measurements |
| disasm-debug | Disassembly and debugging tools |
| slop-docs | Documentation current-state update and mechanical standards |

The next lanes cover the ANSI chapters for numbers, sequences and strings,
format and printer, reader, pathnames and streams, conditions, and types.
Later lanes cover escape analysis, LICM, e-graph optimization, phases 1c, 2,
5, and 6, ASDF/UIOP, profiling, coverage, and the 14-library load matrix.

## Current executable surface

The root binary supports:

* `--eval SOURCE`, including recursive source-level `fib(25)`, which
  prints `75025`.
* `--load FILE`, `--script FILE`, and `--compile-file FILE`.
* An interactive REPL when no option is supplied.

This status is verified by the CLI integration tests. The full ANSI surface,
benchmark targets, and library matrix are still work in progress.

## Acceptance gates

Every lane keeps its own tests and ownership data. The shared gates are:

* `nix develop --command cargo test --workspace`
* `nix develop --command cargo clippy --workspace --all-targets -- -D warnings`
* `nix develop --command cargo fmt --check`
* `nix develop --command python3 scripts/check_standards.py`
* `nix develop --command python3 scripts/check_added_lines.py --base main`
* `nix develop --command mkdocs build --strict --config-file docs/mkdocs.yml`
* regions coverage of at least 95 percent

Measured results must identify the command, platform, and baseline. SBCL
values in `conformance/baselines/` are comparison data, not an API
compatibility requirement.
