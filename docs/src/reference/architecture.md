# Architecture

## Workspace layout

The repository is a Cargo workspace:

| Path | Role |
| --- | --- |
| <code>src</code> | Root crate, public re-exports, and CLI entry point. |
| <code>packages/core/syntax</code> | Reader, forms, spans, symbols, and lambda-list parsing. |
| <code>packages/core/compiler</code> | Bytecode instruction definitions, compiler passes, destructuring, control forms, and VM support. |
| <code>packages/core/runtime</code> | Values, environments, evaluator passes, builtins, packages, conditions, and control flow. |
| <code>tests</code> | Workspace-level integration coverage. |

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

## Evaluation boundary

The syntax crate turns source text into span-aware forms. The runtime evaluates
those forms directly and provides lexical environments, closures, multiple
values, packages, conditions, and non-local control. The compiler lowers the
same language forms to stack bytecode, and the VM executes that bytecode.

The compiled path shares the runtime's value and environment model. It is
intended to provide a second execution route for the implemented surface, not
to claim compatibility with an external Lisp compiler.

## Internal separation

The runtime keeps data representation separate from evaluation logic. Value
types, display, environments, packages, and errors define the data boundary;
the evaluator modules implement form dispatch, special forms, invocation,
definitions, sequences, primitives, conditions, and package operations. Builtin
families are registered in one table but implemented in focused modules for
arrays, characters, numbers, formatting, types, and sequence operations.

The compiler follows the same boundary: instruction and compilation state are
kept separate from the passes that handle control forms, destructuring, and
ordinary forms. This makes the interpreted and compiled paths share language
values without introducing compatibility adapters.

Integration tests use a small shared evaluation helper and exercise both
execution paths where behavior is expected to match. Coverage is measured with
LLVM instrumentation in the Nix development shell; the current target is
explicitly tracked as an unmet release gate until uncovered behavior has a
corresponding test or is removed as unreachable code.

## Public boundary

The root crate exposes the common runtime and syntax types needed by an
embedding application. The CLI is a consumer of that API. Lower-level compiler
instructions and runtime helpers remain workspace implementation details unless
they are explicitly re-exported.
