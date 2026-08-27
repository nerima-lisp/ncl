//! Interpreted evaluator integration tests.

mod support;

use ncl_runtime::Runtime;
use support::{MustExist, MustFail, assert_evaluates_to, assert_value_cases, evaluate_with};

fn evaluate(source: &str) -> ncl_runtime::Value {
    evaluate_with(Runtime::eval_source, source)
}

#[path = "evaluator/conditions.rs"]
mod conditions;
#[path = "evaluator/control.rs"]
mod control;
#[path = "evaluator/core.rs"]
mod core;
#[path = "evaluator/objects.rs"]
mod objects;
#[path = "evaluator/primitives.rs"]
mod primitives;
#[path = "evaluator/sequence_validation.rs"]
mod sequence_validation;
#[path = "evaluator/sequences.rs"]
mod sequences;
#[path = "evaluator/setf.rs"]
mod setf;
