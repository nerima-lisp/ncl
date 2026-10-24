# Roadmap

NCL targets compatibility with SBCL 2.6.0. The [wave plan](wave-plan.md) records the surface tables, acceptance criteria, and lane dependencies.

## Decisions D-9 to D-13

| # | Decision | Relation |
|---|---|---|
| D-9 | The compatibility scope includes the public APIs of SB-EXT, SB-THREAD, SB-ALIEN, SB-SYS, SB-MOP, SB-GRAY, SB-UNICODE, and SB-DEBUG, the contribs, and 58 internal symbols referenced by libraries. | D-2, D-3, FR-001 |
| D-10 | `*features*` includes `:ncl` and never `:sbcl`. Libraries load through `#+ncl` backends. | Keeps D-5 |
| D-11 | NCL passes every ansi-test test that SBCL 2.6.0 passes on the same machine and day. For cl-bench, NCL's geometric mean is at most SBCL's, and each of the 65 benchmarks runs within 1.5 times SBCL's time. | Adds ANSI-test and cl-bench acceptance criteria |
| D-12 | Targets are x86-64 and AArch64 on macOS and Linux. | Confirms D-7 |
| D-13 | A contrib is in scope exactly on platforms where SBCL 2.6.0 loads it. The extraction loaded 18 in-scope contribs on macOS arm64; `sb-simd` and `sb-perf` were out of scope there. All 20 contribs loaded on x86-64 Linux, including `sb-simd` (312 external symbols) and `sb-perf` (2). The extractor's 20-entry list has no additional contribs to account for the difference from D-13's stated macOS count. | `conformance/sbcl/symbols/README.md` |

D-2's SB-* exclusion, D-3's contrib exclusion, and FR-001's SBCL-surface removal requirement were withdrawn by D-9.

## Delivery phases

| Phase | Work |
|---|---|
| P0 | Decisions, SBCL surface tables, and the coverage check |
| P0b | Record SBCL baselines as sets of passing test names |
| P1 | ANSI inclusion |
| P2 | SB-* public packages, starting with SB-ALIEN and SB-SYS |
| P3 | Contribs |
| P4 | Performance |
| P5 | ASDF, UIOP, and 14 libraries |
| P6 | Startup time and executable size |

Release follows P6.

## Execution lanes

Gate 0 establishes the shared contracts and integration path. Its six lanes are N01 for the extension API, ownership tables, surface extraction, and coverage check; N02 for IR contract v2; N03 for README and documentation; N04 for integration across the target platforms; N05 for coverage recovery; and N76 for the disassembler. N01 restores and checks SBCL surface data instead of deleting it.

Wave A starts after N01 and N02 freeze: N60 runtime and binary; N27 lib-macros; N20 lib-numbers; N13 compiler-front surface migration; N40 Phase 1b; N10 threads, N11 FFI, N12 conditions, N14 types/reader/printer/image/object, N21 sequences, N22 strings, N24 streams, N25 packages, N26 CLOS; and N43 pass manager and inlining.

Wave B starts when dependencies freeze: N23 hash-arrays; N28 LOOP; N29 pathnames; N30 format; N44 to N47 escape analysis, GVN/SCCP, LICM, and e-graph; N41 Phase 1c; N49 SSA register allocation; N50 parallel GC; N62 conformance runner; N63 differential tests and GC stress; N70 ncl-os; and N73 ncl-profiler.

Wave C covers N42 Phase 2; N52 Phase 5; N53 compiler SIMD; N71 ncl-uiop; N72 ncl-asdf; N74 ncl-coverage; N75 ncl-debug; and N77 library backends and the load matrix. D-13's sb-simd and sb-perf contribs are scheduled last on x86-64 Linux, separately from compiler SIMD.

D-6 limits concurrent lanes to 16, including one integration lane. D-12 targets x86-64 and AArch64 on macOS and Linux. The current CI matrix covers x86-64 Linux and AArch64 macOS.

## Milestones

| ID | Observable state |
|---|---|
| M0 | SBCL surface tables are present. The ownership check rejects mismatched or unknown SB-* rows, reports missing rows by package, and preserves CL 978-symbol coverage and duplicate rejection. |
| M1 | `ncl --eval` defines fib, evaluates `(fib 25)`, and prints `75025` from native execution on the D-12 target platforms. |
| M2 | P0b records SBCL passing test names, and NCL passes every ansi-test test SBCL passes on the same machine and day. |
| M3 | All 65 cl-bench benchmarks run with a geometric mean at most SBCL's and each benchmark within 1.5 times SBCL's time. |
| M4 | The 14 library backends load through NCL's ASDF. |
| M5 | Startup time and executable size meet the specified targets. |
