# Symbol ownership for Phase 1 lanes

This note is the lane-facing summary of `conformance/ownership/symbols.tsv`. Counts are generated from the current table; the checker is the source of truth for coverage.

## Lane summary

| crate | phase 1 symbols | representative symbols | dependencies |
| --- | ---: | --- | --- |
| `ncl-clos` | 136 | ALLOCATE-INSTANCE, BUILT-IN-CLASS, CALL-NEXT-METHOD, CLASS, CLASS-NAME, CLASS-OF, COMPUTE-APPLICABLE-METHODS, DESCRIBE | ncl-object, ncl-types, ncl-conditions |
| `ncl-compiler-front` | 42 | BLOCK, CATCH, DECLAIM, DECLARE, DEFCLASS, DEFGENERIC, DEFINE-CONDITION, DEFINE-METHOD-COMBINATION | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-ir |
| `ncl-conditions` | 65 | *DEBUGGER-HOOK*, ABORT, ARITHMETIC-ERROR, ARITHMETIC-ERROR-OPERANDS, BREAK, CELL-ERROR, CELL-ERROR-NAME, CERROR | ncl-object |
| `ncl-ffi` | 100 | *, ADDR, ALIEN, ALIEN-CALLABLE, ALIEN-CALLABLE-FUNCTION, ALIEN-FUNCALL, ALIEN-SAP, ALIEN-SIZE | ncl-object, ncl-conditions, libloading |
| `ncl-image` | 4 | *POSIX-ARGV*, EXIT, QUIT, SAVE-LISP-AND-DIE | ncl-object, ncl-compiler-back |
| `ncl-lib-format` | 1 | FORMAT | ncl-object and standard-library lower layers |
| `ncl-lib-hash-arrays` | 44 | ADJUST-ARRAY, AREF, ARRAY, ARRAY-DIMENSION, ARRAY-DIMENSIONS, ARRAY-DISPLACEMENT, ARRAY-ELEMENT-TYPE, ARRAY-IN-BOUNDS-P | ncl-object |
| `ncl-lib-macros` | 81 | AND, ASSERT, CALL-METHOD, CASE, CCASE, CHECK-TYPE, COND, CTYPECASE | ncl-object, ncl-types, ncl-reader, ncl-conditions |
| `ncl-lib-numbers` | 146 | *, *READ-DEFAULT-FLOAT-FORMAT*, +, -, /, 1+, 1-, ABS | ncl-object |
| `ncl-lib-packages` | 42 | *PACKAGE*, BOUNDP, COPY-SYMBOL, DELETE-PACKAGE, EXPORT, FBOUNDP, FIND-ALL-SYMBOLS, FIND-PACKAGE | ncl-object |
| `ncl-lib-pathnames` | 39 | *COMPILE-FILE-PATHNAME*, *DEFAULT-PATHNAME-DEFAULTS*, *LOAD-PATHNAME*, COMPILE-FILE-PATHNAME, DELETE-FILE, DIRECTORY, DIRECTORY-NAMESTRING, ENOUGH-NAMESTRING | ncl-object |
| `ncl-lib-sequences` | 90 | ACONS, ADJOIN, APPEND, ASSOC, ASSOC-IF, ASSOC-IF-NOT, ATOM, CAAAR | ncl-object |
| `ncl-lib-streams` | 93 | *DEBUG-IO*, *ERROR-OUTPUT*, *PRINT-ARRAY*, *PRINT-BASE*, *PRINT-CASE*, *PRINT-CIRCLE*, *PRINT-ESCAPE*, *PRINT-GENSYM* | ncl-object |
| `ncl-lib-strings` | 101 | ALPHA-CHAR-P, ALPHANUMERICP, BASE-CHAR, BOTH-CASE-P, CHAR, CHAR-CODE, CHAR-DOWNCASE, CHAR-EQUAL | ncl-object |
| `ncl-object` | 14 | *AFTER-GC-HOOKS*, *GC-REAL-TIME*, *GC-RUN-TIME*, CANCEL-FINALIZATION, FINALIZE, GC, GENERATION-BYTES-ALLOCATED, HASH-TABLE-WEAKNESS | ncl-sys |
| `ncl-threads` | 91 | *EXIT-TIMEOUT*, MAKE-TIMER, SCHEDULE-TIMER, TIMEOUT, TIMER, TIMER-SCHEDULED-P, UNSCHEDULE-TIMER, WITH-TIMEOUT | ncl-object, ncl-conditions, ncl-sys |
| `ncl-types` | 12 | COERCE, NULL, REAL, SEQUENCE, SIMPLE-ARRAY, SIMPLE-BIT-VECTOR, STREAM, SUBTYPEP | ncl-object |

## Direct-expansion primitives

The `direct-expansion=yes` rows are the compiler pipeline primitive surface: `car`, `cdr`, `rplaca`, `rplacd`, `svref`, `aref`/`aset`, fixnum arithmetic and comparisons, `eq`, `eql`, `typep`, character predicates, and structure slot accessors represented by the extracted symbols. The exact machine-checkable set is the `direct-expansion` column.

Phase 2 runtime symbols are intentionally not repeated as lane ownership above. Phase 3 contrib and internal compatibility rows remain reserved under `ncl-contrib-*`.
