# NCL

NCL is a Rust-native Common Lisp runtime. It combines a span-aware reader,
an interpreted evaluator, and a stack-bytecode compiler and VM. The supported
language surface is intentionally bounded while the runtime and conformance
tests continue to grow.

~~~lisp
(defun square (x)
  (* x x))

(square 5)
~~~

Start with [Getting started](getting-started.md), then use the
[API reference](reference/api.md) for the current language and CLI surface.
[Core concepts](guide/core-concepts.md) explains the workspace boundaries,
while [Compatibility](reference/compatibility.md) records the current limits.

The documentation is built from this directory with the configuration in
<code>docs/mkdocs.yml</code>.
