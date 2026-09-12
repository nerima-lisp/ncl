#[allow(clippy::wildcard_imports)]
use super::*;
use ncl_syntax::Form;

pub(super) fn execute_setf_aref_vector_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf aref place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf aref place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf aref place has no vector", span))?;
    let index = stack
        .last()
        .ok_or_else(|| invalid("setf aref place has no index", span))?;
    runtime.set_aref_vector_place_value(place, index, &current, value.clone(), environment)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf aref place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_map_into_aref_vector_place(
    runtime: &Runtime,
    sequence_count: usize,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let required = sequence_count.saturating_add(3);
    if stack.len() < required {
        return Err(invalid(
            "map-into aref vector place has insufficient values",
            span,
        ));
    }
    let start = stack.len() - required;
    let index = stack[start].clone();
    let container = stack[start + 1].clone();
    let function = stack[start + 2].clone();
    let sequences = stack[start + 3..].to_vec();
    let index_number = Runtime::setf_index(index.clone(), span)?;
    let destination = container
        .vector_items()
        .and_then(|items| items.get(index_number).cloned())
        .ok_or_else(|| invalid("MAP-INTO AREF index is out of bounds", span))?;
    let result =
        runtime.apply_sequence_map_into(&destination, &function, &sequences, environment, span)?;
    runtime.set_aref_vector_place_value(place, &index, &container, result.clone(), environment)?;
    stack.truncate(start);
    stack.push(result);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_setf_aref_array_place(
    runtime: &Runtime,
    index_count: usize,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if index_count == 0 || stack.len() < index_count + 2 {
        return Err(invalid(
            "setf aref array place has insufficient values",
            span,
        ));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf aref array place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf aref array place has no array", span))?;
    let start = stack.len() - index_count;
    let indices = stack[start..].to_vec();
    runtime.set_aref_array_place_value(place, &indices, &current, value.clone(), environment)?;
    stack.truncate(start);
    stack.push(value);
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_setf_row_major_aref_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid(
            "setf row-major-aref place has insufficient values",
            span,
        ));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf row-major-aref place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf row-major-aref place has no array", span))?;
    let index = stack
        .last()
        .ok_or_else(|| invalid("setf row-major-aref place has no index", span))?;
    runtime.set_row_major_aref_place_value(place, index, &current, value.clone(), environment)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf row-major-aref place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_setf_svref_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf svref place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf svref place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf svref place has no vector", span))?;
    let index = stack
        .last()
        .ok_or_else(|| invalid("setf svref place has no index", span))?;
    runtime.set_svref_place_value(place, index, &current, value.clone(), environment)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf svref place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_setf_string_char_place(
    runtime: &Runtime,
    _operator: &str,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf char place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf char place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf char place has no string", span))?;
    let index = stack
        .last()
        .ok_or_else(|| invalid("setf char place has no index", span))?;
    runtime.set_string_char_place_value(place, index, &current, &value, environment)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf char place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) fn execute_setf_elt_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < 3 {
        return Err(invalid("setf elt place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf elt place has no value", span))?
        .primary_value();
    let current = stack
        .pop()
        .ok_or_else(|| invalid("setf elt place has no sequence", span))?;
    let index = stack
        .last()
        .ok_or_else(|| invalid("setf elt place has no index", span))?;
    runtime.set_elt_place_value(place, index, &current, value.clone(), environment)?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf elt place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}

pub(super) struct MapIntoListPlaceExecution<'a> {
    pub(super) runtime: &'a Runtime,
    pub(super) operator: &'a str,
    pub(super) place: &'a Form,
    pub(super) sequence_count: usize,
    pub(super) stack: &'a mut Vec<Value>,
    pub(super) environment: &'a Environment,
    pub(super) program_counter: &'a mut usize,
    pub(super) span: Span,
}

impl MapIntoListPlaceExecution<'_> {
    pub(super) fn execute(self) -> Result<bool, RuntimeError> {
        let Self {
            runtime,
            operator,
            place,
            sequence_count,
            stack,
            environment,
            program_counter,
            span,
        } = self;
        let required = sequence_count.saturating_add(2);
        if stack.len() < required {
            return Err(invalid("MAP-INTO list place has insufficient values", span));
        }
        let start = stack.len() - required;
        let destination = stack[start].clone();
        let function = stack[start + 1].clone();
        let sequences = stack[start + 2..].to_vec();
        let mapped_destination = match operator {
            "CAR" | "FIRST" => destination
                .list_items()
                .and_then(|items| items.first().cloned())
                .ok_or_else(|| invalid("cannot MAP-INTO CAR of NIL", span))?,
            "CDR" | "REST" => Value::list(
                destination
                    .list_items()
                    .map(|items| items.iter().skip(1).cloned().collect())
                    .ok_or_else(|| invalid("cannot MAP-INTO CDR of NIL", span))?,
            ),
            _ => return Err(invalid("unsupported MAP-INTO list place", span)),
        };
        let result = runtime.apply_sequence_map_into(
            &mapped_destination,
            &function,
            &sequences,
            environment,
            span,
        )?;
        runtime.set_list_place_value(
            operator,
            place,
            &destination,
            result.clone(),
            environment,
            span,
        )?;
        stack.truncate(start);
        stack.push(result);
        *program_counter += 1;
        Ok(true)
    }
}
