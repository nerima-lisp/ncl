# Wave plan

## Overview

This document records NCL's implementation plan and acceptance criteria. Each lane follows this plan and the frozen design contracts.

NCL targets compatibility with SBCL 2.6.0. The target includes ANSI Common Lisp, the public SBCL APIs and contribs, 58 internal symbols referenced by libraries, ASDF/UIOP, and 14 libraries. P0 establishes the SBCL surface tables and coverage check so later lanes can count the remaining symbols in each package. Work proceeds through P0, P0b, and P1 to P6, followed by release.

## Current executable surface

The root binary handles `--eval`, `--load`, `--script`, and `--compile-file`. Running it without an option starts the interactive REPL. The CLI integration tests verify the recursive `fib(25)` example, which prints `75025`. The remaining ANSI surface, performance targets, and 14-library load matrix remain in progress.

## Decisions

| # | Decision | Relation |
|---|---|---|
| D-1 | Keep the existing SBCL-independent core and update the SBCL surface and its ownership tables. | None |
| D-2 | Do not create SB-* packages; provide only NCL-specific APIs. | Withdrawn by D-9 |
| D-3 | Do not port contribs beyond the runtime and development tools. | Withdrawn by D-9 |
| D-4 | Implement ASDF/UIOP in NCL and use the ASDF/INTERFACE and UIOP/DRIVER packages. Preserve D-24-3 only for ASDF/UIOP. | D-24-3, limited to ASDF/UIOP |
| D-5 | Keep `#+ncl` backends for portability-layer libraries in the NCL repository. | Preserved by D-10 |
| D-6 | Run at most 16 lanes concurrently, including one integration lane. | None |
| D-7 | Target the architecture and operating system combinations in D-12. | Confirmed by D-12 |
| D-8 | Target SSA optimization and register allocation, parallel GC, tier-1 speculative optimization with deopt/OSR, SIMD, and e-graph rewriting. | Extends D-24-1 |
| D-9 | Compatibility covers the public APIs of SB-EXT, SB-THREAD, SB-ALIEN, SB-SYS, SB-MOP, SB-GRAY, SB-UNICODE, and SB-DEBUG; contribs; and 58 internal symbols referenced by libraries. | D-2, D-3, and FR-001 |
| D-10 | `*features*` includes `:ncl` and never includes `:sbcl`. Libraries use `#+ncl` backends. | Preserves D-5 |
| D-11 | NCL passes every ansi-test test passed by SBCL 2.6.0 on the same day and machine. For cl-bench, NCL's geometric mean is at most SBCL's, and each of the 65 benchmarks runs within 1.5 times SBCL's time. | Adds ANSI-test and cl-bench acceptance criteria |
| D-12 | Target x86-64 and AArch64 on macOS and Linux. | Confirms D-7 |
| D-13 | A contrib is in scope on exactly the platforms where SBCL 2.6.0 loads it. The extraction loaded 18 in-scope contribs on macOS arm64; `sb-simd` and `sb-perf` were out of scope there. All 20 contribs loaded on x86-64 Linux, including `sb-simd` (312 external symbols) and `sb-perf` (2). The extractor's 20-entry list has no additional contribs to account for the difference from D-13's stated macOS count. | [SBCL surface inventory](../../../conformance/sbcl/symbols/README.md) |

D-9 withdraws D-2's SB-* exclusion, D-3's contrib exclusion, and FR-001's SBCL-surface removal requirement.

The eight public SB package tables contain 698 symbols on macOS arm64 and 731 on x86-64 Linux. Linux adds 32 rows in SB-EXT and one in SB-DEBUG; the other six counts match. At revision `57cc06ab`, the package counts in this plan's linked [SBCL surface inventory](../../../conformance/sbcl/symbols/README.md) summed to 690, one below the corresponding TSV row count in each package. The current macOS TSVs and the TSVs under `conformance/sbcl/symbols/` at that revision have identical symbol-name sets; the repository contains no 690-symbol membership list. The COMMON-LISP table's 978 symbols and 58 library-referenced internal symbols are separate counts.

## Project acceptance criteria

| Criterion | Target | Verification |
|---|---|---|
| ansi-test | NCL passes every test passed by SBCL 2.6.0 on the same day and machine. | P0b records SBCL's passing test names and compares NCL under the same conditions. The baseline has 21,768 of 21,942 tests passing; the gate uses the test-name set. |
| cl-bench | Geometric mean at most SBCL's; each of the 65 benchmarks within 1.5 times SBCL's time. | Measure SBCL and NCL on the same day and machine. |
| Startup time | Median at most 17.331 ms. | Measure startup with empty input. |
| Executable size | At most 82,501,760 bytes. | Measure the release executable. |

The recorded SBCL baseline has positive timings for 64 of the 65 named benchmarks. SBCL disables `WALK-LIST/MESS`, so it has no baseline runtime. The 1.5x comparison is undefined for that benchmark; decide whether to exclude it or revise the 65-benchmark target before marking this gate complete. See the [recorded baseline](../../../conformance/baselines/cl-bench-sbcl-2.6.0.md).

## Functional requirements

### FR-001 SBCL surface removal

Withdrawn by D-9. SB-* surface is allowed and is checked against the SBCL surface tables.

### FR-002 ANSI Common Lisp

Register all 978 external symbols in the COMMON-LISP package in the ownership tables. The ansi-test acceptance criterion follows D-11.

### FR-003 Runtime and extension API

| Function | Owning crate |
|---|---|
| Threads, synchronization, timers, concurrent queues | ncl-threads |
| FFI | ncl-ffi |
| MOP | ncl-clos |
| Gray streams | ncl-lib-streams |
| Weak pointers, finalizers, GC control | ncl-object |
| Image saving and standalone executables | ncl-image |
| Package locks | ncl-lib-packages |
| Unicode | ncl-lib-strings |
| POSIX, processes, sockets | ncl-os |

NCL's public APIs use NCL-THREADS, NCL-FFI, NCL-MOP, NCL-GRAY, NCL-GC, NCL-IMAGE, NCL-UNICODE, NCL-OS, NCL-EXT, and NCL-SYS. When NCL and SBCL packages expose the same named API for the same behavior, import and export one shared symbol object instead of interning a duplicate. A name used only by SBCL is owned by the domain crate that implements its behavior. Do not create SB-* nickname or alias packages, or compatibility crates that duplicate behavior.

### FR-004 Development tools and contribs

| Function | Owning crate |
|---|---|
| Statistical profiler | ncl-profiler |
| Coverage | ncl-coverage |
| Debugger and introspection | ncl-debug |
| x86-64 and AArch64 disassembler | ncl-disasm |

Load libcapstone at runtime through ncl-ffi. Contrib ownership follows the table below; assign each unlisted contrib to the crate responsible for its primary functionality.

| Contrib | Owning crate |
|---|---|
| sb-posix, sb-bsd-sockets | ncl-os |
| sb-sprof | ncl-profiler |
| sb-cover | ncl-coverage |
| sb-introspect | ncl-debug |
| sb-concurrency, sb-queue | ncl-threads |
| sb-executable | ncl-image |
| sb-simple-streams | ncl-lib-streams |

D-13 states 20 contribs for macOS arm64. The extraction loaded 18 in-scope contribs there; `sb-simd` and `sb-perf` were out of scope. All 20 entries in the extractor's `*sbcl-contribs*` list loaded on x86-64 Linux, including `sb-simd` (312 external symbols) and `sb-perf` (2). ASDF and UIOP are recorded separately.

### FR-005 ASDF/UIOP

Implement ASDF/UIOP in ncl-uiop and ncl-asdf. The existing ownership tables contain 230 ASDF/INTERFACE symbols and 427 UIOP/DRIVER symbols. NCL's ASDF must load the pinned upstream `.asd` files.

### FR-006 Library loading

The 14 target libraries are cffi, bordeaux-threads, closer-mop, trivial-features, trivial-garbage, trivial-gray-streams, trivial-backtrace, alexandria, cl-ppcre, babel, usocket, flexi-streams, static-vectors, and cl-fad. Place `#+ncl` backends under `libraries/<name>/` and load them through NCL's ASDF. Determine which libraries need backends by inspecting their `#+sbcl` branches.

### FR-007 Performance

Use the cl-bench criteria in D-11. Startup time and executable size follow the project acceptance criteria table.

### FR-008 Compiler and GC phases

| Phase | Work |
|---|---|
| 1b | Linear-scan register allocation; GC scans the full frame chain. |
| 1c | Tail calls and dedicated `&rest`/`&key` prologues. |
| 2 | Unboxing, type inference, inline cache. |
| 3 | Inlining, escape analysis and stack allocation, GVN, SCCP, LICM, e-graph. |
| 3 | SSA-based register allocation. |
| 4 | Parallel GC. |
| 5 | Tier-1 speculative optimization with deopt/OSR. |
| 6 | SIMD. |

Run the IR verifier after each pass. Give each e-graph rewrite an IR round-trip test. E-graph introduction must not worsen the cl-bench geometric mean.

### FR-009 Stability

Run the IR verifier after each pass. Compare results for the same program at tier-0, with optimizations, and at tier-1. In GC stress mode, collect at every safepoint. Add a deopt test for each speculative optimization, watchdogs for concurrency tests, and maintain at least 95% regions coverage.

### FR-010 Documentation and implementation quality

Update the README and design documents to reflect current decisions. Do not add stubs that return Unsupported, unused code, comments that restate behavior, or tests without assertions.

### FR-011 Parallel execution

Use one worktree and one branch per lane. Split ownership tables by crate under `packages/<crate>/ownership.tsv`. Each lane freezes its crate's table and public API in its first commit. Only N01 (extension API and ownership) and N02 (IR contract v2) may change the root Cargo.toml, Cargo.lock, and frozen contract documents.

## Technical specification

### P0: surface tables and coverage check

Extract the SBCL 2.6.0 surface into tables under `conformance/sbcl/symbols/`. Use `conformance/sbcl/extract-surface.lisp` as the extraction script. The flat tables preserve the macOS arm64 baseline; Linux x86-64 differences and Linux-only tables go under `conformance/sbcl/symbols/linux-x86_64/`. The `contribs.tsv` matrix records each platform's status, external count, and reason. The ownership checker validates the resolved platform table and its count.

The SBCL surface tables record each symbol's presence, external status, kind,
and function or macro lambda list for comparison against SBCL 2.6.0. Kinds
include special operator, macro, function, variable, constant, class, and type.

`conformance/ownership/check.py` reads every crate's `packages/<crate>/ownership.tsv` and preserves the existing seven-column format. Fail when an SB-* row's package, symbol name, or kind does not match the surface table, and list each failure as `package::symbol (kind): reason`. For symbols in the surface table but absent from ownership tables, list names and counts by package and exit 0. Support filtering by package. Preserve the 978-symbol COMMON-LISP coverage requirement and reject duplicate ownership rows.

### Ownership and registration

- `ncl-ownership` checks the table text for each crate.
- `ncl-stdlib::register_all` is the registration entry point for each crate.
- A crate that has not landed registers no symbols; the conformance runner reports it as not landed.

### Planned crate dependency edges

These are target edges. N76 adds `ncl-ffi` to `ncl-disasm`'s current
`ncl-object` dependency for runtime Capstone access.

```text
ncl-os -> ncl-object, ncl-sys, ncl-conditions, ncl-lib-streams
ncl-uiop -> ncl-object, ncl-conditions, ncl-lib-streams, ncl-lib-pathnames, ncl-lib-strings, ncl-os
ncl-asdf -> ncl-object, ncl-conditions, ncl-clos, ncl-uiop
ncl-profiler -> ncl-object, ncl-sys, ncl-threads, ncl-conditions
ncl-coverage -> ncl-object, ncl-compiler-front
ncl-debug -> ncl-object, ncl-sys, ncl-conditions, ncl-codegen, ncl-lib-streams
ncl-disasm -> ncl-object, ncl-ffi
ncl-stdlib -> existing dependencies plus ncl-threads, ncl-ffi, ncl-image, ncl-compiler-front, and the seven extension crates
```

### IR contract v2

- Add function-entry constants, closure creation, and handler-region lowering for `catch`, `unwind-protect`, and `progv`.
- Closures and Phase 3 passes depend on stable IR.
- Land codegen templates for both architectures in the same lane.
- Freeze the register-class contract after N49 lands.

## Constraints

- Load libgmp, libmpfr, and libcapstone at runtime through ncl-ffi.
- Do not provide backward compatibility for the old NCL API or FASL v1.
- Run Rust validation in a Nix development shell.

## Test requirements

- Pass each crate's `registers_every_owned_symbol` test.
- Target at least 95% regions coverage per crate. CI currently enforces the
  threshold at workspace level; its coverage job does not check the per-crate
  target.
- For local lane work, run `cargo test -p <crate>` for each changed crate. If a
  test under `tests/` changes, run `cargo test -p ncl --test <name>`.
- CI runs formatting, checks, debug and release tests, and release builds on
  x86-64 Linux and AArch64 macOS. The coverage job runs on Ubuntu:

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --all-targets
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo build --locked --workspace --release
cargo test --locked --workspace --all-features --all-targets --release
cargo llvm-cov --locked --workspace --all-features --all-targets --summary-only --fail-under-regions 95.0
```

- Run `python3 scripts/check_standards.py`.
- Validate all target operating systems and architectures in D-12.

### Failure behavior

Reject duplicate ownership rows, SB-* rows absent from the SBCL surface, and package, name, or kind mismatches. Report surface symbols that have not been implemented as missing entries for coverage progress.

### Milestones

| ID | Observable state |
|---|---|
| M0 | SBCL surface tables are complete. The ownership check rejects mismatches, reports missing symbols by package, and preserves CL 978-symbol coverage and duplicate rejection. |
| M1 | `ncl --eval` defines fib, evaluates `(fib 25)`, and prints `75025` from native execution on D-12 target platforms. |
| M2 | P0b records SBCL's passing test-name set, and NCL meets D-11's ansi-test criterion. |
| M3 | All 65 cl-bench benchmarks finish and meet D-11's geometric-mean and per-benchmark limits. |
| M4 | The 14 library `#+ncl` backends load through NCL's ASDF. |
| M5 | Startup time and executable size meet the targets in the acceptance criteria table above. |

## Work breakdown

Lane IDs use the N prefix. A start condition refers to an upstream lane's frozen API.

### Gate 0

| N | Scope | Start condition | Acceptance |
|---|---|---|---|
| N01 | Extension API contract, `crates.md`, ownership-table split, ncl-ownership API and type unification, restored surface extraction, coverage check, skeletons for seven new crates, root Cargo.toml, and `register_all` order table. | None | M0; freeze the ownership format and API. |
| N02 | IR contract v2, compiler-pipeline, and codegen templates for both architectures. | None | New operations pass execution tests on both architectures. |
| N03 | README and documentation. | None | Record current decisions and pass the strict MkDocs build. |
| N04 | Integration, full test runs, and checks on all target operating systems and architectures. | None | Validate D-12 target combinations. |
| N05 | Recover coverage for sys/object/objfile/asm. | None | Each crate reaches at least 95% regions coverage. |
| N76 | ncl-disasm decoder. | After N01 establishes ncl-ffi. | Disassemble assembler output on target architectures; load libcapstone at runtime through ncl-ffi. |

### Wave A

Start in priority order after N01 and N02 freeze.

| Priority | N | Scope | Start condition |
|---|---|---|---|
| 1 | N60 | Runtime and binary: eval/compile/compile-file/load, MacroCaller, REPL, `--eval`/`--load`/`--script`; owns M1. | N01, N02 |
| 2 | N27 | lib-macros A: definitions, control, setf. | N01 |
| 3 | N20 | lib-numbers. | N01 |
| 4 | N13 | compiler-front surface migration and IR v2 lowering for closures, catch/unwind-protect/progv, and macrolet. | N01, N02 |
| 5 | N40 | Phase 1b. | N02 |
| 6-14 | N10 threads; N11 FFI; N12 conditions; N14 types/reader/printer/image/object; N21 sequences; N22 strings; N24 streams; N25 packages; N26 CLOS. | N01 |
| 15 | N43 | Pass manager and inlining. | N02 |

### Wave B

| N | Scope | Start condition |
|---|---|---|
| N23 | hash-arrays | N01 |
| N28 | LOOP | N27 |
| N29 | pathnames | N22 |
| N30 | format | N22, N24, printer |
| N44-N47 | Escape analysis, GVN/SCCP, LICM, e-graph. | N43 |
| N41 | Phase 1c. | N40 |
| N49 | SSA register allocation. | N40 |
| N50 | Phase 4 parallel GC. | N40 (`collect.rs` overlaps.) |
| N62 | Conformance runner for M2/M3/M5. | N60 |
| N63 | Differential tests and GC stress. | N60 |
| N70 | ncl-os. | N24 |
| N73 | ncl-profiler. | N10 |

### Wave C

| N | Scope | Start condition |
|---|---|---|
| N42 | Phase 2. | N41, N13 |
| N52 | Phase 5. | N42, N44-N47, N49 |
| N53 | Phase 6 compiler SIMD. | After N49 and register-class contract freeze. |
| N71 | ncl-uiop. | N24, N29, N70 |
| N72 | ncl-asdf. | N71, N26 |
| N74 | ncl-coverage. | N13, N60 |
| N75 | ncl-debug. | N12, N60 |
| N77 | Library backends and load matrix for M4. | N72, N10, N11, N26 |

D-13's sb-simd and sb-perf contribs are scheduled last on x86-64 Linux. This work is separate from N53 compiler SIMD.
