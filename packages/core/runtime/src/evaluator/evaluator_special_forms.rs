#![allow(clippy::wildcard_imports)]
use super::*;

type DoBinding = (String, bool, Option<Form>, Option<Form>);

struct PushnewOptions {
    test: Option<Value>,
    test_not: Option<Value>,
    key: Option<Value>,
}

mod bindings;
mod conditionals;
mod control;
mod control_flow;
mod control_primitives;
#[cfg(test)]
mod control_primitives_tests;
mod definitions;
mod evaluator_definitions;
mod evaluator_function_calls;
mod evaluator_invocation;
pub mod evaluator_sequences;
mod macro_parsing;
mod misc;
mod quasiquote;
mod setf;
mod setf_expansion;
mod setf_helpers;
mod setf_places;
