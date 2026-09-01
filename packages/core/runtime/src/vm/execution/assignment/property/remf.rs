#[allow(clippy::wildcard_imports)]
use super::super::super::*;

pub(super) fn execute(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let (current, setter) = match instruction {
        Instruction::Remf { name, escaped } => {
            let indicator = stack
                .pop()
                .ok_or_else(|| invalid("remf has no indicator", span))?
                .primary_value();
            let current = stack
                .pop()
                .ok_or_else(|| invalid("remf has no property list", span))?
                .primary_value();
            (
                current,
                RemfSetter::Symbol {
                    name,
                    escaped,
                    indicator,
                },
            )
        }
        Instruction::RemfGetDynamic => {
            let remf_indicator = stack
                .pop()
                .ok_or_else(|| invalid("remf has no indicator", span))?
                .primary_value();
            let property = stack
                .pop()
                .ok_or_else(|| invalid("remf get has no property", span))?
                .primary_value();
            let symbol = stack
                .pop()
                .ok_or_else(|| invalid("remf get has no target", span))?
                .primary_value();
            if symbol.symbol_reference().is_none() {
                return Err(invalid("remf get target must be a symbol", span));
            }
            let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
            let plist_items = plist.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            })?;
            if !plist_items.len().is_multiple_of(2) {
                return Err(invalid("REMF GET needs an even property list", span));
            }
            let current = (0..plist_items.len())
                .step_by(2)
                .find(|&index| plist_items[index].eq_value(&property))
                .map(|index| plist_items[index + 1].clone())
                .unwrap_or(Value::Nil);
            (
                current,
                RemfSetter::Get {
                    symbol,
                    property,
                    remf_indicator,
                },
            )
        }
        _ => return Ok(false),
    };
    let mut properties = current.list_items().ok_or_else(|| RuntimeError::Type {
        expected: "LIST".to_string(),
        actual: current.type_name().to_string(),
        span: Some(span),
    })?;
    if !properties.len().is_multiple_of(2) {
        return Err(invalid("REMF needs an even property list", span));
    }
    let indicator = match &setter {
        RemfSetter::Symbol { indicator, .. } => indicator,
        RemfSetter::Get { remf_indicator, .. } => remf_indicator,
    };
    let found_index = (0..properties.len())
        .step_by(2)
        .find(|&index| crate::builtins::eql_value(&properties[index], indicator));
    let found = found_index.is_some();
    if let Some(index) = found_index {
        properties.drain(index..=index + 1);
    }
    let updated = Value::list(properties);
    match setter {
        RemfSetter::Symbol { name, escaped, .. } => {
            if *escaped {
                runtime.set_or_define_exact_in(name, updated.clone(), environment, span)?;
            } else {
                runtime.set_or_define_in(name, updated.clone(), environment, span)?;
            }
        }
        RemfSetter::Get {
            symbol, property, ..
        } => {
            let plist = environment.symbol_plist(&symbol).unwrap_or(Value::Nil);
            let mut properties = plist.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: plist.type_name().to_string(),
                span: Some(span),
            })?;
            if let Some(index) = (0..properties.len())
                .step_by(2)
                .find(|&index| properties[index].eq_value(&property))
                .map(|index| index + 1)
            {
                properties[index] = updated.clone();
            } else {
                properties.extend([property, updated.clone()]);
            }
            environment.set_symbol_plist(&symbol, Value::list(properties));
        }
    }
    stack.push(Value::values(vec![updated, Value::boolean(found)]));
    *program_counter += 1;
    Ok(true)
}

enum RemfSetter<'a> {
    Symbol {
        name: &'a str,
        escaped: &'a bool,
        indicator: Value,
    },
    Get {
        symbol: Value,
        property: Value,
        remf_indicator: Value,
    },
}
