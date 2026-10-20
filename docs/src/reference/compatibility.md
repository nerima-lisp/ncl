# Compatibility

NCL targets ANSI Common Lisp and the selected SBCL public APIs, contribs, and
58 library-referenced internal symbols recorded by D-9 in the
[wave plan](../project/wave-plan.md). SBCL 2.6.0 supplies the conformance and
performance baselines. NCL has its own implementation and extension API.

## Language target

The ANSI language target is the 978 external symbols of the `COMMON-LISP`
package. The selected SBCL surface is enumerated in the wave plan and
conformance tables. NCL-specific extensions are exposed through NCL-named
packages: `NCL-THREADS`,
`NCL-FFI`, `NCL-MOP`, `NCL-GRAY`, `NCL-GC`, `NCL-IMAGE`, `NCL-UNICODE`,
`NCL-OS`, `NCL-EXT`, and `NCL-SYS`.

## Acceptance

Acceptance uses same-day comparisons with SBCL 2.6.0. The reference values
recorded in `conformance/baselines/` are:

- ansi-test pass count: 21,768 of 21,942 tests
- cl-bench geometric mean: 0.0755 s
- startup time: 17 ms
- executable size: 82.5 MB

Each comparison is measured against SBCL again on the same machine and
environment, not against the recorded numbers alone.

## Boundary

The intended compatibility boundary is defined by the design contracts in
`docs/src/design/` and summarized in the wave plan. The
`ncl-conformance` runner is planned under lane N62. The retired interpreter,
VM, and syntax crates ended at commit `d9bbb4ec` and offer no backward
compatibility.

See the [API reference](api.md) for the current surface, the
[architecture](architecture.md) for the crate layout, and the
[wave plan](../project/wave-plan.md) for how the surface lands.
