# Roadmap

NCL is being rewritten as a native Common Lisp implementation with no SBCL
compatibility layer. Implementation runs as one Gate 0 phase plus three waves;
the decisions, requirements, and per-lane acceptance criteria are in the
[wave plan](wave-plan.md).

- Gate 0 starts six lanes first: the extension API contract and ownership-table split (N01), IR contract v2 (N02), slop removal and documentation (N03), the resident integration lane (N04), coverage recovery (N05), and the ncl-disasm decoder (N76).
- Wave A fills the free slots in priority order once N01/N02 freeze: runtime plus binary with M1 (N60), lib-macros A (N27), lib-numbers (N20), compiler-front surface migration (N13), Phase 1b (N40), the library and language crate lanes (N10, N11, N12, N14, N21, N22, N24, N25, N26), and the pass manager with inlining (N43).
- Wave B starts each lane once its dependencies freeze: hash-arrays, LOOP, pathnames, format, the Phase 3 passes (N44 to N47), Phase 1c, SSA register allocation, parallel GC, the conformance runner, differential testing, ncl-os, and ncl-profiler.
- Wave C holds the remaining performance phases and tooling: Phase 2, Phase 5, Phase 6 SIMD, ncl-uiop, ncl-asdf, ncl-coverage, ncl-debug, and the library backends with the M4 load matrix.

Milestones run M0 to M5: M0 removes the SB- surface and makes the ownership
check cover all 978 CL symbols; M1 runs fib(25) from source natively and prints
75025; M2 completes ansi-test and reports pass, fail, and unexecuted counts with
a commit hash; M3 runs all 65 cl-bench benchmarks; M4 loads all 14 libraries; M5
beats SBCL's numbers for FR-002 and FR-007.

At most 16 lanes run at once, one of them dedicated to integration. CI covers
x86-64 only; the integration lane catches AArch64 regressions with local arm64
runs at every merge.
