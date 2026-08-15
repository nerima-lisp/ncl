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
- Complete file-stream option semantics, the general stream protocol, binary
  streams, and standard I/O behavior.
- Complete package objects, symbol identity, shadowing, and package protocol
  semantics.
- Complete the remaining standard condition hierarchy, constructor and
  reporting details, continuable handlers, and restart protocol behavior.
- Complete CLOS slot and method options, class precedence details, and the
  remaining object-protocol behavior.

## Compiler and execution

- Move evaluator-backed compiler paths to native bytecode incrementally; the
  current native surface includes multiple-value selection through
  <code>nth-value</code> and <code>PUSH</code>/<code>POP</code> for simple symbol
  places, while generalized-place and remaining fallback paths stay explicit
  in compatibility tests.
- Define and test a behavior matrix for interpreted execution, compiled
  execution, and compile-only artifact generation, including compile-time
  macro/package effects and runtime non-execution.
- Improve compiler diagnostics and execution behavior without weakening
  interpreter tests, then establish a stable serialized-artifact boundary
  before treating compiled output as a portable deliverable.
- Establish a larger conformance suite and investigate memory-management and
  garbage-collection requirements.

Implemented foundations such as arrays, hash tables, rationals, string and
file streams, structures, CLOS classes and methods, packages, and
condition/control constructs are documented as bounded features rather than
listed as absent work.
