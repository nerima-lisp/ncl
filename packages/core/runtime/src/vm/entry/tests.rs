use std::rc::Rc;

use ncl_compiler::{Constant, FunctionCode, Instruction, Program};
use ncl_syntax::Span;

use crate::{Environment, Runtime, RuntimeError};

use super::run_entry;

fn function(instructions: Vec<Instruction>) -> FunctionCode {
    FunctionCode {
        name: Some("test-function".to_string()),
        parameters: Vec::new(),
        required_escaped: Vec::new(),
        optional: Vec::new(),
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        rest: None,
        rest_escaped: false,
        auxiliary: Vec::new(),
        instructions,
    }
}

#[test]
fn rejects_an_entry_function_id_out_of_range() {
    let runtime = Runtime::new();
    let program = Rc::new(Program {
        functions: Vec::new(),
        entry: 0,
    });
    let Err(error) = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1)) else {
        panic!("an invalid function id must be rejected");
    };

    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "compiled function id is out of range"
    ));
}

#[test]
fn rejects_invalid_jump_targets_before_indexing_the_instruction_stream() {
    let runtime = Runtime::new();
    let program = Rc::new(Program {
        functions: vec![function(vec![Instruction::Jump(1)])],
        entry: 0,
    });
    let Err(error) = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1)) else {
        panic!("an invalid jump target must be rejected");
    };

    assert!(matches!(
        error,
        RuntimeError::InvalidForm { message, .. }
            if message == "compiled jump target is out of range"
    ));
}

#[test]
fn rejects_invalid_compiled_rational_constants() {
    let runtime = Runtime::new();
    let program = Rc::new(Program {
        functions: vec![function(vec![Instruction::Constant(Constant::Rational {
            numerator: 1,
            denominator: 0,
        })])],
        entry: 0,
    });

    let result = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1));

    assert!(matches!(
        result,
        Err(RuntimeError::InvalidForm { message, .. })
            if message == "compiled rational constant is invalid"
    ));
}

#[test]
fn rejects_entry_functions_with_parameters_or_dynamic_bindings() {
    let runtime = Runtime::new();
    let mut compiled = function(vec![Instruction::Return]);
    compiled.parameters.push("value".to_string());
    let program = Rc::new(Program {
        functions: vec![compiled],
        entry: 0,
    });

    let result = run_entry(&runtime, &program, 0, &Environment::new(), Span::new(0, 1));

    assert!(matches!(
        result,
        Err(RuntimeError::Arity { expected, actual, .. }) if expected == "0" && actual == 0
    ));
}
