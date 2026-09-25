# Symbol ownership for crate-local tables

Ownership is recorded in one seven-column table per implementation crate at
`packages/<crate>/ownership.tsv`. The tables are intentionally independent so
parallel lanes can add their ownership rows without a shared-file merge
conflict. The checker is the source of truth for coverage and field validity.

## Surface policy

The retained public surface has three fixed groups:

- `COMMON-LISP`, with exactly 978 symbols, each owned once.
- `ASDF/INTERFACE` and `UIOP/DRIVER`, which retain the ASDF and UIOP public
  surfaces.
- The ten NCL extension packages: `NCL-THREADS`, `NCL-FFI`, `NCL-MOP`,
  `NCL-GRAY`, `NCL-GC`, `NCL-IMAGE`, `NCL-UNICODE`, `NCL-OS`, `NCL-EXT`,
  and `NCL-SYS`.

NCL extension tables are open-ended. Their rows are validated for shape and
package policy, but the checker does not compare them with a precomputed
symbol list. No package beginning with `SB-` is part of the retained surface.

Each row has the following columns:

```text
package	symbol	kind	crate	phase	direct-expansion	notes
```

`kind` may contain multiple values joined by `+`. `phase` identifies lane
implementation, runtime integration, or supporting work. The
`direct-expansion` flag records the primitive set defined by the compiler
pipeline contract.

## Lane summary

| crate | phase 1 symbols | representative symbols | dependencies |
| --- | ---: | --- | --- |
| `ncl-clos` | 147 | ADD-METHOD, BUILT-IN-CLASS, CLASS, COPY-STRUCTURE, FIND-METHOD | ncl-object, ncl-types, ncl-conditions |
| `ncl-compiler-front` | 151 | &KEY, BLOCK, COMPILE-FILE-LINE, DECLAIM, DEFCLASS, MAKE-LOAD-FORM, TRULY-THE | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-ir, ncl-clos |
| `ncl-conditions` | 129 | *DEBUGGER-HOOK*, BREAK, DIVISION-BY-ZERO, NAME-CONFLICT, SYSTEM-CONDITION | ncl-object |
| `ncl-ffi` | 106 | *, ADDR, ALIEN, DLOPEN-OR-LOSE, MEMMOVE, RUN-PROGRAM | ncl-object, ncl-conditions, ncl-sys |
| `ncl-image` | 7 | *POSIX-ARGV*, EXIT, OS-COLD-INIT-OR-REINIT, QUIT, SAVE-LISP-AND-DIE | ncl-object, ncl-objfile, ncl-sys |
| `ncl-lib-format` | 1 | FORMAT | ncl-object, ncl-types, ncl-conditions, ncl-lib-streams, ncl-lib-strings, ncl-printer |
| `ncl-lib-hash-arrays` | 55 | ADJUSTABLE-ARRAY-P, AREF, ARRAY, ARRAY-DIMENSION, HASH-TABLE-SYNCHRONIZED-P, WITH-LOCKED-HASH-TABLE | ncl-object, ncl-types, ncl-conditions |
| `ncl-lib-macros` | 97 | AND, APPLY, CONSTANTP, FUNCALL, MACROEXPAND, VALUES | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-lib-sequences |
| `ncl-lib-numbers` | 164 | *, +, /, <, ASH, BOOLE-ANDC1, RANDOM, REM | ncl-object, ncl-types, ncl-conditions |
| `ncl-lib-packages` | 58 | *GENSYM-COUNTER*, ADD-PACKAGE-LOCAL-NICKNAME, EXPORT, FIND-PACKAGE, GENTEMP | ncl-object, ncl-types, ncl-conditions |
| `ncl-lib-pathnames` | 43 | *COMPILE-FILE-PATHNAME*, DELETE-DIRECTORY, NATIVE-NAMESTRING, PATHNAME | ncl-object, ncl-types, ncl-conditions, ncl-lib-strings, ncl-sys |
| `ncl-lib-sequences` | 150 | APPEND, BUTLAST, CAAAR, COPY-TREE, EQUAL, SUBLIS, TREE-EQUAL | ncl-object, ncl-types, ncl-conditions |
| `ncl-lib-streams` | 121 | *DEBUG-IO*, FD-STREAM, MAKE-FD-STREAM, READ-BYTE, SERVE-EVENT, WRITE | ncl-object, ncl-types, ncl-conditions, ncl-sys |
| `ncl-lib-strings` | 108 | ALPHA-CHAR-P, CHAR=, CHAR-CODE, NAME-CHAR, STANDARD-CHAR-P | ncl-object, ncl-types, ncl-conditions |
| `ncl-object` | 39 | *AFTER-GC-HOOKS*, GENERATION-AVERAGE-AGE, HEAP-ALLOCATED-P, PURIFY, WITH-PINNED-OBJECTS | ncl-sys |
| `ncl-printer` | 24 | *PRINT-READABLY*, PPRINT, PRINT, PRINT-UNREADABLY, WRITE-TO-STRING | ncl-object |
| `ncl-reader` | 23 | READ, READ-FROM-STRING, *READTABLE*, READTABLE, PARSE-INTEGER, READTABLE-CASE | ncl-object |
| `ncl-threads` | 130 | ATOMIC-INCF, PROCESS-WAIT, TIMER-NAME, WAIT-FOR, WITH-DEADLINE, WITH-INTERRUPTS | ncl-object, ncl-conditions, ncl-sys |
| `ncl-types` | 64 | BASE-STRING, BIGNUM, BROADCAST-STREAM, FLOAT, KEYWORD, WORD | ncl-object |

## Runtime boundary

Phase 2 rows are limited to evaluation and compilation, loading and
module/feature state, implementation-environment and time functions,
REPL/debugger operations, and runtime integration hooks. The table contains
no SBCL compatibility surface.

## Direct-expansion primitives

The `direct-expansion=yes` rows are the compiler pipeline primitive surface:
list access and mutation, array access, fixnum arithmetic/comparison, `eq`,
`eql`, type and character predicates, and structure slot accessors. The
machine-checkable set is the `direct-expansion` column.

## Verification

Run the ownership gate from the repository root:

```sh
python3 conformance/ownership/check.py
```

It reads every crate-local table, requires complete unique coverage of the
978 `COMMON-LISP` symbols, rejects `SB-` packages, and validates the allowed
packages and seven-column fields. NCL extension row counts remain a crate
contract rather than a global fixed list.
