# API reference

This page documents the current command-line surface. The language and Rust
APIs are documented as their implementation lanes land.

## Command-line interface

| Invocation | Behavior | Exit status |
| --- | --- | --- |
| `ncl --version` or `ncl -V` | Prints the package version. | 0 |
| `ncl --eval SOURCE` | Evaluates one source string and prints its value. | 0 or 1 |
| `ncl --load FILE` | Loads a file and prints its final value. | 0 or 1 |
| `ncl --script FILE` | Loads a file without printing its final value. | 0 or 1 |
| `ncl --compile-file FILE` | Compiles a file. | 0 or 1 |
| `ncl` | Reads forms from the interactive REPL. | 0 |

Malformed or extra arguments produce a diagnostic and exit status 2. Runtime
and compilation errors produce a diagnostic and exit status 1.

The current native path is covered by CLI integration tests, including
recursive functions, closures, loading, compilation, and REPL input.

## Language and Rust API

The language surface follows ANSI Common Lisp where implemented. NCL-specific
extensions are described in the design documents and owned by their crates.
The Rust API is internal to the workspace and is not yet a stable external
interface.
