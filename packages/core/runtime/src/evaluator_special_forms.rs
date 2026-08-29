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
mod definitions;
mod misc;
mod quasiquote;
mod setf;
mod setf_expansion;
mod setf_helpers;
mod setf_places;
include!("evaluator_definitions.rs");
include!("evaluator_function_calls.rs");
include!("evaluator_sequences.rs");
include!("evaluator_invocation.rs");
mod macro_parsing;
