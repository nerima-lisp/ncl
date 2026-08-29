use super::*;

pub(super) struct BranchInstructionContext<'a> {
    pub(super) runtime: &'a Runtime,
    pub(super) program: &'a Rc<Program>,
    pub(super) function: &'a FunctionCode,
    pub(super) stack: &'a mut Vec<Value>,
    pub(super) environment: &'a Environment,
    pub(super) program_counter: &'a mut usize,
    pub(super) span: Span,
}

pub(super) fn execute_binding_and_branch_instruction(
    instruction: &Instruction,
    context: &mut BranchInstructionContext<'_>,
) -> Result<bool, RuntimeError> {
    let runtime = context.runtime;
    let program = context.program;
    let function = context.function;
    let stack = &mut *context.stack;
    let environment = context.environment;
    let program_counter = &mut *context.program_counter;
    let span = context.span;
    match instruction {
        Instruction::BindValues(names) => {
            let value = pop_value(stack, span, "multiple-value-bind")?;
            let values = value.multiple_values();
            for (index, name) in names.iter().enumerate() {
                runtime.define_in(
                    name,
                    values.get(index).cloned().unwrap_or(Value::Nil),
                    environment,
                );
            }
            *program_counter += 1;
        }
        Instruction::BindValuesExact(names) => {
            let value = pop_value(stack, span, "multiple-value-bind")?;
            let values = value.multiple_values();
            for (index, (name, escaped)) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                if *escaped {
                    runtime.define_exact_in(name, value, environment);
                } else {
                    runtime.define_in(name, value, environment);
                }
            }
            *program_counter += 1;
        }
        Instruction::Destructure(specification) => {
            let value = pop_value(stack, span, "destructuring-bind")?;
            destructure_specification(
                specification,
                value.primary_value(),
                runtime,
                program,
                environment,
                span,
            )?;
            *program_counter += 1;
        }
        Instruction::JumpIfFalse(target) => {
            let condition = pop_value(stack, span, "conditional jump")?;
            *program_counter = if condition.is_truthy() {
                *program_counter + 1
            } else {
                jump_target(function, *target, span)?
            };
        }
        Instruction::Jump(target) => {
            *program_counter = jump_target(function, *target, span)?;
        }
        Instruction::MakeClosure(function_id) => {
            if *function_id >= program.functions.len() {
                return Err(invalid("compiled closure id is out of range", span));
            }
            stack.push(Value::compiled(
                program.clone(),
                *function_id,
                environment.clone(),
            ));
            *program_counter += 1;
        }
        Instruction::IgnoreErrors(function_id) => {
            let function = program.functions.get(*function_id).ok_or_else(|| {
                invalid("compiled ignore-errors function id is out of range", span)
            })?;
            match run_code(runtime, program, function, environment.clone(), span) {
                Ok(value) => stack.push(value),
                Err(
                    error @ (RuntimeError::ReturnFrom { .. }
                    | RuntimeError::Go { .. }
                    | RuntimeError::InvokeRestart { .. }),
                ) => return Err(error),
                Err(error) => stack.push(Value::values(vec![Value::Nil, Value::condition(&error)])),
            }
            *program_counter += 1;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
