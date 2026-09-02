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
            runtime,
            places,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PsetfPlaces(places) => {
            let dynamic_targets = places
                .iter()
                .map(|place| match place {
                    ncl_compiler::PsetfPlace::SymbolPlist => 1,
                    ncl_compiler::PsetfPlace::Get => 2,
                    ncl_compiler::PsetfPlace::Nth(..) => 2,
                    _ => 0,
                })
                .sum::<usize>();
            if stack.len() < places.len() + dynamic_targets {
                return Err(invalid("psetf has fewer values than targets", span));
            }
            let operands = stack.split_off(stack.len() - places.len() - dynamic_targets);
            let (values, targets) = operands.split_at(places.len());
            let mut target_index = 0;
            let mut last = Value::Nil;
            for (place, value) in places.iter().zip(values.iter()) {
                last = value.primary_value();
                match place {
                    ncl_compiler::PsetfPlace::Symbol(name, escaped) => {
                        if *escaped {
                            runtime.set_or_define_exact_in(
                                name,
                                last.clone(),
                                environment,
                                span,
                            )?;
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
                    ncl_compiler::PsetfPlace::Nth(accessors, name, escaped) => {
                        let index = crate::builtins::index_argument(
                            "PSETF NTH",
                            &targets[target_index].primary_value(),
                        )?;
                        let current = targets[target_index + 1].primary_value();
                        target_index += 2;
                        let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                            expected: "LIST".to_string(),
                            actual: current.type_name().to_string(),
                            span: Some(span),
                        })?;
                        let elements = super::assignment::list::nested::update_dynamic(
                            elements, accessors, index, &last, span,
                        )?;
                        let updated = Value::list(elements);
                        if *escaped {
                            runtime.set_or_define_exact_in(name, updated, environment, span)?;
                        } else {
                            runtime.set_or_define_in(name, updated, environment, span)?;
                        }
                    }
                    ncl_compiler::PsetfPlace::SymbolPlist => {
                        let target = targets[target_index].primary_value();
                        target_index += 1;
                        if target.symbol_reference().is_none() {
                            return Err(invalid(
                                "psetf symbol-plist target must be a symbol",
                                span,
                            ));
                        }
                        let properties = last.list_items().ok_or_else(|| RuntimeError::Type {
                            expected: "LIST".to_string(),
                            actual: last.type_name().to_string(),
                            span: Some(span),
                        })?;
                        if !properties.len().is_multiple_of(2) {
                            return Err(invalid("SYMBOL-PLIST needs an even property list", span));
                        }
                        environment.set_symbol_plist(&target, last.clone());
                    }
                    ncl_compiler::PsetfPlace::Get => {
                        let target = targets[target_index].primary_value();
                        let indicator = targets[target_index + 1].primary_value();
                        target_index += 2;
                        if target.symbol_reference().is_none() {
                            return Err(invalid("psetf GET target must be a symbol", span));
                        }
                        let plist = environment.symbol_plist(&target).unwrap_or(Value::Nil);
                        let mut properties =
                            plist.list_items().ok_or_else(|| RuntimeError::Type {
                                expected: "LIST".to_string(),
                                actual: plist.type_name().to_string(),
                                span: Some(span),
                            })?;
                        if !properties.len().is_multiple_of(2) {
                            return Err(invalid("PSETF GET needs an even property list", span));
                        }
                        if let Some(index) = (0..properties.len())
                            .step_by(2)
                            .find(|&index| properties[index].eq_value(&indicator))
                            .map(|index| index + 1)
                        {
                            properties[index] = last.clone();
                        } else {
                            properties.extend([indicator, last.clone()]);
                        }
                        environment.set_symbol_plist(&target, Value::list(properties));
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
