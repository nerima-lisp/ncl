use super::super::{Environment, Runtime, RuntimeError, Span, Value, invalid};
use ncl_syntax::{Form, FormKind};

pub(super) fn execute_setf_subseq_place(
    runtime: &Runtime,
    place: &Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    let FormKind::List(items) = &place.kind else {
        return Err(invalid("setf subseq place is malformed", span));
    };
    let has_end = items.len() == 4;
    let needed = if has_end { 4 } else { 3 };
    if stack.len() < needed {
        return Err(invalid("setf subseq place has insufficient values", span));
    }
    let value = stack
        .pop()
        .ok_or_else(|| invalid("setf subseq place has no value", span))?
        .primary_value();
    let end = if has_end {
        Some(
            stack
                .pop()
                .ok_or_else(|| invalid("setf subseq place has no end", span))?,
        )
    } else {
        None
    };
    let start = stack
        .pop()
        .ok_or_else(|| invalid("setf subseq place has no start", span))?;
    let current = stack
        .last()
        .ok_or_else(|| invalid("setf subseq place has no sequence", span))?;
    runtime.set_subseq_place_value(
        &items[1],
        current,
        &start,
        end.as_ref(),
        &value,
        environment,
        span,
    )?;
    *stack
        .last_mut()
        .ok_or_else(|| invalid("setf subseq place has no result", span))? = value;
    *program_counter += 1;
    Ok(true)
}
