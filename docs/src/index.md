# NCL

NCL is a Rust-native Common Lisp implementation. It is being rebuilt toward
ANSI Common Lisp plus an NCL-specific extension API. SBCL compatibility is
deliberately dropped; SBCL remains the acceptance baseline, measured
against its 2.6.0 numbers.

The previous interpreter, VM, and syntax crates were removed. The workspace
now connects the reader-to-native pipeline, and <code>ncl --eval</code> can
evaluate source forms.

Start with [Getting started](getting-started.md), then use the
[API reference](reference/api.md) for the language and CLI surface.
[Core concepts](guide/core-concepts.md) explains the workspace layers.
[Compatibility](reference/compatibility.md) records the current limits.
The [wave plan](project/wave-plan.md) lays out the current parallel lanes and
their follow-up work.

The documentation is built from this directory with the configuration in
<code>docs/mkdocs.yml</code>.
