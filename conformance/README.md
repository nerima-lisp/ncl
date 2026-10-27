# Conformance data

[`ownership/README.md`](ownership/README.md) documents symbol ownership by
crate and the coverage gate. Run `python3 conformance/ownership/check.py` to
check retained `COMMON-LISP` symbols. `libraries/` records the fixed revisions
used by the library-load matrix. `baselines/` contains ansi-test, cl-bench,
startup, size, and compile-file reference values.

Measurements are tied to their date and environment, so compare them with a
baseline made on the same machine and day. Published files must not include
host names, user names, company names, or absolute home paths.

Run the commands below from the repository root.

## Scoreboard runner

The scoreboard runner requires caller-supplied commands for both
`--ansi-command` and `--cl-bench-command`; it has no default suite commands.
The ANSI command must print a JSON object with `passed`, `failed`, and
`unexecuted` counts, for example `{"passed": 1, "failed": 0, "unexecuted": 0}`.
The cl-bench command must print a JSON object containing `status`, `times`, and
`benchmarks`. Each benchmark entry records its `status`, `stage`, `times`, and
`condition`; for example:

```json
{
  "status": "failed",
  "times": [1.25],
  "benchmarks": {
    "EXAMPLE/PASSED": {
      "status": "passed",
      "stage": "finish",
      "times": [1.25],
      "condition": null
    },
    "EXAMPLE/FAILED": {
      "status": "failed",
      "stage": "load",
      "times": [],
      "condition": "load error details"
    }
  }
}
```

Stages are `load`, `register`, `execute`, and `finish`. A passed entry reaches
`finish` and has positive sample times; a failed entry keeps the stage and
condition that stopped it. The flat `times` array contains the samples from
passed entries. Disabled entries remain in `benchmarks` without sample times.
The runner executes each command from its pinned suite checkout. Assertion
failures remain measurement results; timeouts and signal termination remain
harness failures.

`ncl-driver.sh` and the paired SBCL baseline use
`conformance/cl-bench/driver_supervisor.py` in per-benchmark mode. The supervisor
bootstraps package declarations from every source file, preflights each source
in its own runner process, then starts one runner for each selected benchmark.
A failed source marks its benchmarks as failed while workers for other sources
continue. NCL and SBCL use the same source and benchmark process boundaries.
For the scoreboard command, `--cl-bench-command` can invoke
`conformance/cl-bench/ncl-driver.sh` with absolute paths to the repository and
NCL executable, followed by the optimization mode, as in
`.github/workflows/scoreboard.yml`.

The command below is a template: replace both placeholders with commands that
satisfy these output contracts.

```sh
python3 scripts/conformance_scoreboard.py \
  --ansi-command '<ANSI runner command>' \
  --cl-bench-command '<cl-bench runner command>'
```

The cache defaults to `.cache/conformance` relative to the current working
directory, normally the repository root. It stores the pinned checkouts in
`ansi-test/` and `cl-bench/` below that directory; `--cache-dir` selects a
different location.

The upstream sources are not vendored. ansi-test is pinned to
`ca06bd919661af162c67407c9d994e881870bdb3`, and cl-bench is pinned to
`553fbcdf88d2ca4e340a0cdf02679055f3279c8f`; the harness retries the
ansi-test GitLab project before falling back to matching GitHub mirrors. Each
checkout is verified against its exact commit before use. The license files
remain in the cache checkout.

## Paired optimization baseline

Build the release binary, then run the paired baseline to measure NCL and SBCL
on the current machine. The paired runner is
`scripts/optimization_baseline.py`; it invokes the shared supervisor for both
engines. The command runs both benchmark processes, and `--sbcl` selects the
SBCL 2.6.0 executable. The suggested output filename is
`conformance/scoreboard/u04-optimization.json`. Omit `--output` to print the JSON
report to standard output or choose another path:

```sh
nix develop --command cargo build --locked --release -p ncl
python3 scripts/optimization_baseline.py \
  --ncl target/release/ncl \
  --sbcl sbcl \
  --cache-dir "$TMPDIR/ncl-conformance" \
  --warmups 1 \
  --pairs 3 \
  --sbcl-samples 3 \
  --output conformance/scoreboard/u04-optimization.json
```

Each run uses the pinned cl-bench commit and records each benchmark's status
and raw times for every process. `conformance/cl-bench/ncl-driver.sh` takes the
NCL binary path followed by `off` or `default`, and passes the mode as
`--optimization=<mode>`. The harness warms up both modes, then measures the
same binary in `off`, `default`, `default`, `off`, `off`, `default` order. It
also warms up SBCL 2.6.0 and measures three independent processes. The report
records the build revision, platform, source commit, and measurement date. It
computes geometric means from the positive benchmark values shared by NCL
`off`, NCL `default`, and SBCL;
`WALK-LIST/MESS`, which SBCL disables, is listed separately.
Aggregate and per-benchmark NCL/SBCL ratios divide NCL time by SBCL time, so a
ratio above 1 means NCL takes longer and a ratio below 1 means it takes less
time.

## ansi-test inclusion comparison

The checked-in SBCL baseline is a same-day macOS arm64 measurement made with
SBCL 2.6.0. It contains the sorted names of tests that pass under SBCL and a
TSV map from each name to its source chapter. Generate or refresh it on the
same machine and day as the NCL comparison:

For both baseline generation and comparison, `--command` takes an interpreter
command prefix. The runner appends a generated Lisp runner file to each
process invocation.

```sh
python3 scripts/ansi_test_inclusion.py baseline \
  --command 'sbcl --noinform --userinit /dev/null --load' \
  --cache-dir "$TMPDIR/ncl-conformance" \
  --output-prefix conformance/baselines/ansi-test-sbcl-2.6.0-macos-arm64
```

This command requires exactly SBCL 2.6.0 and the pinned ansi-test commit. The
`--output-prefix` value is a filename stem: the runner adds `.txt`, `.tsv`, and
`.md`. The `.txt` file lists passing test names in sorted order; `.tsv` maps
each name to its source chapter; `.md` records the date, platform, runtime,
registration and result counts, and names the four tests marked disabled by
SBCL in the source. When comparison
reads the `.txt` baseline, it also reads the matching `.tsv` and `.md` files.
On Linux, omit `--output-prefix`; the command chooses a
`linux-x86_64` filename automatically. Other platforms use their own platform
name in the generated filename. Before each baseline or chapter process, the
runner removes the pinned suite's `sandbox/scratch` directory.
`ENSURE-DIRECTORIES-EXIST.8` leaves that directory behind, which would
otherwise make later measurements depend on run order.

Build the release binary with
`nix develop --command cargo build --locked --release`, then run the comparison
command:

```sh
python3 scripts/ansi_test_inclusion.py compare \
  --command "$PWD/target/release/ncl --load" \
  --baseline conformance/baselines/ansi-test-sbcl-2.6.0-macos-arm64.txt \
  --cache-dir "$TMPDIR/ncl-conformance" \
  --format markdown
```

The comparator checks the baseline date, platform, SBCL version, and ansi-test
source commit before running. It starts a separate NCL process for each
top-level ANSI chapter loader and reports tests under their source directories,
including nested directories such as `printer/format`. A timeout or harness
error affects that loader's baseline-passing names without stopping other
loaders. The report lists every baseline-passing test that NCL fails, times
out, cannot run because the harness fails, or leaves unexecuted. A `partial`
measurement means one or more chapter loaders did not finish; the affected
tests are still listed with their timeout or harness-error status. The report
calls the set `T-ANSI`; it contains tests that pass in the SBCL baseline but
not under NCL. The command returns exit status 1 when that set is nonempty, but
still prints the Markdown report. Interpreter startup failure classifies all
baseline names as harness errors and preserves the first diagnostic.

The Linux x86-64 workflow has no same-day SBCL 2.6.0 baseline.
`.github/workflows/scoreboard.yml` records ansi-test as `unmeasured` with a
reason and runs cl-bench; it does not run the same-day ANSI inclusion
comparison. To produce an inclusion comparison on Linux, generate an SBCL
baseline on that machine and day. The default output is
`conformance/baselines/ansi-test-sbcl-2.6.0-linux-x86_64.txt`; pass that path
as `--baseline` in the comparison command. The ANSI scoreboard workflow is
separate from `.github/workflows/ci.yml`.
