# cl-bench Isolation Results

## Measurement contract

The report covers the NCL release build from main revision `215a0f1ff9560e378552b9693ab5d10efca78326` and SBCL 2.6.0. Both runs used the pinned cl-bench revision `553fbcdf88d2ca4e340a0cdf02679055f3279c8f` and the shared supervisor in `per-benchmark` mode. The NCL run completed before the scoreboard workflow's 1,800-second timeout; its exact elapsed time was not captured.

The comparison denominator is 64 common benchmark IDs. The configured 65th row, `WALK-LIST/MESS`, is retained as a separate row because cl-bench marks it disabled for SBCL. Row IDs use a `DEFBENCH` registration’s `:short` value when present, or its symbol name otherwise. Seven CLOS registrations map to `clos-janderson.lisp`; `EQL-SPECIALIZED-FIB` maps to `clos.lisp`.

Each source was checked in a fresh child process. A preflight loads earlier sources that passed their checks before it loads the source under test. Each selected benchmark then ran in its own child process after the package bootstrap and successful source loads. This mode kept both complete runs below the scoreboard workflow timeout of 1,800 seconds.

| Engine | Rows | Result stages | Elapsed | Samples | Exit |
|---|---:|---|---:|---:|---:|
| NCL, 64 common IDs | 64 | 37 failed/load, 27 failed/execute | <1,800 s | 0 | 1 |
| SBCL, 64 common IDs | 64 | 58 passed/finish, 6 failed/execute | 151.458 s | 5,007 | 1 |

## Common benchmark rows

`NCL` and `SBCL` columns show `stage/status`. Findings keep the captured failure category and name an assigned stream when the source mapping establishes one. GC unit 1 owns SIGSEGV; reader `#.` owns that reader failure; R6 B7 owns the SETF macroexpansion failures. The CLOS type-specifier fix for the named class element type is in main; the current `DEFMETHOD` macroexpansion failure has no assigned stream. Condition details remain in the per-row JSON output. A blank SBCL finding cell means the JSON condition is `null`.

| Benchmark | NCL | NCL finding and owner | SBCL | SBCL finding and owner |
|---|---|---|---|---|
| `1D-ARRAYS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `2D-ARRAYS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `3D-ARRAYS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `ACKERMANN` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BENCH-STRINGS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BIGNUM/ELEM-100-1000` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BIGNUM/ELEM-1000-100` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BIGNUM/ELEM-10000-1` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BIGNUM/PARI-100-10` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BIGNUM/PARI-200-5` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BITVECTORS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BOEHM-GC` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `BOYER` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `BROWSE` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `CLOS/complex-methods` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | execute/failed | no applicable method for `complex-method`; owner `clos-janderson.lisp` |
| `CLOS/defclass` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | finish/passed |  |
| `CLOS/defmethod` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | execute/failed | missing class `CLASS-2-0`; owner `clos-janderson.lisp` |
| `CLOS/instantiate` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | execute/failed | missing class `CLASS-10-0`; owner `clos-janderson.lisp` |
| `CLOS/method+after` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | execute/failed | missing class `CLASS-2-0`; owner `clos-janderson.lisp` |
| `CLOS/methodcalls` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | execute/failed | no applicable method for `simple-method`; owner `clos-janderson.lisp` |
| `CLOS/simple-instantiate` | load/failed | `DEFMETHOD` macroexpansion TypeError; no assigned stream | finish/passed |  |
| `COMPILER` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `CRC40` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `CTAK` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `DDERIV` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `DEFLATE-FILE` | load/failed | INCF macroexpansion (`UndefinedFunction`); no assigned owner identified | execute/failed | `files/message.gz` absent; no assigned owner identified |
| `DERIV` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `DESTRUCTIVE` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `DIV2-TEST-1` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `DIV2-TEST-2` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `EQL-SPECIALIZED-FIB` | load/failed | READER-ERROR at `#.`; reader `#.` stream | finish/passed |  |
| `FACTORIAL` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `FFT` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `FIB` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `FIB-RATIO` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `FPRINT/PRETTY` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `FPRINT/UGLY` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `FRPOLY/BIGNUM` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `FRPOLY/FIXNUM` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `FRPOLY/FLOAT` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `HASH-INTEGERS` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `HASH-STRINGS` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `LOAD-FASL` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `MANDELBROT/COMPLEX` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `MANDELBROT/DFLOAT` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `MRG32K3A` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `PI-ATAN` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `PI-DECIMAL/BIG` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `PI-DECIMAL/SMALL` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `PI-RATIOS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `PUZZLE` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `RICHARDS` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `SEARCH-SEQUENCE` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `SLURP-LINES` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `STAK` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `STRING-CONCAT` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |
| `SUM-PERMUTATIONS` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `TAK` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `TAKL` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `TRAVERSE` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `TRIANGLE` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `TRTAK` | load/failed | READER-ERROR (`Unsupported`); no assigned owner identified | finish/passed |  |
| `WALK-LIST/SEQ` | load/failed | SETF macroexpansion (`UndefinedFunction`); R6 B7 | finish/passed |  |
| `fill-strings/adjustable` | execute/failed | SIGSEGV; GC unit 1 | finish/passed |  |

## Additional SBCL-disabled row

| Benchmark | NCL | SBCL |
|---|---|---|
| `WALK-LIST/MESS` | `load/failed`, `misc.lisp` SETF macroexpansion failure owned by R6 B7 | `finish/disabled`, marked disabled by the pinned suite |

## Failure classification

- NCL `execute`: 27 child workers ended with SIGSEGV. The assigned owner is GC unit 1.
- NCL CLOS `load`: 7 rows fail during `DEFMETHOD` macroexpansion in `clos-janderson.lisp`. A minimal release-binary probe passes the named class array element type and a simple method, then fails on a custom `and` method combination with the same TypeError. The named class element-type issue is fixed in main; no stream is assigned to this current failure.
- NCL `EQL-SPECIALIZED-FIB` `load`: `clos.lisp` contains `#.` forms and fails with `READER-ERROR`. The assigned owner is the reader `#.` stream.
- NCL `gabriel.lisp` `load`: 21 rows fail with `READER-ERROR` and an `Unsupported` condition. The captured run does not identify a specific reader form or an assigned owner.
- NCL `SETF` macroexpansion `load`: 7 common rows fail in `hash.lisp` or `misc.lisp`. The assigned owner is SETF place (R6 B7). The additional `WALK-LIST/MESS` row has the same source failure.
- NCL `DEFLATE-FILE` `load`: `INCF` macroexpansion reports `UndefinedFunction`; no assigned owner was identified.
- SBCL CLOS `execute`: five rows fail in `clos-janderson.lisp` due to missing classes or no applicable methods.
- SBCL `DEFLATE-FILE` `execute`: the fixture `files/message.gz` is absent; no assigned owner was identified.

## Commands and captures

Both engines used `python3 conformance/cl-bench/driver_supervisor.py` with `--process-mode per-benchmark`, the engine-specific Lisp driver, the pinned cl-bench checkout, and the 65 configured rows, of which 64 are common comparison IDs. NCL used the release binary built from the main revision above; SBCL used version 2.6.0.

Raw captures are local artifacts, not committed to this repository. The current-main NCL workflow result is `/tmp/ncl-clbench-run-repro/scoreboard-main-215a-final.json`; it contains all 65 configured benchmark rows and confirms the workflow completed before its timeout. The NCL `ncl-per-benchmark-final.*` files are from the earlier `0fb1d76e` main run and are not used for this table. The SBCL run artifacts are `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.stdout.json`, `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.stderr.log`, `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.elapsed-seconds`, `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.exit`, `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.command.json`, `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.rows.csv`, and `/tmp/ncl-clbench-run-repro/sbcl-per-benchmark-final.summary.json`.

| Check | Command | Exit |
|---|---|---:|
| NCL workflow-level scoreboard run | `python3 scripts/conformance_scoreboard.py --timeout 1800` with the release runner, shared driver, and pinned checkout | 0 for scoreboard collection; embedded cl-bench driver returned 1 with all row failures retained |
| SBCL full supervisor run | `python3 conformance/cl-bench/driver_supervisor.py --engine sbcl --process-mode per-benchmark` with SBCL 2.6.0, SBCL driver, pinned checkout, default mode | 1, six row failures retained |
