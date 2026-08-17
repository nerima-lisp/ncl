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
forbid unsafe Rust. The flake derives the package version from the workspace
Cargo manifest and runs the Rust coverage ratchet as `ncl-rust-coverage`; the
required formatter, build, test, and Clippy checks are also run by
[.github/workflows/ci.yml](https://github.com/nerima-lisp/ncl/blob/main/.github/workflows/ci.yml).

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

The package state module assembles its data model and operations from private
fragments under <code>packages/core/runtime/src/package</code>: queries,
symbols, package relationships, lifecycle operations, and name normalization
are kept in separate files while remaining inside the same runtime module
boundary.

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

The stream lowering module expands <code>with-open-file</code>,
<code>with-output-to-string</code>, and <code>with-input-from-string</code> into
ordinary lexical bindings, cleanup boundaries, and stream operations before
emitting bytecode. Its direct compiler tests cover binding validation, option
selection, duplicate options, and destination/index places; runtime stream
tests separately verify the resulting behavior.

## Syntax reader organization

The syntax reader keeps its public lifecycle and source-position state in
<code>packages/core/syntax/src/reader.rs</code>. Private implementation modules
under <code>reader/</code> separate core form parsing, dispatch directives,
feature conditionals, literals, sequences, atoms, and comments. The fragments
share the same <code>Reader</code> state and preserve the crate's existing
<code>read</code> and <code>read_with_features</code> entry points, so parsing
logic can be read and tested by responsibility without adding a compatibility
layer.

## Runtime source organization

The evaluator and builtin implementations are split by responsibility while
remaining private to their owning runtime modules:

| Directory | Responsibilities |
| --- | --- |
| <code>packages/core/runtime/src/evaluator</code> | Evaluation, compilation, special forms, definitions, macros, packages, sequences, conditions, primitives, generic functions, lambdas, and runtime helpers. |
| <code>packages/core/runtime/src/builtins</code> | Numeric, bitwise, collection, character, string, type, array, predicate, stream, and format builtins. |
| <code>packages/core/runtime/src/package</code> | Package state model, queries, symbol operations, package relationships, lifecycle operations, and name normalization. |

The evaluator and builtin directories use <code>mod.rs</code>, while the package
module uses <code>package.rs</code>; each assembles its responsibility fragments
into the same module boundary. This keeps private invariants local without
introducing a compatibility layer or changing the public runtime API.

The evaluator's core runtime state follows the same boundary in
<code>evaluator/runtime_core/</code>. The private fragments separate runtime
construction and form resolution, environment lookup and mutation,
condition/restart and dynamic-stack management, special and constant values,
and symbol cleanup, package candidates, and symbol-macro expansion. They are
assembled by <code>evaluator/runtime_core.rs</code> into the same
<code>Runtime</code> implementation, so the split improves navigation without
adding a compatibility layer.

Generic dispatch and general callable application use private fragments under
<code>evaluator/generic/</code>. The <code>dispatch.rs</code> fragment owns method
ordering, hooks, continuations, and generic invocation, while
<code>application.rs</code> owns application of each callable function kind.
<code>evaluator/generic.rs</code> assembles both fragments into the same
<code>Runtime</code> implementation. Structure constructor argument binding is
isolated in <code>evaluator/structure_constructors.rs</code>; the
<code>Function::StructureConstructor</code> application path stays in the
application fragment while BOA and keyword binding, defaults, and slot
assembly are owned by the constructor fragment.

Generic-function definition and method-dispatch responsibilities are also
separated under <code>evaluator/definitions/</code>. The
<code>generic_definition.rs</code> fragment validates and constructs
<code>defgeneric</code> definitions, while <code>generic.rs</code> retains method
lookup, applicability, and invocation support. The parent
<code>definitions.rs</code> assembles these fragments into the same
<code>Runtime</code> implementation, so the split changes navigation and review
boundaries without changing the evaluator API.

Sequence aggregation follows the same boundary in
<code>evaluator/sequences/aggregation/</code>. The private
<code>data.rs</code> fragment converts supported sequence values into the shared
<code>SequenceItems</code> representation, while <code>reduce.rs</code>,
<code>search.rs</code>, <code>order.rs</code>, and <code>quantifiers.rs</code>
contain the operation-specific logic. The surrounding evaluator entry points
remain unchanged.

Sequence mutation uses the same split in
<code>evaluator/sequences/mutation/</code>. The private fragments separate
removal, substitution, <code>MAP-INTO</code>, and vector storage updates;
sequence conversion is shared through <code>SequenceItems</code> so each
operation keeps its algorithmic logic separate from list, vector, and string
representation details.

The generalized-place dispatcher follows the same data/logic boundary in
<code>evaluator/setf/places.rs</code>. It owns expansion and dispatch only;
place updates are grouped into the private fragments
<code>places/accessors.rs</code>, <code>places/sequences.rs</code>,
<code>places/arrays.rs</code>, <code>places/objects.rs</code>, and
<code>places/symbols.rs</code>. Each fragment evaluates its place arguments and
updates the corresponding runtime data, while the dispatcher preserves the
single <code>Runtime::set_place</code> entry point.

Numeric format directives follow the same dispatcher-and-fragments pattern in
<code>builtins/format/numeric.rs</code>. The dispatcher assembles the private
<code>numeric/integer.rs</code>, <code>numeric/floating.rs</code>,
<code>numeric/characters.rs</code>, and <code>numeric/radix.rs</code> fragments;
each fragment owns one numeric representation or directive family while the
format module keeps the runtime-facing entry points unchanged.

The VM execution loop follows the same boundary. <code>vm/execution.rs</code>
owns the instruction-pointer loop and shared execution state, while the private
handlers in <code>vm/execution/</code> separate state and stack operations
(<code>state.rs</code>), condition and restart flow
(<code>conditions.rs</code>), dynamic control transfer
(<code>dynamic.rs</code>), and calls and returns (<code>calls.rs</code>). The
handlers return a small outcome enum, so dispatch remains explicit without a
compatibility adapter.

## Runtime test organization

The runtime keeps interpreter and compiled execution as separate integration
test roots in <code>tests/evaluator.rs</code> and <code>tests/compiled.rs</code>.
Scenarios with identical language-level expectations are defined once under
<code>tests/common/</code> and loaded by both roots. The roots provide only the
execution-specific helpers, such as reporting whether a source form fails in
the interpreter or compiled path. Scenarios that exercise a mode-specific
implementation remain in their corresponding <code>evaluator/</code> or
<code>compiled/</code> directory.

Unit tests for value-level invariants live beside the implementation under
<code>packages/core/runtime/src/value/</code>. The predicate test module covers
every <code>Value</code> type-name variant, primary multiple-value truthiness,
condition hierarchy matching, condition slots, and simple-condition formatting
metadata. These tests keep data-facing invariants close to their implementation;
the interpreter and compiled integration roots continue to verify language-level
behavior.

## Common Lisp evaluator organization

The Common Lisp evaluator keeps shared validation, macro expansion, CPS
stepping, callable invocation, and binding operations in
<code>lisp/evaluator.lisp</code>. Special-form dispatch and the public
<code>evaluate</code> entry points live in <code>lisp/evaluator-dispatch.lisp</code>.
The ASDF component order loads these implementation layers before the
lambda-list and standard-library layers that depend on them.

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
