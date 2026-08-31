#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn execute_parallel_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Psetq(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetq has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            for (name, value) in names.iter().zip(values) {
                let value = value.primary_value();
                runtime.set_or_define_in(name, value, environment, span)?;
            }
            stack.push(Value::Nil);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PsetqExact(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetq has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            for ((name, escaped), value) in names.iter().zip(values) {
                let value = value.primary_value();
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?;
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
            }
            stack.push(Value::Nil);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::MultipleValueSetq(names) => {
            let source = pop_value(stack, span, "multiple-value-setq")?;
            let values = source.multiple_values();
            for (index, name) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                runtime.set_or_define_in(name, value, environment, span)?;
            }
            stack.push(source.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        Instruction::MultipleValueSetqExact(names) => {
            let source = pop_value(stack, span, "multiple-value-setq")?;
            let values = source.multiple_values();
            for (index, (name, escaped)) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?;
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
            }
            stack.push(source.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}
