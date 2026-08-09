# Architecture

## Workspace layout

The repository is a Cargo workspace:

| Path | Role |
| --- | --- |
| <code>src</code> | Root crate, public re-exports, and CLI entry point. |
| <code>packages/core/syntax</code> | Reader, forms, spans, symbols, and lambda-list parsing. |
| <code>packages/core/compiler</code> | Bytecode instruction definitions, compiler, and VM support. |
| <code>packages/core/runtime</code> | Values, environments, evaluator, builtins, packages, conditions, and control flow. |
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

## Public boundary

The root crate exposes the common runtime and syntax types needed by an
embedding application. The CLI is a consumer of that API. Lower-level compiler
instructions and runtime helpers remain workspace implementation details unless
they are explicitly re-exported.
