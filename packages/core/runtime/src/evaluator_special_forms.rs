type DoBinding = (String, bool, Option<Form>, Option<Form>);

struct PushnewOptions {
    test: Option<Value>,
    test_not: Option<Value>,
    key: Option<Value>,
}

#[path = "evaluator_special_forms_bindings.rs"]
mod bindings;
#[path = "evaluator_special_forms_conditionals.rs"]
mod conditionals;
#[path = "evaluator_special_forms_control.rs"]
mod control;
#[path = "evaluator_special_forms_control_flow.rs"]
mod control_flow;
#[path = "evaluator_control_primitives.rs"]
mod control_primitives;
#[path = "evaluator_special_forms_definitions.rs"]
mod definitions;
#[path = "evaluator_special_forms_misc.rs"]
mod misc;
#[path = "evaluator_quasiquote.rs"]
mod quasiquote;
#[path = "evaluator_setf.rs"]
mod setf;
#[path = "evaluator_setf_expansion.rs"]
mod setf_expansion;
#[path = "evaluator_setf_helpers.rs"]
mod setf_helpers;
#[path = "evaluator_setf_places.rs"]
mod setf_places;
include!("evaluator_definitions.rs");
include!("evaluator_function_calls.rs");
include!("evaluator_sequences.rs");
include!("evaluator_invocation.rs");
#[path = "evaluator_macro_parsing.rs"]
mod macro_parsing;
