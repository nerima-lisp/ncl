# SBCL surface tables

Regenerate the tables from the repository root with:

```sh
nix develop --command sbcl --script conformance/sbcl/extract-surface.lisp
```

See [`symbols/README.md`](symbols/README.md) for the schema, counts, and provenance.

D-13 states that 20 contribs are in scope on macOS arm64 and schedules `sb-simd` and `sb-perf` for x86-64 Linux. This macOS arm64 extraction loaded 18 of the historical 20 SBCL contrib entries and marked those two out of scope. The excluded entries do not reconcile D-13's 20 in-scope contribs; the historical inventory has no two additional names to close that gap. The x86-64 Linux extraction has not run. ASDF and UIOP are recorded separately.

The extractor marks `sb-simd` and `sb-perf` out of scope unless SBCL reports both `:x86-64` and `:linux`. The ownership checker validates this recorded status and reason but does not independently inspect the host platform.
