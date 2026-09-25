# Architecture

NCL is a Rust-native Common Lisp implementation with a native compiler
pipeline. There is no interpreter and no stack-bytecode VM. The former
interpreter, VM, and syntax crates are removed; their history ends at
commit `d9bbb4ec`.

## Crate groups

The workspace is split into layers, each with a fixed responsibility:

| group | crates |
| --- | --- |
| unsafe platform | `ncl-sys` |
| typed object layer | `ncl-object`, `ncl-types` |
| language front end | `ncl-reader`, `ncl-printer`, `ncl-conditions`, `ncl-clos`, `ncl-compiler-front` |
| machine backend | `ncl-ir`, `ncl-codegen`, `ncl-asm-x86-64`, `ncl-asm-aarch64`, `ncl-objfile` |
| library crates | `ncl-lib-*` |
| integration | `ncl-threads`, `ncl-ffi`, `ncl-image`, `ncl-runtime`, `ncl-conformance`, and the root `ncl` binary |

The unsafe platform crate owns pages, threads, OS declarations, roots, and
code space. The typed object layer turns raw handles into allocation and
typed accessors. The language front end reads, prints, and lowers Lisp
forms. The machine backend encodes, relocates, and writes native code for
x86-64 and AArch64. The library crates register the ANSI Common Lisp
library surface. The integration crates wire execution, images, the
conformance runner, and the command-line interface together.

The workspace uses zero external crates. `unsafe` is confined to
`ncl-sys`; every other crate forbids it.

The crate adjacency list and public API summary are in the
[crates contract](../design/crates.md). The remaining design contracts
live in `docs/src/design/`, and the lane plan that builds this
architecture is the [wave plan](../project/wave-plan.md).
