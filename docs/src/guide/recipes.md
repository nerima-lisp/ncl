# Recipes

## Evaluate an expression

~~~sh
nix develop --command cargo run -- --eval '(+ 1 2)'
~~~

The command prints <code>3</code>.

## Define and call a function

~~~sh
nix develop --command cargo run -- --eval \
  '(progn (defun square (x) (* x x)) (square 12))'
~~~

The command prints <code>144</code>. The same forms can be entered in the
interactive REPL. Use <code>--load file.lisp</code> to evaluate a file,
<code>--script file.lisp</code> to run it without printing its final value,
and <code>--compile-file file.lisp</code> to write compiled output.

The [API reference](../reference/api.md) documents the current command-line
surface. The [wave plan](../project/wave-plan.md) records work still in
progress.
