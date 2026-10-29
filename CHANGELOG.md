# Changelog

Sections follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions follow Semantic Versioning.

## [0.1.1] - 2026-10-29

### Fixed

- Fixed a macOS arm64 crash during `LOAD` when garbage collection ran while a closure was live in a Lisp parent frame. The program no longer terminates with `SIGSEGV`.
- Fixed garbage collection of specialized array elements by tracing their values.

### Added

- Added specialized and general array literal support, lowered through constants.

### Changed

- Split Nix checks into a separate CI job so documentation checks stay within their timeout.

## [0.1.0] - 2026-10-29

### Added

- Added `EVAL` and pathname-aware `LOAD`. `COMPILE-FILE` stores source text in its FASL output, and `LOAD` recompiles that text.
- Added compiler optimization modes and pass diagnostics, with GC-safe optimization passes.
- Added `DECLAIM` and `SPECIAL`, `DEFSETF`, more `LOOP` clauses, and condition and restart facilities including `MAKE-CONDITION`, `HANDLER-CASE`, and `IGNORE-ERRORS`.
- Added readtable copying and feature conditionals, pathname literals and logical pathnames, filesystem operations, and character and string stream operations.
- Added array element types, hash-table options, array-aware `COPY-SEQ`, sequence selection and duplicate removal, and further sequence operations.
- Added image save and load, initial CLOS and MOP support, typed foreign calls and tracked system-area-pointer operations, and internal-time queries.
- Added character package designators for package introspection, symbol interning, and package operations, plus `MOD` element types for `MAKE-ARRAY`.
- Added reader evaluation of `#.` forms, standard short-form CLOS method combinations, `SUBST` and the tree substitution sequence family, and CLOS class designators for array element types.
- Added `DEFTYPE` expansion, typed `LOOP` variables, implicit generic function creation in `DEFMETHOD`, package-symbol iteration, SB package registration, `WITH-STANDARD-IO-SYNTAX`, and `COPY-PPRINT-DISPATCH`.
- Added scalar type and effect analysis and optimization passes for checked fixnum operations, direct calls, and global values.
- Added heap-rooted word sequences, the `NCL-GC` full-collection builtin, per-thread GC state, and a typed evaluator-access service.
- Added tables of the SBCL APIs and symbols available on Linux, selection of matching NCL/SBCL benchmark cases, and sharded ANSI conformance runs.
- Added standardized SETF places.

### Fixed

- Corrected sequence ordering, selection options, list and higher-order function contracts, `SEARCH` and `MISMATCH` bounds, and `EQUAL`/`EQUALP` behavior for vectors and fill pointers.
- Preserved dynamic state across nonlocal exits and multiple values through cleanup and capture.
- Aligned package rename, deletion, and introspection with Common Lisp semantics, and corrected string comparison bounds.
- Corrected reader handling of `NIL`, exponent-like symbols, extended numeric tokens, and empty tokens. `LOAD` now respects the active package and readtable and preserves reader, verifier, and native errors.
- Corrected handler and restart cleanup on nonlocal exits, closure cell capture, macro expansion roots, and evaluator restoration after nested operations.
- Corrected type-range checks for bignums, `SUBTYPEP` certainty, indexed simple-vector primitives, builtin argument marshalling, C character signedness, and system-area-pointer places.
- Corrected x86 and AArch64 instruction decoding and address-provenance tracking across safepoints and raw-memory escapes.

### Changed

- Moved the generic dispatcher wrapper into a dedicated runtime module.
- Isolated cl-bench rows in a worker pool of up to four, with 20-second child timeouts and a 1,800-second workflow budget with a 240-second reserve. The driver records per-row wall time and retains completed rows when a child times out.

[0.1.1]: https://github.com/nerima-lisp/ncl/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/nerima-lisp/ncl/compare/f9574a4d49490daca08b0ffd3375938cef847166...v0.1.0
