//! Integration coverage for runtime roots and control/instance behavior.

use ncl_object::{ObjectError, Runtime, ThreadContext};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[path = "coverage_deep/arrays_and_errors.rs"]
mod arrays_and_errors;
