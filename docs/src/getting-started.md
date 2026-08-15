# Getting started

## Rust runtime requirements

Install Rust 1.85 or newer. The repository is a Cargo workspace, so all
commands below are run from its root.

## Evaluate an expression

Run one expression with the CLI:

~~~sh
cargo run -- --eval '(+ 1 2)'
~~~

The result is printed to standard output. Repeated <code>--eval</code> options
share one runtime:

~~~sh
cargo run -- --eval '(define square (lambda (x) (* x x)))' --eval '(square 5)'
~~~

Use <code>--compiled</code> to send the evaluated source through the
stack-bytecode compiler and VM:

~~~sh
cargo run -- --compiled --eval '(+ 1 2)'
~~~

## Run a file or the REPL

Evaluate a Lisp file with an explicit path:

~~~sh
cargo run -- --file path/to/program.lisp
~~~

Start an interactive session with either of these forms:

~~~sh
cargo run -- --repl
cargo run
~~~

The second command starts the REPL because no file or expression was supplied.
<code>--quiet</code> suppresses REPL prompts and normal value output:

~~~sh
cargo run -- --quiet --repl
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

When Rust is supplied by Nix, run Cargo and the formatter from a shell that
contains the required tools:

~~~sh
nix shell nixpkgs#rustc nixpkgs#rustfmt --command cargo run -- --eval '(+ 1 2)'
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
