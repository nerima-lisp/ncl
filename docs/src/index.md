# NCL

NCL combines a Rust-native Common Lisp runtime with a direct, CPS-first Common
Lisp core. The Rust layer provides a span-aware reader, an interpreted
evaluator, and a stack-bytecode compiler and VM. The Common Lisp layer keeps
the language model visible in small data, logic, reader, evaluator, and
standard-library files.

~~~lisp
(defun square (x)
  (* x x))

(square 5)
~~~

Start with [Getting started](getting-started.md), then use the
[API reference](reference/api.md) for the current language and CLI surface.
[Core concepts](guide/core-concepts.md) explains the workspace boundaries,
while [Common Lisp core](guide/common-lisp-core.md) explains the macro and CPS
implementation. [Compatibility](reference/compatibility.md) records the
current limits.

The documentation is built from this directory with the configuration in
<code>docs/mkdocs.yml</code>.
