# Common Lisp core

The Common Lisp core is a direct implementation layer for NCL. It is designed
to make the language machinery easy to inspect and extend while retaining a
small, explicit surface. It does not claim full conformance with an external
Common Lisp implementation.

## Source boundaries

The implementation follows the data/logic split:

| File | Responsibility |
| --- | --- |
| `lisp/package.lisp` | Package boundary and public API |
| `lisp/constants.lisp` | Runtime configuration constants |
| `lisp/data.lisp` | Environments, bindings, and closures |
| `lisp/logic.lisp` | CPS combinators |
| `lisp/cps-macros.lisp` | CPS sequencing macros |
| `lisp/conditions-base.lisp` | Base condition type |
| `lisp/reader.lisp` | Reading a source string into forms |
| `lisp/conditions.lisp` | Public NCL condition types and reports |
| `lisp/evaluator.lisp` | Macro expansion, special forms, and CPS evaluation |
| `lisp/lambda-list.lisp` | Required, optional, rest, keyword, and auxiliary bindings |
| `lisp/standard.lisp` | Direct standard functions and macros |
| `lisp/cli.lisp` | CLI arguments, files, and the REPL |

The evaluator expands macros to a fixed point before dispatching special forms
or function calls. User functions are closures over an environment, and
function invocation passes through the same CPS continuation boundary as
special-form evaluation.

The ASDF system is serial. `lisp/package.lisp` establishes the package first;
the remaining components are compiled with `*package*` bound to `NCL`. This
keeps package declarations at the build boundary and lets implementation files
focus on their data or execution responsibilities.

## Direct execution

`ncl.asd` is the source-of-truth system declaration. The command-line entry
point loads the declared `lisp/` source sequence directly:

~~~sh
sbcl --script run.lisp --eval '(+ 2 3)'
~~~

The flake packages the same path:

~~~sh
nix run path:. -- --eval '(+ 2 3)'
~~~

## Tests and coverage

Tests are written with cl-weave. The standalone `run-tests.lisp` entry point
loads the declared source sequence and then the test definitions. Run it
directly with SBCL (or another supported Common Lisp implementation):

~~~sh
sbcl --script run-tests.lisp
~~~

The coverage runner fails when no tests are discovered and writes a raw
coverage artifact plus a report directory under `artifacts/ncl-coverage/`.
Set `NCL_COVERAGE_DIR` when the output should live elsewhere.
The test runner's options are passed directly to the script and cl-weave.

Coverage is measured for executable implementation paths. The package,
constant, CPS-macro, and base-condition files are load-time declarations and
are excluded from the instrumented source set; macro expansions and condition
behavior remain covered through the tests that use them. The report shows
expression coverage for every instrumented file and branch coverage where a
file has branch points.

## Editing workflow

Use paredit-cli for structural checks and formatting inside the development
shell:

~~~sh
paredit inspect check --file lisp/evaluator.lisp
paredit inspect check --file test/core.lisp
~~~

The Rust runtime remains the production embedding path for the workspace; the
Common Lisp core is the direct language layer and shares the repository's
explicit-test and documentation requirements.
