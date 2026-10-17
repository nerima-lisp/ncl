# Symbol ownership

Ownership is split by implementation crate. Every table lives at
`packages/<crate>/ownership.tsv`; there is no global ownership table. A lane
can therefore add its rows without editing a shared file.

## Table contract

Every table has exactly these seven tab-separated columns:

```text
package	symbol	kind	crate	phase	direct-expansion	notes
```

`kind` is one or more of `function`, `macro`, `special-operator`,
`variable`, `constant`, `type`, `class`, `condition`, and `other`, joined with
`+`. `phase` is `1` for lane implementation, `2` for runtime integration,
or `3` for supporting implementation work. This ownership-table category is
separate from the roadmap's P0-P6 delivery phases and compiler phases.
`direct-expansion` is `yes` only for primitives covered by the
compiler-pipeline contract.

The package column may contain `COMMON-LISP`, `ASDF/INTERFACE`,
`UIOP/DRIVER`, one of the ten frozen NCL extension packages, or a package in
the restored SBCL surface tables:

```text
NCL-THREADS  NCL-FFI      NCL-MOP       NCL-GRAY      NCL-GC
NCL-IMAGE    NCL-UNICODE  NCL-OS        NCL-EXT       NCL-SYS
```

NCL extension rows are intentionally open-ended. The checker validates their
row shape and package names without requiring a fixed symbol list; each
implementing crate owns its symbols. SB-prefixed ownership rows must match the
restored SBCL surface by package, symbol, and kind.

## Coverage rules

The checker reads all `packages/**/ownership.tsv` files and validates every
row. It requires the 978 retained `COMMON-LISP` symbols to be present exactly
once, rejects duplicate `(package, symbol)` pairs, and validates the
seven-column fields. The `COMMON-LISP` set in the SBCL surface table must match
the retained source table; named missing or extra symbols fail the check. Each
SB-prefixed ownership row must name a symbol present
in the SBCL surface tables and use the same kind. Public SBCL surface rows marked
`present=yes` and `external=yes`, plus present rows in
`conformance/sbcl/symbols/internal-referenced.tsv`, are listed by package when
they have no ownership row. The report includes counts and symbol names. Those
missing rows do not fail the checker; unimplemented symbols remain unexported.
The checker also requires the COMMON-LISP, eight public SBCL, and internal
reference tables. It validates contrib names and packages, each loaded table
against its declared external row count, and permits only `sb-simd` and
`sb-perf` to have status `out-of-scope` with reason `platform-not-in-scope`.
The extractor marks those two out of scope unless SBCL reports both `:x86-64`
and `:linux`; the checker validates the recorded metadata but does not
independently inspect the host platform. Any failed contrib status fails the
check.

The ASDF and UIOP surfaces are retained under `ASDF/INTERFACE` and
`UIOP/DRIVER`; NCL extensions are checked for valid shape and package names
without imposing a fixed symbol list.

The machine-checkable gate is:

```sh
python3 conformance/ownership/check.py
```

Use `--package PACKAGE` one or more times to limit the missing SBCL surface
report to selected packages. Ownership validation and COMMON-LISP coverage
remain global.

## Assignment rules

- Core special operators, declarations, lambda-list keywords, and front-end syntax go to `ncl-compiler-front`.
- Macro expansion and function-object operations go to `ncl-lib-macros`; LOOP and FORMAT have dedicated crates.
- Numeric, sequence/list, string/character, array/hash, stream, pathname, package/symbol, conditions, CLOS, and type surfaces go to their dedicated crates.
- Built-in data types go to `ncl-types`; condition classes go to `ncl-conditions`; standard CLOS classes and MOP objects go to `ncl-clos`. A `class+function` row follows its primary kind and records the secondary responsibility in `notes` when needed.
- Evaluation, compilation, loading, feature/module state, implementation-environment functions, REPL/debugger operations, and evaluator/compiler or image hooks go to the runtime or extension crate that implements them.
- NCL extensions are assigned by subsystem: GC/object state to `NCL-GC`, threads and synchronization to `NCL-THREADS`, FFI to `NCL-FFI`, MOP to `NCL-MOP`, Gray streams to `NCL-GRAY`, image lifecycle to `NCL-IMAGE`, Unicode to `NCL-UNICODE`, operating-system services to `NCL-OS`, language extensions to `NCL-EXT`, and internal primitives to `NCL-SYS`.

## Judgement notes

Only boundary cases that required an explicit choice are listed here:

- Built-in stream classes such as `BROADCAST-STREAM` remain in `ncl-types`; their constructors and accessors are in `ncl-lib-streams`.
- `MAKE-LOAD-FORM` and `MAKE-LOAD-FORM-SAVING-SLOTS` are compiler-front load-form expansion hooks.
- `EQ`/`EQL` are numeric-lane direct primitives; `EQUAL`/`EQUALP` are sequence/object comparison utilities.
- `COPY-STRUCTURE` and the `STRUCTURE` marker are CLOS-owned, while list/tree copying remains in `ncl-lib-sequences`.
- `Y-OR-N-P` and `YES-OR-NO-P` are stream-facing interaction functions.
- Package-lock and name-conflict conditions are in `ncl-conditions`; package management functions remain in `ncl-lib-packages`.
- Atomic/hash-table synchronization forms split between `ncl-threads` and `ncl-lib-hash-arrays` by the protected resource.
- File-descriptor stream and event functions are stream-owned; deadline and interrupt functions are thread-owned.
- Image lifecycle functions are reserved for `ncl-image`; shared-object and pinned-object operations are FFI/object-owned.

