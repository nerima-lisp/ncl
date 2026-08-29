use std::rc::Rc;

use ncl_compiler::{FunctionCode, Program};
use ncl_syntax::Span;

use super::return_instruction::execute_return_instruction;
use super::value_instruction::execute_value_instruction;
use crate::vm::execution::control_dispatch::{
    ControlInstructionContext, execute_control_instruction,
};
use crate::vm::execution::pre_control::{
    PreControlInstructionContext, execute_pre_control_instruction,
};
use crate::vm::primitives::invalid;
use crate::{Environment, Runtime, RuntimeError, Value};

pub(in crate::vm::execution) fn run_code_from(
    runtime: &Runtime,
    program: &Rc<Program>,
    function: &FunctionCode,
    mut environment: Environment,
    span: Span,
    start_program_counter: usize,
) -> Result<Value, RuntimeError> {
    let mut stack = Vec::with_capacity(function.instructions.len());
    let mut scopes: Vec<(Environment, usize, usize)> = Vec::new();
    let _dynamic_guard = runtime.dynamic_guard();
    let mut program_counter = start_program_counter;

    loop {
        let Some(instruction) = function.instructions.get(program_counter) else {
            return Err(invalid(
                "compiled function reached an invalid instruction pointer",
                span,
            ));
        };

        let mut pre_control_context = PreControlInstructionContext {
            runtime,
            program,
            function,
            stack: &mut stack,
            scopes: &mut scopes,
            environment: &mut environment,
            program_counter: &mut program_counter,
            span,
        };
        if execute_pre_control_instruction(instruction, &mut pre_control_context)? {
            continue;
        }

        let mut control_context = ControlInstructionContext {
            runtime,
            program,
            stack: &mut stack,
            environment: &environment,
            program_counter: &mut program_counter,
            span,
        };
        if execute_control_instruction(instruction, &mut control_context)? {
            continue;
        }

        if execute_value_instruction(
            runtime,
            instruction,
            &mut stack,
            &environment,
            &mut program_counter,
            span,
        )? {
            continue;
        }

        if let Some(value) =
            execute_return_instruction(instruction, &mut stack, &environment, &scopes, span)?
        {
            return Ok(value);
        }

        unreachable!("load instruction was not handled before dispatch");
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ncl_compiler::{FunctionCode, Instruction, Program};
    use ncl_syntax::Span;

    use super::run_code_from;
    use crate::{Environment, Runtime, RuntimeError};

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
    fn rejects_invalid_instruction_pointer() {
        let runtime = Runtime::new();
        let program = Rc::new(Program {
            functions: vec![function(Vec::new())],
            entry: 0,
        });
        let error = match run_code_from(
            &runtime,
            &program,
            &program.functions[0],
            Environment::new(),
            Span::new(0, 1),
            0,
        ) {
            Err(error) => error,
            Ok(value) => panic!("unexpected successful result: {value:?}"),
        };
        assert!(
            matches!(error, RuntimeError::InvalidForm { message, .. } if message == "compiled function reached an invalid instruction pointer")
        );
    }
}
