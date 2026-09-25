# Extension API

NCL extensions are workspace crates with explicit ownership and dependency
boundaries. They use only path dependencies and must not introduce external
crates or reverse dependencies.

## Frozen package names

The public extension packages are `NCL-THREADS`, `NCL-FFI`, `NCL-MOP`,
`NCL-GRAY`, `NCL-GC`, `NCL-IMAGE`, `NCL-UNICODE`, `NCL-OS`, `NCL-EXT`, and
`NCL-SYS`. Their owning crates are, in order, `ncl-threads`, `ncl-ffi`,
`ncl-clos`, `ncl-lib-streams`, `ncl-object`, `ncl-image`, `ncl-lib-strings`,
`ncl-os`, `ncl-compiler-front`, and `ncl-sys`.

`NCL-` packages are the only extension namespace. The ASDF and UIOP package
names remain `ASDF/INTERFACE` and `UIOP/DRIVER`.

## Ownership tables

Each owning crate stores a seven-column TSV at `packages/<crate>/ownership.tsv`:
`package`, `symbol`, `kind`, `crate`, `phase`, `direct-expansion`, and `notes`.
The `crate` column is retained so a row is self-describing. COMMON-LISP has
978 rows; ASDF/INTERFACE has 230 rows; UIOP/DRIVER has 427 rows. SB-prefixed
packages and SBCL contrib rows are not part of the contract.

`conformance/ownership/check.py` reads all split tables and checks retained
surface coverage, duplicate `(package, symbol)` keys, valid fields, and the
absence of SB-prefixed packages.

## Registration order

`ncl-stdlib::register_all` is the sole standard-library registration entry
point. It calls registrations in dependency order:

1. `ncl-types`
2. `ncl-reader`
3. `ncl-printer`
4. `ncl-conditions`
5. `ncl-clos`
6. `ncl-lib-numbers`
7. `ncl-lib-sequences`
8. `ncl-lib-strings`
9. `ncl-lib-hash-arrays`
10. `ncl-lib-streams`
11. `ncl-lib-pathnames`
12. `ncl-lib-packages`
13. `ncl-lib-format`
14. `ncl-lib-macros`
15. `ncl-threads`
16. `ncl-ffi`
17. `ncl-image`
18. `ncl-os`
19. `ncl-uiop`
20. `ncl-asdf`
21. `ncl-profiler`
22. `ncl-coverage`
23. `ncl-debug`
24. `ncl-disasm`

Crates without a landed registration implementation contribute no call until
their lane lands. No extension crate calls another crate's registration
function directly.

## Dependency graph

```text
ncl-os -> ncl-object, ncl-sys, ncl-conditions, ncl-lib-streams
ncl-uiop -> ncl-object, ncl-conditions, ncl-lib-streams, ncl-lib-pathnames,
           ncl-lib-strings, ncl-os
ncl-asdf -> ncl-object, ncl-conditions, ncl-clos, ncl-uiop
ncl-profiler -> ncl-object, ncl-sys, ncl-threads, ncl-conditions
ncl-coverage -> ncl-object, ncl-compiler-front
ncl-debug -> ncl-object, ncl-sys, ncl-conditions, ncl-codegen, ncl-lib-streams
ncl-disasm -> ncl-object
```

`ncl-disasm` uses `ncl-asm-x86-64` and `ncl-asm-aarch64` only as development
dependencies for round-trip tests.
