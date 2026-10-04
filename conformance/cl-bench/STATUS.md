# cl-bench status

## Current state

The driver loads through `deflate.lisp`. The `destructive` compiler regression
was fixed and is covered by the runtime test suite. The complete 65-benchmark
run was not completed in this handoff.

The earlier load appeared to stop at `fprint-init 6 6` and reached about 585 MB
RSS. Isolated runs of `fprint-init` at `2 2`, `3 3`, `4 4`, `5 5`, and `6 6`
all exited with status 0 within the 120-second limit and used about 7.2 to
8.2 MB RSS. The expected recursive call counts are 3, 15, 31, 364, and 1,093
respectively. Form-level tracing showed that `fprint-init` completed during
the full load; the next observed stop was the Puzzle `DEFTYPE` form in
`gabriel.lisp`. `DEFTYPE` had been marked as a compiler-owned macro but had no
expansion implementation. The front end now treats it like `DECLAIM`, as a
non-runtime declaration, with a regression test.

## Reproduction

From the pinned cl-bench checkout, run the driver with the release binary built
from this repository:

```sh
"$NCL_REPO/.target-c2-clbench/release/ncl" \
  --eval '(load "$CL_BENCH_ROOT/conformance/cl-bench/ncl-driver.lisp")'
```

For isolated initializer measurements, load
`packages/runtime/tests/fixtures/fprint_init.lisp` and evaluate
`(progn (fprint-init N N '(a b c d e f g h)) 42)` with `N` set to each size.
Each process was limited to 120 seconds and measured with `/usr/bin/time -l`.

## Next work

Rebuild the release binary containing `c78bae86`, rerun the complete load, then
run all 65 benchmarks individually with per-benchmark status and timing before
running the full driver. Commit the resulting table with the measurement
conditions and verify PR #56 CI.
