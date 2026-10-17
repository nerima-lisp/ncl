# Conformance data

`ownership/` documents symbol ownership by crate and the gate that checks the
retained `COMMON-LISP` symbols. `libraries/` records the fixed revisions used
by the library-load matrix. `baselines/` contains ansi-test, cl-bench, startup,
size, and compile-file reference values.

The harness fetches ansi-test and cl-bench into a temporary directory outside
the source tree. Measurements are tied to their date and environment, so
compare them with a baseline made on the same machine and day. Published files
must not include host names, user names, company names, or absolute home paths.

## Scoreboard runner

`python3 scripts/conformance_scoreboard.py` downloads the pinned upstream
repositories into a cache outside the source tree and supervises one command
per suite. Commands must print a JSON object to standard output: ansi-test
prints `passed`, `failed`, and `unexecuted`; cl-bench prints a positive
`times` array in seconds. Assertion failures remain measurement results;
timeouts and signal termination remain harness failures.

The upstream sources are not vendored. ansi-test is pinned to
`ca06bd919661af162c67407c9d994e881870bdb3`, and cl-bench is pinned to
`553fbcdf88d2ca4e340a0cdf02679055f3279c8f`; the harness retries the
ansi-test GitLab project before falling back to matching GitHub mirrors. Each
checkout is verified against its exact commit before use. The license files
remain in the temporary checkout.

## ansi-test inclusion comparison

The checked-in SBCL baseline is a same-day macOS arm64 measurement made with
SBCL 2.6.0. It contains the sorted names of tests that pass under SBCL and a
TSV map from each name to its source chapter. Generate or refresh it on the
same machine and day as the NCL comparison:

```sh
python3 scripts/ansi_test_inclusion.py baseline \
  --command 'sbcl --noinform --userinit /dev/null --load' \
  --cache-dir "$TMPDIR/ncl-conformance" \
  --output-prefix conformance/baselines/ansi-test-sbcl-2.6.0-macos-arm64
```

This command requires exactly SBCL 2.6.0 and the pinned ansi-test commit. It
records the date, platform, runtime, registration counts, and pass/fail counts
in the adjacent Markdown file. The `.txt` file is the sorted passing-name
list; the `.tsv` file maps those names to source directories. The metadata
explains why the runner removes two tests and why SBCL disables four tests by
note. On Linux, omit `--output-prefix`; the command chooses a
`linux-x86_64` filename automatically. Other platforms use their own platform
name in the generated filename. Before each baseline or chapter process, the
runner removes the pinned suite's `sandbox/scratch` directory.
`ENSURE-DIRECTORIES-EXIST.8` leaves that directory behind, which would
otherwise make later measurements depend on run order.

Build the release binary with
`nix develop --command cargo build --locked --release`, then run this one
comparison command on the same machine and day as the baseline generation:

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
tests are still listed with their timeout or harness-error status. The command
returns exit status 1 when any T-ANSI test exists; the Markdown report is still
printed. Interpreter startup failure classifies all baseline names as harness
errors and preserves the first diagnostic.

The Linux x86-64 workflow has no same-day SBCL 2.6.0 baseline. Its scoreboard
records ansi-test as `unmeasured` with a reason and still runs cl-bench. To
produce an inclusion comparison on Linux, generate an SBCL baseline on that
machine and day. The default output is
`conformance/baselines/ansi-test-sbcl-2.6.0-linux-x86_64.txt`; pass that path
as `--baseline` in the comparison command. The ANSI scoreboard remains
separate from `ci.yml`.
