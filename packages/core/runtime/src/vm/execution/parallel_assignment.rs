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
        Instruction::PsetfSymbols(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetf has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            let mut last = Value::Nil;
            for ((name, escaped), value) in names.iter().zip(values) {
                last = value.primary_value();
                if *escaped {
                    runtime.set_or_define_exact_in(name, last.clone(), environment, span)?;
                } else {
                    runtime.set_or_define_in(name, last.clone(), environment, span)?;
                }
            }
            stack.push(last);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PsetfList(places) => super::assignment::list::execute_parallel(
            runtime, places, stack, environment, program_counter, span,
        ),
        Instruction::PsetfPlaces(places) => {
            if stack.len() < places.len() {
                return Err(invalid("psetf has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - places.len());
            let mut last = Value::Nil;
            for (place, value) in places.iter().zip(values) {
                last = value.primary_value();
                match place {
                    ncl_compiler::PsetfPlace::Symbol(name, escaped) => {
                        if *escaped {
                            runtime.set_or_define_exact_in(name, last.clone(), environment, span)?;
                        } else {
                            runtime.set_or_define_in(name, last.clone(), environment, span)?;
                        }
                    }
                    ncl_compiler::PsetfPlace::List(accessors, name, escaped) => {
                        let current = if *escaped {
                            runtime.lookup_exact_in(name, environment)
                        } else {
                            runtime.lookup_in(name, environment)
                        }
                        .ok_or_else(|| invalid("unbound PSETF list target", span))?;
                        let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                            expected: "LIST".to_string(),
                            actual: current.type_name().to_string(),
                            span: Some(span),
                        })?;
                        let updated = Value::list(super::assignment::list::nested::update(
                            elements, accessors, &last, span,
                        )?);
                        if *escaped {
                            runtime.set_or_define_exact_in(name, updated, environment, span)?;
                        } else {
                            runtime.set_or_define_in(name, updated, environment, span)?;
                        }
                    }
                }
            }
            stack.push(last);
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
