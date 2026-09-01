pub fn execute_evaluation_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "evaluation operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_evaluation_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown evaluation operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_package_introspection_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "package introspection operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_package_introspection_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown package introspection operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_package_mutation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "package mutation operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = match operation {
        "USE-PACKAGE" | "UNUSE-PACKAGE" | "EXPORT" | "UNEXPORT" => {
            runtime.apply_package_use_primitive(operation, &arguments, span)
        }
        "IMPORT" | "SHADOWING-IMPORT" | "SHADOW" | "UNINTERN" => {
            runtime.apply_package_symbol_primitive(operation, &arguments, span)
        }
        _ => None,
    }
    .unwrap_or_else(|| Err(invalid("unknown package mutation operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_package_listing_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "package listing operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_package_listing_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown package listing operation", span)))?;
    stack.push(value);
    Ok(())
}

#[allow(clippy::wildcard_imports)]
use super::*;

pub fn execute_property_list_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "property-list operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = match operation {
        "GETF" => crate::builtins::getf(&arguments),
        "GET-PROPERTIES" => crate::builtins::get_properties(&arguments),
        "GET" | "PUTPROP" | "REMPROP" | "SYMBOL-PLIST" => runtime
            .apply_symbol_property_primitive(operation, &arguments, environment, span)
            .unwrap_or_else(|| Err(invalid("unknown property-list operation", span))),
        _ => Err(invalid("unknown property-list operation", span)),
    }?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_value_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "symbol value operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_symbol_value_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol value operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_binding_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "symbol binding operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_symbol_property_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol binding operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_function_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "symbol function operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_symbol_function_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol function operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_symbol_creation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "symbol creation operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_symbol_creation_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown symbol creation operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_class_introspection_instruction(
    _runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "class introspection operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value =
        Runtime::apply_class_introspection_primitive(operation, &arguments, environment, span)
            .unwrap_or_else(|| Err(invalid("unknown class introspection operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_slot_operation_instruction(
    _runtime: &Runtime,
    stack: &mut Vec<Value>,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("slot operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = Runtime::apply_slot_primitive(operation, &arguments, span)
        .unwrap_or_else(|| Err(invalid("unknown slot operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_condition_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid(
            "condition operation has too few stack values",
            span,
        ));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_condition_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown condition operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_restart_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("restart operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_restart_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown restart operation", span)))?;
    stack.push(value);
    Ok(())
}

pub fn execute_method_operation_instruction(
    runtime: &Runtime,
    stack: &mut Vec<Value>,
    environment: &Environment,
    operation: &str,
    argument_count: usize,
    span: Span,
) -> Result<(), RuntimeError> {
    if stack.len() < argument_count {
        return Err(invalid("method operation has too few stack values", span));
    }
    let arguments = stack
        .split_off(stack.len() - argument_count)
        .into_iter()
        .map(|value| value.primary_value())
        .collect::<Vec<_>>();
    let value = runtime
        .apply_method_primitive(operation, &arguments, environment, span)
        .unwrap_or_else(|| Err(invalid("unknown method operation", span)))?;
    stack.push(value);
    Ok(())
}
