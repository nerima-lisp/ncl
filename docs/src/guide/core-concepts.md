# Core concepts

## Workspace layers

NCL is one Cargo workspace. Its crates are grouped by responsibility:

| Layer | Crates | Responsibility |
| --- | --- | --- |
| Unsafe platform | <code>ncl-sys</code> | OS declarations: pages, threads, dynamic loading, and code-space permissions. The only crate allowed to use unsafe code. |
| Typed object | <code>ncl-object</code>, <code>ncl-types</code> | <code>Word</code>, tags, accessors, allocation, <code>Runtime</code>, <code>ThreadContext</code>, and type specifiers. |
| Language front end | <code>ncl-reader</code>, <code>ncl-printer</code>, <code>ncl-conditions</code>, <code>ncl-clos</code>, <code>ncl-compiler-front</code> | Reading and printing, conditions, classes, macro expansion, and lowering to IR. |
| Machine backend | <code>ncl-ir</code>, <code>ncl-codegen</code>, <code>ncl-asm-x86-64</code>, <code>ncl-asm-aarch64</code>, <code>ncl-objfile</code> | IR descriptors, register allocation, instruction encoding, and native object output. |
| Library | <code>ncl-lib-*</code> | Standard library builtins: numbers, sequences, strings, hash arrays, streams, pathnames, packages, format, and macros. |
| Integration | <code>ncl-threads</code>, <code>ncl-ffi</code>, <code>ncl-image</code>, <code>ncl-runtime</code>, <code>ncl-conformance</code>, and the root <code>ncl</code> bin | Threads, foreign calls, image save and load, evaluation and compilation, the conformance runner, and the command-line interface. |

## From source to native code

The dependency edges already follow the target pipeline. Source flows from
the reader into the compiler front end, which expands macros and lowers
forms to IR. Codegen turns IR into machine code through the two assemblers,
and objfile writes the native object. <code>ncl-runtime</code> sequences
compilation, loading, and execution, and the <code>ncl</code> bin is its
command-line interface.

There is no interpreter and no stack-bytecode VM. The interpreter, VM, and
syntax crates ended at commit <code>d9bbb4ec</code> and were removed.

External crates are zero, in dependencies and dev-dependencies alike, and
unsafe code is confined to <code>ncl-sys</code>. Every other crate compiles
with <code>unsafe_code = "forbid"</code>.

## Forms and values

The design targets the usual Common Lisp surface. The reader produces
forms with source spans. Values include numbers, strings, characters,
symbols, lists, vectors, arrays, hash tables, functions, structures, and
stream values. <code>NIL</code> is the false and empty-list value, and
<code>T</code> is the true constant. Operations may return multiple
values.

This is the target design, not implemented behavior. Source-to-native
execution is not connected yet: the crates exist, but
<code>ncl --eval</code> is not implemented during the native rewrite.
Milestone M1 is the first point where source text executes as native
code.

See the [compatibility reference](../reference/compatibility.md) for how
compatibility claims are defined and the [roadmap](../project/roadmap.md)
for the plan.
