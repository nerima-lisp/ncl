# Compatibility

NCL is a bounded Common Lisp-oriented runtime, not an SBCL-compatible
implementation. The table below describes the current boundary at the feature
level.

| Area | Implemented surface | Current boundary |
| --- | --- | --- |
| Reader | Symbols, keywords, numbers, strings, characters, lists, dotted lists, vectors, quote prefixes, line comments, nested block comments, and discarded forms. | Reader depth and syntax coverage are bounded; an unquote outside quasiquote is rejected. |
| Evaluation | Conditional forms, sequencing, lexical bindings, local functions, definitions, iteration, macros, packages, declarations, places, and structures. | Several declaration and standard semantic details remain partial. |
| Functions | Builtins, closures, separate local function bindings, ordinary lambda lists, <code>funcall</code>, <code>apply</code>, and mapping operations. | The lambda-list and function protocol do not cover every Common Lisp edge case. |
| Multiple values | Creation, binding, calls, list conversion, sequencing, assignment, and multiple-value status results. | Interactions with unsupported standard facilities are outside the tested surface. |
| Conditions | <code>ignore-errors</code>, <code>handler-case</code>, non-continuable <code>handler-bind</code>, simple restarts, and cleanup with <code>unwind-protect</code>. | The full condition hierarchy, reporting, continuable handlers, and restart protocol are incomplete. |
| Packages | Package definition, use lists, exports, qualification, interning, lookup, and package predicates. | Full package objects and all symbol identity/shadowing semantics are not complete. |
| Numbers | Integers, floats, rationals, arithmetic, comparisons, rounding, integer operations, and rational accessors. | The complete numeric tower and all coercion/printing rules are not implemented. |
| Strings and streams | Character/string operations, string input and output streams, bounded line and character I/O, and formatted output. | The general stream model and the remaining <code>format</code> directives are incomplete. |
| Arrays and tables | Simple and multidimensional arrays plus hash tables with <code>eq</code>, <code>eql</code>, <code>equal</code>, and <code>equalp</code> tests. | Specialized arrays, full sequence semantics, and standard edge cases remain work. |
| Structures | Basic <code>defstruct</code> support. | Classes, generic functions, methods, and the object system are not implemented. |
| Compiler | Stack bytecode compiler and VM selected by <code>--compiled</code>. | This is not an optimizing or SBCL-compatible compiler. |

Both interpreted and compiled execution are covered by the repository's local
tests where the corresponding feature is supported. Passing the test suite
does not establish conformance with SBCL or the full Common Lisp standard.
Compatibility claims should be made only for behavior backed by executable
tests.
