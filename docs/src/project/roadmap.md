# Roadmap

NCL is in active parallel implementation. The executable path already
evaluates source text natively. The remaining work is grouped by the
milestones below; lane ownership and dependencies are in the
[wave plan](wave-plan.md).

| Milestone | Observable result |
| --- | --- |
| M0 | ANSI ownership is complete and no `SB-*` compatibility surface exists. |
| M1 | `ncl --eval` evaluates recursive `fib(25)` from source and prints `75025`. |
| M2 | ansi-test reports pass, fail, and unexecuted counts. |
| M3 | cl-bench completes its current benchmark set. |
| M4 | The 14-library load matrix succeeds. |
| M5 | Conformance and performance gates meet their recorded targets. |

Current parallel lanes include ansi-runner, cl-bench, loop-macros, clos-mop,
opt-core, regalloc, parallel-gc, threads, os-ffi, image-startup,
disasm-debug, and documentation/slop removal. Follow-up lanes cover the ANSI
chapters, escape analysis and LICM, later optimization phases, ASDF/UIOP,
profiling and coverage, and the library load matrix.

Measured claims must include the command, platform, and recorded baseline.
See `conformance/` for baseline and scoreboard data.
