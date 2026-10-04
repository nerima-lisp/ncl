//! Typed Common Lisp list and sequence builtins.

extern crate self as ncl_lib_sequences;

mod domain;
mod register;

pub use register::register;

#[cfg(test)]
#[path = "../tests/builtins.rs"]
mod builtins_coverage_tests;

#[cfg(test)]
#[path = "../tests/callbacks.rs"]
mod callbacks_coverage_tests;

#[cfg(test)]
#[path = "tests_coverage.rs"]
mod tests_coverage;
