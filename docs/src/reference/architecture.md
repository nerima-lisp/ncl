# Architecture

## Workspace layout

The repository is a Cargo workspace:

| Path | Role |
| --- | --- |
| <code>src</code> | Root crate, public re-exports, and CLI entry point. |
| <code>packages/core/syntax</code> | Reader, forms, spans, symbols, and lambda-list parsing. |
| <code>packages/core/compiler</code> | Bytecode instruction definitions, compiler, and VM support. |
| <code>packages/core/runtime</code> | Runtime data types, environments, evaluator, builtins, packages, conditions, and control flow. |
| <code>tests</code> | Workspace-level integration coverage. |

The workspace uses Rust edition 2024, declares Rust 1.97 as its minimum
version, and pins Rust 1.97.1 through `rust-toolchain.toml`. Workspace lints
forbid unsafe Rust. The required formatter, build, test, and Clippy checks are
also run by [.github/workflows/ci.yml](https://github.com/nerima-lisp/ncl/blob/main/.github/workflows/ci.yml).

## Dependency graph

Cargo dependencies point from the layer that uses an API to the layer that
provides it:

~~~text
root (src)
├── runtime
│   ├── compiler
│   │   └── syntax
│   └── syntax
└── syntax
~~~

This is a dependency graph, not an evaluation pipeline. At runtime, the CLI
reads source through the syntax crate and selects either the interpreter or the
compiler and VM. The runtime owns the execution environment and builtins, so
the CLI does not contain language semantics.

## Data and logic boundaries

The runtime keeps data-facing types and execution logic separate where the
language model permits it. `value.rs`, `error.rs`, `environment.rs`, and
`package.rs` define the principal runtime state and boundary types; evaluator,
builtin, and VM modules own evaluation, dispatch, and control flow. Small
invocation-context structs make data passed between those operations explicit,
while preserving the existing execution semantics.

## Compiler source organization

The compiler keeps its bytecode and error data model in
<code>packages/core/compiler/src/model.rs</code>. <code>Instruction</code>,
<code>Program</code>, <code>FunctionCode</code>, compile errors, and the
destructuring metadata types are defined there and re-exported by the compiler
crate. <code>state.rs</code> owns mutable compiler state, while the
concern-specific modules <code>expressions.rs</code>, <code>conditions.rs</code>,
<code>control_flow.rs</code>, <code>conditionals.rs</code>,
<code>definitions.rs</code>, <code>iteration.rs</code>,
<code>destructuring.rs</code>, <code>bindings.rs</code>, and
<code>streams.rs</code> own syntax-to-bytecode lowering. <code>lib.rs</code>
keeps the public compiler API and shared entry points. This keeps reusable data
definitions separate from stateful compilation logic without changing the
public compiler API.

## Runtime source organization

The evaluator and builtin implementations are split by responsibility while
remaining private to their owning runtime modules:

| Directory | Responsibilities |
| --- | --- |
| <code>packages/core/runtime/src/evaluator</code> | Evaluation, compilation, special forms, definitions, macros, packages, sequences, conditions, primitives, generic functions, lambdas, and runtime helpers. |
| <code>packages/core/runtime/src/builtins</code> | Numeric, bitwise, collection, character, string, type, array, predicate, stream, and format builtins. |

Each directory has a <code>mod.rs</code> that assembles its responsibility
fragments into the same module boundary. This keeps private invariants local
without introducing a compatibility layer or changing the public runtime API.

## Evaluation boundary

The syntax crate turns source text into span-aware forms. The runtime evaluates
those forms directly and provides lexical environments, closures, multiple
values, packages, conditions, and non-local control. The compiler lowers the
same language forms to stack bytecode, and the VM executes that bytecode.

The compiled path shares the runtime's value and environment model. Before a
form reaches the compiler, the runtime resolves package names and performs the
supported macro-expansion and compile-time preparation steps. Those steps may
evaluate forms such as macro or package definitions and
<code>load-time-value</code>; ordinary runtime forms are not executed by
<code>Runtime::compile</code>. The resulting <code>Program</code> can either be
returned as a <code>CompiledForm</code> for inspection/embedding or passed to
the VM for execution.

~~~text
source
  -> syntax::read
  -> Runtime::compile / compile_source
       -> package resolution and macro expansion
       -> compiler::compile_form -> Program
  -> optional VM execution
~~~

The <code>--compile</code> CLI mode stops at the compiled artifact after
compile-time preparation; it does not execute ordinary runtime forms. The
<code>--compiled</code> execution path compiles and executes forms in order,
which preserves definitions and package operations across a source stream.
This is a second execution route for the implemented surface, not a claim of
compatibility with an external Lisp compiler.

## Public boundary

The root crate exposes the common runtime and syntax types needed by an
embedding application, including <code>CompiledForm</code> for the public
compilation boundary. Its <code>program()</code> accessor permits inspection or
embedding of the current in-memory bytecode representation, but it does not
promise a stable instruction set or serialized-artifact format. The CLI is a
consumer of that API. Lower-level compiler instructions and runtime helpers
remain workspace implementation details unless they are explicitly
re-exported.
