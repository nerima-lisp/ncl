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
or `3` for supporting implementation work. `direct-expansion` is `yes` only
for primitives covered by the compiler-pipeline contract.

The package column may contain `COMMON-LISP`, `ASDF/INTERFACE`,
`UIOP/DRIVER`, or one of the eleven frozen NCL extension packages:

```text
NCL-THREADS  NCL-FFI      NCL-MOP       NCL-GRAY      NCL-GC
NCL-IMAGE    NCL-UNICODE  NCL-OS        NCL-EXT       NCL-SYS       NCL-PROFILER
```

NCL extension rows are intentionally open-ended. Their count and individual
symbols are owned by the implementing crate and are not treated as extra
rows. Packages beginning with `SB-` are forbidden.

## Coverage rules

The checker reads all `packages/**/ownership.tsv` files and validates every
row. It requires the 978 retained `COMMON-LISP` symbols to be present exactly
once, rejects duplicate `(package, symbol)` pairs, rejects `SB-` packages,
and validates the seven-column fields. The ASDF and UIOP surfaces are retained
under `ASDF/INTERFACE` and `UIOP/DRIVER`; NCL extensions are checked for valid
shape and package names without imposing a fixed symbol list.

The machine-checkable gate is:

```sh
python3 conformance/ownership/check.py
```

## Assignment rules

- Core special operators, declarations, lambda-list keywords, and front-end syntax go to `ncl-compiler-front`.
- Macro expansion and function-object operations go to `ncl-lib-macros`; LOOP and FORMAT have dedicated crates.
- Numeric, sequence/list, string/character, array/hash, stream, pathname, package/symbol, conditions, CLOS, and type surfaces go to their dedicated crates.
- Built-in data types go to `ncl-types`; condition classes go to `ncl-conditions`; standard CLOS classes and MOP objects go to `ncl-clos`. A `class+function` row follows its primary kind and records the secondary responsibility in `notes` when needed.
- Evaluation, compilation, loading, feature/module state, implementation-environment functions, REPL/debugger operations, and evaluator/compiler or image hooks go to the runtime or extension crate that implements them.
- NCL extensions are assigned by subsystem: GC/object state to `NCL-GC`, threads and synchronization to `NCL-THREADS`, FFI to `NCL-FFI`, MOP to `NCL-MOP`, Gray streams to `NCL-GRAY`, image lifecycle to `NCL-IMAGE`, Unicode to `NCL-UNICODE`, operating-system services to `NCL-OS`, language extensions to `NCL-EXT`, statistical profiling to `NCL-PROFILER`, and internal primitives to `NCL-SYS`.

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

The crate-local tables are the source of truth for ownership. The checker
verifies the retained COMMON-LISP coverage, duplicate rows, package policy,
and valid crate/phase fields; it does not require a fixed count for NCL
extension rows.
