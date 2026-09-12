#[allow(clippy::wildcard_imports)]
use super::*;
use ncl_syntax::Form;

pub(super) fn execute_setf_list_place(
    runtime: &Runtime,
    operator: &str,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid("setf list place has no place value", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf list place has no value", span))?
        .primary_value();
    let current = stack
        .last()
        .ok_or_else(|| invalid("setf list place has no place value", span))?;
    runtime.set_list_place_value(operator, place, current, value.clone(), environment, span)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf list place has no place value", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_psetf_list_places(
    runtime: &Runtime,
    places: &[(String, Form)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() * 2 {
        return Err(invalid("psetf list places has insufficient values", span));
    }
    let value_count = places.len();
    let base_len = stack.len() - value_count * 2;
    let first_place = base_len + value_count;
    let values = stack[base_len..first_place]
        .iter()
        .map(Value::primary_value)
        .collect::<Vec<_>>();
    for (index, (operator, place)) in places.iter().enumerate() {
        let current = stack[first_place + index].clone();
        runtime.set_list_place_value(
            operator,
            place,
            &current,
            values[index].clone(),
            environment,
            span,
        )?;
    }
    let result = values.last().cloned().unwrap_or(Value::Nil);
    stack.truncate(base_len);
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pop_list_place(
    runtime: &Runtime,
    operator: &str,
    place: &Form,
    stack: &mut [Value],
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let current = stack
        .last()
        .cloned()
        .ok_or_else(|| invalid("pop list place has no place value", span))?;
    let value = runtime.pop_list_place_value(operator, place, &current, environment, span)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("pop list place has no place value", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_push_list_place(
    runtime: &Runtime,
    operator: &str,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid("push list place has no item or place value", span));
    }
    let current = stack
        .pop()
        .ok_or_else(|| invalid("push list place has no place value", span))?;
    let item = stack
        .last()
        .cloned()
        .ok_or_else(|| invalid("push list place has no item", span))?;
    let value =
        runtime.push_list_place_value(operator, place, &current, item, environment, span)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("push list place has no item", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_pushnew_list_place(
    runtime: &Runtime,
    operator: &str,
    place: &Form,
    options: &[ncl_compiler::PushnewOption],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 2 {
        return Err(invalid(
            "pushnew list place has no item or place value",
            span,
        ));
    }
    let base_len = stack
        .len()
        .checked_sub(options.len() + 2)
        .ok_or_else(|| invalid("pushnew list place has no item or place value", span))?;
    let current = stack
        .pop()
        .ok_or_else(|| invalid("pushnew list place has no place value", span))?;
    let item = stack
        .pop()
        .ok_or_else(|| invalid("pushnew list place has no item", span))?;
    let option_values = (0..options.len())
        .map(|_| {
            stack
                .pop()
                .ok_or_else(|| invalid("pushnew list place has no option value", span))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut test = None;
    let mut test_not = None;
    let mut key = None;
    for (option, value) in options.iter().zip(option_values.into_iter().rev()) {
        match option {
            ncl_compiler::PushnewOption::Test => test = Some(value),
            ncl_compiler::PushnewOption::TestNot => test_not = Some(value),
            ncl_compiler::PushnewOption::Key => key = Some(value),
        }
    }
    let value = runtime.pushnew_list_place_value_with_options(
        operator,
        place,
        &current,
        item,
        test,
        test_not,
        key,
        environment,
        span,
    )?;
    stack.truncate(base_len);
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}
