# API reference

NCL is mid-rewrite, so the current user-visible surface is small. This page
describes only what exists today. The language API and the Rust API
reference will be rebuilt here as the implementation lanes land; the
boundaries those lanes implement are the design contracts in
`docs/src/design/` and the [wave plan](../project/wave-plan.md).

## Command-line interface

The `ncl` binary accepts one argument:

| invocation | behavior | exit status |
| --- | --- | --- |
| `ncl --version` or `ncl -V` | prints the package version | 0 |
| `ncl --eval` | reports that `--eval` is not implemented during the native rewrite | 1 |
| `ncl <any other argument>` | reports an unsupported option | 2 |
| `ncl` | reports that NCL is being rewritten | 2 |

There is no REPL, file loading, or compiled-evaluation option in the
current CLI. The options of the retired implementation (`--file`, `-f`,
`--repl`, `--compiled`, `--quiet`, `-e`, and `-h`) are gone with the
retired interpreter, VM, and syntax crates.

## Language and Rust API

No language or Rust API reference exists yet. The retired evaluator and VM
surface is not documented here and will not be restored; the native
implementation's surface will be documented as its lanes land.
