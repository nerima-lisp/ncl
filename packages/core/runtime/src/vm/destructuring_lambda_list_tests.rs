use std::rc::Rc;

use ncl_compiler::{Constant, DestructureLambdaList, FunctionCode, Instruction, Program};

use super::destructuring::destructure_specification;

mod arity_and_shape_tests;
mod auxiliary_and_rest_tests;
mod optional_and_keyword_tests;

fn empty_lambda_list() -> DestructureLambdaList {
    DestructureLambdaList {
        whole: None,
        required: Vec::new(),
        optional: Vec::new(),
        keywords: Vec::new(),
        has_keyword_section: false,
        allow_other_keys: false,
        rest: None,
        auxiliary: Vec::new(),
    }
}

fn constant_program(value: i64) -> Rc<Program> {
    Rc::new(Program {
        functions: vec![FunctionCode {
            name: None,
            parameters: Vec::new(),
            required_escaped: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            rest_escaped: false,
            auxiliary: Vec::new(),
            instructions: vec![
                Instruction::Constant(Constant::Integer(value)),
                Instruction::Return,
            ],
        }],
        entry: 0,
    })
}

fn empty_program() -> Rc<Program> {
    Rc::new(Program {
        functions: Vec::new(),
        entry: 0,
    })
}
