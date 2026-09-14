# Symbol ownership for Phase 1 lanes

This note summarizes `conformance/ownership/symbols.tsv`. The checker is the source of truth for coverage and field validity.

## Lane summary

| crate | phase 1 symbols | representative symbols | dependencies |
| --- | ---: | --- | --- |
| `ncl-clos` | 147 | ADD-METHOD, BUILT-IN-CLASS, CLASS, COPY-STRUCTURE, FIND-METHOD | ncl-object, ncl-types, ncl-conditions |
| `ncl-compiler-front` | 162 | &KEY, BLOCK, COMPILE-FILE-LINE, DECLAIM, DEFCLASS, MAKE-LOAD-FORM, TRULY-THE | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-ir |
| `ncl-conditions` | 100 | *DEBUGGER-HOOK*, BREAK, DIVISION-BY-ZERO, NAME-CONFLICT, SYSTEM-CONDITION | ncl-object |
| `ncl-ffi` | 108 | *, ADDR, ALIEN, DLOPEN-OR-LOSE, MEMMOVE, RUN-PROGRAM | ncl-object, ncl-conditions, libloading |
| `ncl-image` | 7 | *POSIX-ARGV*, EXIT, OS-COLD-INIT-OR-REINIT, QUIT, SAVE-LISP-AND-DIE | ncl-object, ncl-compiler-back |
| `ncl-lib-format` | 1 | FORMAT | ncl-object and standard-library lower layers |
| `ncl-lib-hash-arrays` | 55 | ADJUSTABLE-ARRAY-P, AREF, ARRAY, ARRAY-DIMENSION, HASH-TABLE-SYNCHRONIZED-P, WITH-LOCKED-HASH-TABLE | ncl-object |
| `ncl-lib-macros` | 98 | AND, APPLY, CONSTANTP, FUNCALL, MACROEXPAND, VALUES | ncl-object, ncl-types, ncl-reader, ncl-conditions |
| `ncl-lib-numbers` | 170 | *, +, /, <, ASH, BOOLE-ANDC1, RANDOM, REM | ncl-object |
| `ncl-lib-packages` | 60 | *GENSYM-COUNTER*, ADD-PACKAGE-LOCAL-NICKNAME, EXPORT, FIND-PACKAGE, GENTEMP | ncl-object |
| `ncl-lib-pathnames` | 43 | *COMPILE-FILE-PATHNAME*, DELETE-DIRECTORY, NATIVE-NAMESTRING, PATHNAME | ncl-object |
| `ncl-lib-sequences` | 150 | APPEND, BUTLAST, CAAAR, COPY-TREE, EQUAL, SUBLIS, TREE-EQUAL | ncl-object |
| `ncl-lib-streams` | 136 | *DEBUG-IO*, FD-STREAM, MAKE-FD-STREAM, READ-BYTE, SERVE-EVENT, WRITE | ncl-object |
| `ncl-lib-strings` | 108 | ALPHA-CHAR-P, CHAR=, CHAR-CODE, NAME-CHAR, STANDARD-CHAR-P | ncl-object |
| `ncl-object` | 39 | *AFTER-GC-HOOKS*, GENERATION-AVERAGE-AGE, HEAP-ALLOCATED-P, PURIFY, WITH-PINNED-OBJECTS | ncl-sys |
| `ncl-printer` | 24 | *PRINT-READABLY*, PPRINT, PRINT, PRINT-UNREADABLY, WRITE-TO-STRING | ncl-object |
| `ncl-reader` | 5 | READTABLE-BASE-CHAR-PREFERENCE, READTABLE-CASE, READTABLE-NORMALIZATION | ncl-object |
| `ncl-threads` | 139 | ATOMIC-INCF, PROCESS-WAIT, TIMER-NAME, WAIT-FOR, WITH-DEADLINE, WITH-INTERRUPTS | ncl-object, ncl-conditions, ncl-sys |
| `ncl-types` | 65 | BASE-STRING, BIGNUM, BROADCAST-STREAM, FLOAT, KEYWORD, WORD | ncl-object |

Phase 1 has 1,617 symbols, Phase 2 has 59, and Phase 3 has 1,884.

## Runtime boundary

The 59 Phase 2 rows are limited to evaluation and compilation, loading and module/feature state, implementation-environment and time functions, REPL/debugger operations, and the explicitly retained SB-EXT evaluator/compiler/image hooks. The table contains no other `ncl-runtime` rows.

## Direct-expansion primitives

The `direct-expansion=yes` rows are the compiler pipeline primitive surface: list access and mutation, array access, fixnum arithmetic/comparison, `eq`, `eql`, type and character predicates, and structure slot accessors. The machine-checkable set is the `direct-expansion` column.
