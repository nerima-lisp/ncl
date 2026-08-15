# Compatibility

NCL has a production-oriented Rust runtime and a direct Common Lisp core. Both
are bounded Common Lisp-oriented implementations, not SBCL-compatible
implementations. The table below describes the runtime feature boundary; the
direct core's source layout and entry points are covered in the
[Common Lisp core guide](../guide/common-lisp-core.md).

| Area | Implemented surface | Current boundary |
| --- | --- | --- |
| Reader | Symbols, keywords, numbers, strings, characters, lists, dotted lists, vectors, quote prefixes, dispatch literals for characters, functions, arrays, and discarded forms, line comments, and nested block comments. | Reader depth and syntax coverage are bounded; an unquote outside quasiquote is rejected. |
| Evaluation | Conditional forms, sequencing, lexical bindings, local functions, definitions, iteration, macros, packages, declarations, generalized places, structures, classes, generic functions, methods, and bounded load-time/multiple-value forms. | Several declaration, object-system, and standard semantic details remain partial. |
| Functions | Builtins, closures, separate local function bindings, ordinary lambda lists, <code>funcall</code>, <code>apply</code>, and mapping operations. | The lambda-list and function protocol do not cover every Common Lisp edge case. |
| Multiple values | Creation, binding, calls, list conversion, sequencing, assignment, and multiple-value status results. | Interactions with unsupported standard facilities are outside the tested surface. |
| Conditions | Formatted <code>error</code>/<code>signal</code>/<code>warn</code>/<code>cerror</code> signaling with <code>SIMPLE-CONDITION</code> metadata, <code>define-condition</code>, bounded <code>make-condition</code> construction, inherited condition slots and accessors, condition type predicates, simple condition/report accessors, <code>ignore-errors</code>, <code>handler-case</code>, dynamic <code>handler-bind</code>, bounded <code>restart-bind</code>/<code>restart-case</code>, restart objects with <code>compute-restarts</code>/<code>find-restart</code>/<code>restart-name</code>, condition associations with <code>with-condition-restarts</code>, simple restarts, and cleanup with <code>unwind-protect</code>. | The full standard condition hierarchy, all constructor/reporting details, continuable handler edge cases, and the remaining restart protocol details are incomplete. |
| Packages | Package definition, use lists, exports, qualification, interning, lookup, and package predicates. | Full package objects and all symbol identity/shadowing semantics are not complete. |
| Numbers | Integers, floats, rationals, arithmetic, comparisons, rounding, integer operations, and rational accessors. | The complete numeric tower and all coercion/printing rules are not implemented. |
| Strings and streams | Character/string operations, string and file streams, bounded line and character I/O, <code>with-input-from-string</code>, <code>with-output-to-string</code>, file opening/closing, duplex <code>:io</code> streams, and formatted output. | The general stream protocol, binary streams, full file-option/version semantics, and the remaining <code>format</code> directives are incomplete. |
| Arrays and tables | Simple and multidimensional arrays plus hash tables with <code>eq</code>, <code>eql</code>, <code>equal</code>, and <code>equalp</code> tests. | Specialized arrays, full sequence semantics, and standard edge cases remain work. |
| Structures and objects | Basic <code>defstruct</code> support plus classes, instances, generic functions, methods, slot access, and bounded class introspection. | The full MOP, all slot allocation/options, method qualifiers, and standard object-protocol details remain partial. |
| Compiler | Stack bytecode compiler and VM selected by <code>--compiled</code>; <code>Runtime::compile</code>, <code>Runtime::compile_source</code>, and <code>--compile</code> expose macro-expanded bytecode artifacts. <code>NTH-VALUE</code> is lowered to a native VM instruction. | Compilation performs supported compile-time preparation, but does not execute ordinary runtime forms. The compiler still has evaluator-backed paths, covers only the tested language subset, is not optimizing, and is not SBCL-compatible. Artifact counts do not establish whole-program conformance. |

Both interpreted and compiled execution, as well as compile-only artifact
generation, are covered by the repository's local tests where the
corresponding feature is supported. Passing the test suite does not establish
conformance with SBCL or the full Common Lisp standard. Compatibility claims
should be made only for behavior backed by executable tests.
