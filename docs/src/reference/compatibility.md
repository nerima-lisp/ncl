# Compatibility

NCL is a bounded Common Lisp-oriented runtime, not an SBCL-compatible
implementation. The table below describes the current boundary at the feature
level.

| Area | Implemented surface | Current boundary |
| --- | --- | --- |
| Reader | Symbols, keywords, numbers, strings, characters, lists, dotted lists, vectors, quote prefixes, line comments, nested block comments, and discarded forms. | Reader depth and syntax coverage are bounded; an unquote outside quasiquote is rejected. |
| Evaluation | Conditional forms, sequencing, lexical bindings, local functions, definitions, iteration, macros, packages, declarations, places, structures, classes, generic functions, methods, and bounded load-time/multiple-value forms. | Several declaration, object-system, and standard semantic details remain partial. |
| Functions | Builtins, closures, separate local function bindings, ordinary lambda lists, <code>funcall</code>, <code>apply</code>, and mapping operations. | The lambda-list and function protocol do not cover every Common Lisp edge case. |
| Multiple values | Creation, binding, calls, list conversion, sequencing, assignment, and multiple-value status results. | Interactions with unsupported standard facilities are outside the tested surface. |
| Conditions | Formatted <code>error</code>/<code>signal</code>/<code>warn</code>/<code>cerror</code> signaling with <code>SIMPLE-CONDITION</code> metadata, basic <code>make-condition</code> construction, condition type predicates, <code>simple-condition-format-control</code>/<code>simple-condition-format-arguments</code>, <code>ignore-errors</code>, <code>handler-case</code>, dynamic <code>handler-bind</code>, bounded <code>restart-bind</code>/<code>restart-case</code>, restart objects with <code>compute-restarts</code>/<code>find-restart</code>/<code>restart-name</code>, condition associations with <code>with-condition-restarts</code>, simple restarts, and cleanup with <code>unwind-protect</code>. | The full condition hierarchy, reporting, standard condition constructors/accessors, and remaining handler/restart protocol details are incomplete. |
| Packages | Package definition, use lists, exports, qualification, interning, lookup, and package predicates. | Full package objects and all symbol identity/shadowing semantics are not complete. |
| Numbers | Integers, floats, rationals, arithmetic, comparisons, rounding, integer operations, and rational accessors. | The complete numeric tower and all coercion/printing rules are not implemented. |
| Strings and streams | Character/string operations, string and file streams, bounded line and character I/O, file opening/closing, duplex <code>:io</code> streams, and formatted output. | The general stream protocol, binary streams, full file-option/version semantics, and the remaining <code>format</code> directives are incomplete. |
| Arrays and tables | Simple and multidimensional arrays plus hash tables with <code>eq</code>, <code>eql</code>, <code>equal</code>, and <code>equalp</code> tests. | Specialized arrays, full sequence semantics, and standard edge cases remain work. |
| Structures and objects | Basic <code>defstruct</code> support plus classes, instances, generic functions, methods, slot access, and bounded class introspection. | The full MOP, all slot allocation/options, method qualifiers, and standard object-protocol details remain partial. |
| Compiler | Stack bytecode compiler and VM selected by <code>--compiled</code>. | This is not an optimizing or SBCL-compatible compiler. |

Both interpreted and compiled execution are covered by the repository's local
tests where the corresponding feature is supported. Passing the test suite
does not establish conformance with SBCL or the full Common Lisp standard.
Compatibility claims should be made only for behavior backed by executable
tests.
