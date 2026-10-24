# Changelog

Sections follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and release versions follow Semantic Versioning.

## [Unreleased]

### Added

- Added `EVAL` and pathname-aware `LOAD`. `COMPILE-FILE` stores source text in its FASL output, and `LOAD` recompiles that text.
- Added compiler optimization modes and pass diagnostics, with GC-safe optimization passes.
- Added `DECLAIM` and `SPECIAL`, `DEFSETF`, more `LOOP` clauses, and condition and restart facilities including `MAKE-CONDITION`, `HANDLER-CASE`, and `IGNORE-ERRORS`.
- Added readtable copying and feature conditionals, pathname literals and logical pathnames, filesystem operations, and character and string stream operations.
- Added array element types, hash-table options, array-aware `COPY-SEQ`, sequence selection and duplicate removal, and further sequence operations.
- Added image save and load, initial CLOS and MOP support, typed foreign calls and tracked system-area-pointer operations, and internal-time queries.
- Added character package designators for package introspection, symbol interning, and package operations, plus `MOD` element types for `MAKE-ARRAY`.

### Fixed

- Corrected sequence ordering, selection options, list and higher-order function contracts, `SEARCH` and `MISMATCH` bounds, and `EQUAL`/`EQUALP` behavior for vectors and fill pointers.
- Preserved dynamic state across nonlocal exits and multiple values through cleanup and capture.
- Aligned package rename, deletion, and introspection with Common Lisp semantics, and corrected string comparison bounds.
