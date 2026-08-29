#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_load_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let value =
        match instruction {
            Instruction::Constant(constant) => constant_value(constant, span)?,
            Instruction::Quote(form) => Runtime::quoted_value(form)?,
            Instruction::QuasiQuote(form) => runtime.quasiquote_value(form, environment)?,
            Instruction::Load(name) => runtime.lookup_in(name, environment).ok_or_else(|| {
                RuntimeError::UnboundVariable {
                    name: name.clone(),
                    span: Some(span),
                }
            })?,
            Instruction::LoadExact(name) => {
                runtime.lookup_exact_in(name, environment).ok_or_else(|| {
                    RuntimeError::UnboundVariable {
                        name: name.clone(),
                        span: Some(span),
                    }
                })?
            }
            Instruction::FunctionLoad(name) => runtime
                .lookup_function_in(name, environment)
                .ok_or_else(|| RuntimeError::UnboundVariable {
                    name: name.clone(),
                    span: Some(span),
                })?,
            Instruction::FunctionLoadExact(name) => runtime
                .lookup_function_exact_in(name, environment)
                .ok_or_else(|| RuntimeError::UnboundVariable {
                    name: name.clone(),
                    span: Some(span),
                })?,
            Instruction::IsBound(name) => Value::boolean(runtime.is_bound_in(name, environment)),
            Instruction::IsBoundExact(name) => {
                Value::boolean(runtime.lookup_exact_in(name, environment).is_some())
            }
            _ => return Ok(false),
        };
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_definition_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Define(name) | Instruction::DefineExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("define has no value on the stack", span))?
                .primary_value();
            if matches!(instruction, Instruction::Define(_)) {
                runtime.define_in(name, value.clone(), environment);
            } else {
                runtime.define_exact_in(name, value.clone(), environment);
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("define has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::DefineFunction(name) | Instruction::DefineFunctionExact(name) => {
            let value = pop_value(stack, span, "local function definition")?;
            if matches!(instruction, Instruction::DefineFunction(_)) {
                environment.define_function(name, value);
            } else {
                environment.define_function_exact(name, value);
            }
            *program_counter += 1;
            Ok(true)
        }
        Instruction::DefineSpecial { name, force }
        | Instruction::DefineSpecialExact { name, force } => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("define-special has no value on the stack", span))?;
            let exact = matches!(instruction, Instruction::DefineSpecialExact { .. });
            if *force
                && if exact {
                    runtime.is_constant_exact_in(name)
                } else {
                    runtime.is_constant_in(name)
                }
            {
                return Err(Runtime::constant_modification_error(name, span));
            }
            let value = if exact {
                runtime.define_special_value_exact(name, value.primary_value(), *force)
            } else {
                runtime.define_special_value(name, value.primary_value(), *force)
            };
            *stack
                .last_mut()
                .ok_or_else(|| invalid("define-special has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::DefineValues(name) | Instruction::DefineValuesExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("define-values has no value on the stack", span))?;
            if matches!(instruction, Instruction::DefineValues(_)) {
                runtime.define_in(name, value, environment);
            } else {
                runtime.define_exact_in(name, value, environment);
            }
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}
