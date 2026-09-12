# Core concepts

## Workspace layers

NCL is split into four working parts:

| Layer | Responsibility |
| --- | --- |
| <code>packages/core/syntax</code> | Span-aware reading, forms, symbols, and lambda-list syntax. |
| <code>packages/core/compiler</code> | Stack-bytecode instructions, compilation, and VM execution support. |
| <code>packages/core/runtime</code> | Values, environments, evaluator, builtins, packages, conditions, and control flow. |
| <code>src</code> | The public crate re-exports and command-line interface. |

The Cargo dependency edges point from consumers to providers:
<code>src</code> depends on runtime and syntax, runtime depends on compiler and
syntax, and compiler depends on syntax. At execution time, source is read into
syntax forms and then evaluated either by the interpreter or by the compiler
and VM.

## Forms and values

The reader produces forms with source spans. The runtime evaluates those forms
in an environment containing lexical variable bindings, a separate local
function namespace, and global definitions. Values include numbers, strings,
characters, symbols, lists, vectors, arrays, hash tables, functions,
structures, and stream values.

NCL uses <code>NIL</code> as the false and empty-list value and provides
<code>T</code> as the true constant. Functions may be builtins or closures.
The language also supports multiple values, so operations such as
<code>values</code>, <code>gethash</code>, <code>intern</code>, and
<code>find-symbol</code> can return more than one result.

<code>nth-value</code> selects one value from a multiple-value result. In the
compiled route, the compiler lowers it to a native VM instruction; the
remaining compiled-language boundary is described in the
[compatibility reference](../reference/compatibility.md).

## Interpreted and compiled evaluation

The normal evaluator executes forms directly in the caller's runtime. The
<code>--compiled</code> CLI option selects the stack-bytecode compiler and VM.
Macro expansion is available in both routes. The Rust API exposes the same
boundary through <code>eval_compiled</code> and
<code>eval_compiled_source</code> for embedding applications. The compiler is
an execution backend for the current language surface; it is not an
implementation of an optimizing compiler for another runtime.

## Packages and macros

The runtime includes bounded package operations:
<code>defpackage</code>, <code>in-package</code>, <code>use-package</code>,
<code>export</code>, <code>intern</code>, and <code>find-symbol</code>. The
<code>CL</code> and <code>COMMON-LISP</code> package names are recognized as
aliases. Package and symbol identity semantics are still narrower than the
Common Lisp standard.

User macros are defined with <code>defmacro</code> and inspected with
<code>macroexpand-1</code> and <code>macroexpand</code>. The evaluator expands
macros before evaluating the resulting form, including forms processed by the
compiled route.

## Supported language surface

The project documents behavior that is implemented and tested locally. It does
not provide a backward-compatibility layer or claim whole-program
compatibility with another implementation. See the [compatibility reference](../reference/compatibility.md)
for the current feature boundary and the [roadmap](../project/roadmap.md) for
planned work.
