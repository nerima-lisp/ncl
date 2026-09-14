# Symbol ownership

## `symbols.tsv`

Each row assigns one source surface symbol to one implementation crate. `kind` is the `+`-joined set of positive SBCL extraction flags; `other` covers contrib/internal entries without an ANSI kind. `phase` is 1 for lane implementation, 2 for runtime integration, and 3 for contrib/internal compatibility. `direct-expansion` is `yes` only for the primitive set in `docs/src/design/compiler-pipeline.md`.

The checker compares `(package, symbol)` against every symbol row in `conformance/sbcl/symbols/`, including contrib tables and `internal-referenced.tsv`; `contribs.tsv` is metadata and is not itself a symbol surface.

## Assignment rules

- Core special operators, declarations, lambda-list keywords, and front-end syntax go to `ncl-compiler-front`.
- Macro expansion and function-object operations go to `ncl-lib-macros`; LOOP and FORMAT have dedicated crates.
- Numeric, sequence/list, string/character, array/hash, stream, pathname, package/symbol, conditions, CLOS, and type surfaces go to their dedicated Phase 1 crates.
- Built-in data types go to `ncl-types`; condition classes go to `ncl-conditions`; standard CLOS classes and MOP objects go to `ncl-clos`. A `class+function` row follows its primary kind and records the secondary responsibility in `notes` when needed.
- Evaluation, compilation, loading, feature/module state, implementation-environment functions, REPL/debugger operations, and the listed evaluator/compiler or image hooks go to Phase 2 `ncl-runtime`.
- SB-EXT and SB-SYS extensions are assigned by subsystem: GC/object state to `ncl-object`, threads/timers/deadlines/processes to `ncl-threads`, external formats and fd streams to `ncl-lib-streams`, package extensions to `ncl-lib-packages`, compiler extensions to `ncl-compiler-front`, and SAP/alien/image operations to `ncl-ffi` or `ncl-image`.
- Contrib and internal references use Phase 3 reservation crates named `ncl-contrib-<name>`.

## Counts by crate and phase

| crate | phase | count |
| --- | ---: | ---: |
| `ncl-clos` | 1 | 147 |
| `ncl-compiler-front` | 1 | 151 |
| `ncl-conditions` | 1 | 129 |
| `ncl-contrib-alexandria` | 3 | 2 |
| `ncl-contrib-asdf` | 3 | 230 |
| `ncl-contrib-babel` | 3 | 1 |
| `ncl-contrib-cffi` | 3 | 7 |
| `ncl-contrib-sb-aclrepl` | 3 | 7 |
| `ncl-contrib-sb-bsd-sockets` | 3 | 79 |
| `ncl-contrib-sb-capstone` | 3 | 121 |
| `ncl-contrib-sb-cltl2` | 3 | 9 |
| `ncl-contrib-sb-concurrency` | 3 | 37 |
| `ncl-contrib-sb-cover` | 3 | 13 |
| `ncl-contrib-sb-executable` | 3 | 2 |
| `ncl-contrib-sb-gmp` | 3 | 39 |
| `ncl-contrib-sb-grovel` | 3 | 1 |
| `ncl-contrib-sb-introspect` | 3 | 26 |
| `ncl-contrib-sb-md5` | 3 | 16 |
| `ncl-contrib-sb-mpfr` | 3 | 130 |
| `ncl-contrib-sb-posix` | 3 | 547 |
| `ncl-contrib-sb-queue` | 3 | 9 |
| `ncl-contrib-sb-rotate-byte` | 3 | 1 |
| `ncl-contrib-sb-rt` | 3 | 10 |
| `ncl-contrib-sb-simple-streams` | 3 | 106 |
| `ncl-contrib-sb-sprof` | 3 | 16 |
| `ncl-contrib-uiop` | 3 | 463 |
| `ncl-contrib-usocket` | 3 | 12 |
| `ncl-ffi` | 1 | 106 |
| `ncl-image` | 1 | 7 |
| `ncl-lib-format` | 1 | 1 |
| `ncl-lib-hash-arrays` | 1 | 55 |
| `ncl-lib-macros` | 1 | 97 |
| `ncl-lib-numbers` | 1 | 164 |
| `ncl-lib-packages` | 1 | 58 |
| `ncl-lib-pathnames` | 1 | 43 |
| `ncl-lib-sequences` | 1 | 150 |
| `ncl-lib-streams` | 1 | 121 |
| `ncl-lib-strings` | 1 | 108 |
| `ncl-object` | 1 | 39 |
| `ncl-printer` | 1 | 24 |
| `ncl-reader` | 1 | 23 |
| `ncl-runtime` | 2 | 59 |
| `ncl-threads` | 1 | 130 |
| `ncl-types` | 1 | 64 |

Phase totals are 1,617 / 59 / 1,884 for phases 1 / 2 / 3.

## Counts by kind

| kind | count |
| --- | ---: |
| `class` | 186 |
| `class+function` | 19 |
| `condition+class` | 123 |
| `condition+class+function` | 10 |
| `constant` | 502 |
| `constant+class` | 2 |
| `function` | 2041 |
| `macro` | 194 |
| `macro+class` | 1 |
| `macro+special-operator` | 1 |
| `other` | 217 |
| `special-operator` | 27 |
| `special-operator+class` | 3 |
| `type` | 20 |
| `variable` | 209 |
| `variable+function` | 5 |

## Judgement notes

Only boundary cases that required an explicit choice are listed here:

- Built-in stream classes such as `BROADCAST-STREAM` remain in `ncl-types`; their constructors and accessors are in `ncl-lib-streams`.
- `MAKE-LOAD-FORM` and `MAKE-LOAD-FORM-SAVING-SLOTS` are compiler-front load-form expansion hooks.
- `EQ`/`EQL` are numeric-lane direct primitives; `EQUAL`/`EQUALP` are sequence/object comparison utilities.
- `COPY-STRUCTURE` and the `STRUCTURE` marker are CLOS-owned, while list/tree copying remains in `ncl-lib-sequences`.
- `Y-OR-N-P` and `YES-OR-NO-P` are stream-facing interaction functions.
- SB-EXT package-lock and name-conflict conditions are in `ncl-conditions`; package management functions remain in `ncl-lib-packages`.
- SB-EXT atomic/hash-table synchronization forms split between `ncl-threads` and `ncl-lib-hash-arrays` by the protected resource.
- SB-SYS fd-stream/event functions are stream-owned; deadline and interrupt functions are thread-owned.
- SB-SYS image lifecycle functions are reserved for `ncl-image`; shared-object and pinned-object operations are FFI/object-owned.

The table is the source of truth for all 3,560 rows. `python3 conformance/ownership/check.py` verifies complete source coverage and valid crate/phase fields.
