use ncl_compiler::{FunctionCode, Instruction};

use super::{argument_layout, bind_optional, bind_required};

mod argument_layout_tests;
mod binding_tests;
mod run_tests;

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
