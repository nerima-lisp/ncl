# Getting started

## Rust runtime requirements

The workspace requires Rust 1.97 or newer and pins Rust 1.97.1 in
`rust-toolchain.toml`. It is a Cargo workspace, so all commands below are run
from its root.

## Evaluate an expression

Run one expression with the CLI:

~~~sh
cargo run --locked -- --eval '(+ 1 2)'
~~~

The result is printed to standard output. Repeated <code>--eval</code> options
share one runtime:

~~~sh
cargo run --locked -- --eval '(define square (lambda (x) (* x x)))' --eval '(square 5)'
~~~

Use <code>--compiled</code> to send the evaluated source through the
stack-bytecode compiler and VM:

~~~sh
cargo run --locked -- --compiled --eval '(+ 1 2)'
~~~

Use <code>--compile</code> to build and inspect bytecode artifacts without
executing the forms:

~~~sh
cargo run -- --compile --eval '(defun square (x) (* x x))'
~~~

Compilation still performs source reading, package resolution, and macro
expansion in input order, so later forms can use earlier compile-time state.
Runtime definitions and other runtime effects are not executed in this mode.

## Run a file or the REPL

Evaluate a Lisp file with an explicit path:

~~~sh
cargo run --locked -- --file path/to/program.lisp
~~~

Start an interactive session with either of these forms:

~~~sh
cargo run --locked -- --repl
cargo run --locked
~~~

The second command starts the REPL because no file or expression was supplied.
<code>--quiet</code> suppresses REPL prompts and normal value output:

~~~sh
cargo run --locked -- --quiet --repl
~~~

## Command-line help

Use <code>--help</code> or <code>-h</code> to print usage information, and
<code>--version</code> or <code>-V</code> to print the package version.
Repeated <code>--eval</code>/<code>-e</code> options run in order and share one
runtime. If <code>--file</code>/<code>-f</code> is supplied as well, the file is
evaluated after those expressions; adding <code>--repl</code> enters the REPL
after the non-interactive inputs.

The process exits with status 0 on success, 1 for evaluation or file errors,
and 2 for command-line usage errors.

## Using Nix

The Nix development shell provides `rustc`, Cargo, `rustfmt`, Clippy, and the
Common Lisp and documentation tools used by the project:

~~~sh
nix develop path:.
cargo run -- --eval '(+ 1 2)'
~~~

## Common Lisp core

Enter the project development shell to get SBCL, cl-weave, paredit-cli, and
the documentation tools:

~~~sh
nix develop path:.
~~~

Run the direct Common Lisp CLI:

~~~sh
nix run path:. -- --eval '(defun square (x) (* x x))' --eval '(square 5)'
~~~

Run the executable test suite through cl-weave:

~~~sh
nix run path:.#test
~~~

Run the same suite with coverage instrumentation:

~~~sh
nix run path:.#coverage
~~~

The implementation and test systems are declared in `ncl.asd`. The direct
entry points read that declaration and load source files in its serial order;
this keeps the command-line and test paths aligned.

The coverage command writes an HTML report and coverage data below
`artifacts/ncl-coverage/` by default. Set `NCL_COVERAGE_DIR` to write those
artifacts elsewhere.

## Rust coverage

Run the Rust workspace tests with LLVM coverage and the current ratchet floors:

~~~sh
nix run path:.#rust-coverage -- --summary-only
~~~

The command requires at least 75% line, 78% function, and 75% region coverage.
The target is 100% for every metric. Pass `--html --output-dir
artifacts/rust-coverage` instead of `--summary-only` to write a browsable HTML
report. The same ratchet is enforced by the `ncl-rust-coverage` flake check;
run `nix flake check path:.` to include it in the repository-wide gates.
