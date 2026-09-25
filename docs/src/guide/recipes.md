# Recipes

The recipes that previously lived on this page used the retired CLI
options <code>--eval</code> and <code>--compiled</code>, passed through
Cargo's <code>--locked</code> flag. The interpreter and stack-bytecode VM
those recipes ran on were removed at commit <code>d9bbb4ec</code>, so none
of them can run today. The interpreted and compiled recipes are retired.

## Target usage (not yet available)

When milestone M1, source text to native execution, lands, the CLI will
evaluate an expression like this:

~~~sh
ncl --eval '(+ 1 2)'
~~~

This usage does not work yet. Today <code>ncl --eval</code> prints
<code>--eval is not implemented during the native rewrite</code> and exits
with status 1, and no other evaluation option exists.

Runnable recipes will return once M1 lands. The M1 definition, a native
<code>fib(25)</code> run that prints <code>75025</code>, and the lane plan
are in the [wave plan](../project/wave-plan.md). The
[API reference](../reference/api.md) documents the language and CLI
surface.
