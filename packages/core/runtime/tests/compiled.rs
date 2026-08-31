//! Compiled evaluator integration tests.

mod support;

use ncl_runtime::{Runtime, RuntimeError};
use support::{MustExist, MustFail, assert_evaluates_to, assert_value_cases, evaluate_with};

fn evaluate(source: &str) -> ncl_runtime::Value {
    evaluate_with(Runtime::eval_compiled_source, source)
}

#[path = "compiled/conditions.rs"]
mod conditions;
#[path = "compiled/control.rs"]
mod control;
#[path = "compiled/core.rs"]
mod core;
#[path = "compiled/format.rs"]
mod format;
#[path = "compiled/objects.rs"]
mod objects;
#[path = "compiled/primitives.rs"]
mod primitives;
#[path = "compiled/random.rs"]
mod random;
#[path = "compiled/sequences.rs"]
mod sequences;
#[path = "compiled/setf.rs"]
mod setf;
