# Compatibility

NCL targets ANSI Common Lisp. SBCL compatibility is deliberately out of
scope (decision D-2): the implementation creates no `SB-*` packages and no
compatibility shim or alias package. Programs that use SBCL extension
packages must move to the NCL extension API instead.

## Language target

The language target is the 978 external symbols of the `COMMON-LISP`
package. NCL-specific extensions are exposed only through NCL-named
packages: `NCL-THREADS`,
`NCL-FFI`, `NCL-MOP`, `NCL-GRAY`, `NCL-GC`, `NCL-IMAGE`, `NCL-UNICODE`,
`NCL-OS`, `NCL-EXT`, and `NCL-SYS`.

## Acceptance

Acceptance is measured against SBCL 2.6.0's recorded numbers, not against
SBCL compatibility. The reference values, recorded in
`conformance/baselines/`, are:

- ansi-test pass count: 21,768 of 21,942 tests
- cl-bench geometric mean: 0.0755 s
- startup time: 17 ms
- executable size: 82.5 MB
- compile-file speed: 0.08 s median

Each comparison is measured against SBCL again on the same machine and
environment, not against the recorded numbers alone.

## Boundary

The compatibility boundary is defined by the design contracts in
`docs/src/design/` and the conformance runner (`ncl-conformance`). The
retired interpreter, VM, and syntax crates ended at commit `d9bbb4ec` and
offer no backward compatibility.

See the [API reference](api.md) for the current surface, the
[architecture](architecture.md) for the crate layout, and the
[wave plan](../project/wave-plan.md) for how the surface lands.
