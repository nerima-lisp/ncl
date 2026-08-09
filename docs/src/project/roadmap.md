# Roadmap

NCL uses executable behavior and tests to grow its supported Common Lisp
surface. The project does not attach dates to the roadmap; priorities may
change as implementation and conformance work reveal the next boundary.

## Language and reader

- Extend reader syntax, escaped-symbol behavior, nesting limits, and
  macroexpansion edge cases.
- Complete declaration processing and the remaining ordinary lambda-list
  semantics.
- Expand conformance coverage for special forms, places, multiple values, and
  non-local control.

## Runtime

- Grow the numeric tower, coercion rules, printing, and the remaining
  <code>format</code> directives.
- Generalize streams beyond the current string-stream operations and complete
  standard I/O behavior.
- Complete package objects, symbol identity, shadowing, and package protocol
  semantics.
- Expand condition types, reporting, continuable handlers, and restart
  behavior.
- Add the remaining object-system pieces: classes, generic functions, and
  method dispatch.

## Compiler and execution

- Broaden compiler and VM coverage until it matches the interpreted surface.
- Improve compiler diagnostics and execution behavior without weakening
  interpreter tests.
- Establish a larger conformance suite and investigate memory-management and
  garbage-collection requirements.

Implemented foundations such as arrays, hash tables, rationals, string
streams, basic structures, packages, and condition/control constructs are
documented as bounded features rather than listed as absent work.
