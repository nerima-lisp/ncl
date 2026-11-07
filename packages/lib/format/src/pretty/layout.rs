use std::collections::HashSet;

use ncl_object::{ObjectError, ObjectRef, Word, car, cdr, classify_object, with_rooted_slice};

use crate::executor::{ControlFrame, ExecutionState, Flow, FormatOutput, execute_control};
use crate::model::{
    ControlPart, ControlSegment, Directive, DirectiveBody, FormatControl, Parameter, TerminatorKind,
};
use crate::{FormatError, FormatErrorKind};

pub(super) fn execute_underscore(
    directive: &Directive,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    if !directive.parameters.is_empty() {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    if output.pretty && ((directive.colon && directive.at_sign) || output.column >= output.margin) {
        output.write_char('\n')?;
    }
    Ok(())
}

pub(super) fn execute_indent(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    if directive.at_sign || directive.parameters.len() > 1 {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    let amount = match state.parameters(directive)?.first() {
        None | Some(Parameter::Unsupplied) => 0,
        Some(Parameter::Integer(value)) => *value,
        Some(_) => {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidParameter,
            ));
        }
    };
    if output.pretty {
        let base = if directive.colon {
            effective_column(output)
        } else {
            output.block_start
        };
        output.indentation = add_signed(base, amount)
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    }
    Ok(())
}

pub(super) fn execute_tab(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    if directive.parameters.len() > 2 {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    let parameters = state.parameters(directive)?;
    let origin = if directive.colon {
        output.block_start
    } else {
        0
    };
    let column = effective_column(output).saturating_sub(origin);
    let column_number = parameter_usize(&parameters, 0, 1, directive)?;
    let column_increment = parameter_usize(&parameters, 1, 1, directive)?;
    let spaces = if directive.at_sign {
        relative_tab_spaces(column, column_number, column_increment, directive)?
    } else {
        absolute_tab_spaces(column, column_number, column_increment, directive)?
    };
    for _ in 0..spaces {
        output.write_char(' ')?;
    }
    Ok(())
}

fn absolute_tab_spaces(
    column: usize,
    target: usize,
    increment: usize,
    directive: &Directive,
) -> Result<usize, FormatError> {
    if column < target {
        return Ok(target - column);
    }
    if increment == 0 {
        return Ok(0);
    }
    let steps = column
        .checked_sub(target)
        .and_then(|distance| distance.checked_div(increment))
        .and_then(|steps| steps.checked_add(1))
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    let next = target
        .checked_add(
            steps
                .checked_mul(increment)
                .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?,
        )
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    next.checked_sub(column)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))
}

fn relative_tab_spaces(
    column: usize,
    distance: usize,
    increment: usize,
    directive: &Directive,
) -> Result<usize, FormatError> {
    let target = column
        .checked_add(distance)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    if increment == 0 {
        return Ok(distance);
    }
    let remainder = target % increment;
    let alignment = if remainder == 0 {
        0
    } else {
        increment - remainder
    };
    distance
        .checked_add(alignment)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))
}

pub(super) fn parameter_usize(
    parameters: &[Parameter],
    index: usize,
    default: usize,
    directive: &Directive,
) -> Result<usize, FormatError> {
    match parameters.get(index) {
        None | Some(Parameter::Unsupplied) => Ok(default),
        Some(Parameter::Integer(value)) if *value >= 0 => usize::try_from(*value)
            .map_err(|_| FormatError::at(directive, FormatErrorKind::InvalidParameter)),
        Some(_) => Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        )),
    }
}

fn add_signed(base: usize, amount: i64) -> Option<usize> {
    if amount >= 0 {
        base.checked_add(usize::try_from(amount).ok()?)
    } else {
        base.checked_sub(usize::try_from(amount.unsigned_abs()).ok()?)
    }
}

fn effective_column(output: &FormatOutput<'_>) -> usize {
    if output.line_start {
        output.indentation
    } else {
        output.column
    }
}

pub(super) fn execute_logical_block(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    if !directive.parameters.is_empty()
        || !matches!(
            directive.terminator,
            Some(terminator)
                if terminator.kind == TerminatorKind::GreaterThan && terminator.colon
        )
    {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    let DirectiveBody::Clauses(segments) = &directive.body else {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    };
    let (prefix, body, suffix) = logical_block_parts(segments, directive)?;
    output.write_str(&prefix)?;
    let saved_block_start = output.block_start;
    let saved_indentation = output.indentation;
    output.block_start = effective_column(output);
    output.indentation = output.block_start;
    let fill = directive
        .terminator
        .is_some_and(|terminator| terminator.at_sign);
    let result = if directive.at_sign {
        execute_rest_block(state, body, output, fill)
    } else {
        execute_list_block(directive, state, body, output, fill)
    };
    output.block_start = saved_block_start;
    output.indentation = saved_indentation;
    result?;
    output.write_str(&suffix)
}

fn logical_block_parts<'a>(
    segments: &'a [ControlSegment],
    directive: &Directive,
) -> Result<(String, &'a FormatControl, String), FormatError> {
    match segments {
        [body] => Ok((
            if directive.colon { "(" } else { "" }.to_owned(),
            &body.control,
            if directive.colon { ")" } else { "" }.to_owned(),
        )),
        [prefix, body] => Ok((
            literal_control(&prefix.control, directive)?,
            &body.control,
            if directive.colon { ")" } else { "" }.into(),
        )),
        [prefix, body, suffix] => Ok((
            literal_control(&prefix.control, directive)?,
            &body.control,
            literal_control(&suffix.control, directive)?,
        )),
        _ => Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        )),
    }
}

fn literal_control(control: &FormatControl, directive: &Directive) -> Result<String, FormatError> {
    let mut text = String::new();
    for part in &control.parts {
        match part {
            ControlPart::Literal(literal) => text.push_str(literal),
            ControlPart::Directive(_) => {
                return Err(FormatError::at(
                    directive,
                    FormatErrorKind::InvalidDirectiveUse,
                ));
            }
        }
    }
    Ok(text)
}

fn execute_rest_block(
    state: &mut ExecutionState<'_>,
    body: &FormatControl,
    output: &mut FormatOutput<'_>,
    fill: bool,
) -> Result<(), FormatError> {
    let start = state.index;
    let previous_is_one = state.previous_is_one;
    let runtime = state.runtime;
    let mut frames = state.frames.clone();
    frames.push(ControlFrame::LogicalBlock);
    let flow = {
        let arguments = &state.arguments[start..];
        let mut nested = ExecutionState {
            arguments,
            index: 0,
            previous_is_one,
            frames,
            ctx: state.ctx,
            runtime,
        };
        execute_logical_control(&mut nested, body, output, fill).map(|flow| {
            state.index = state.arguments.len();
            state.previous_is_one = nested.previous_is_one;
            flow
        })?
    };
    let _ = flow;
    Ok(())
}

fn execute_list_block(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    body: &FormatControl,
    output: &mut FormatOutput<'_>,
    fill: bool,
) -> Result<(), FormatError> {
    let list = state.take_argument(directive)?;
    let elements = list_elements(state.ctx, list, directive)?;
    let previous_is_one = state.previous_is_one;
    let runtime = state.runtime;
    let mut frames = state.frames.clone();
    frames.push(ControlFrame::LogicalBlock);
    let mut nested_previous_is_one = previous_is_one;
    let mut execution_error = None;
    let rooted = with_rooted_slice(state.ctx, &elements, |ctx, rooted| {
        let mut nested = ExecutionState {
            arguments: rooted,
            index: 0,
            previous_is_one,
            frames,
            ctx,
            runtime,
        };
        nested_previous_is_one = nested.previous_is_one;
        match execute_logical_control(&mut nested, body, output, fill) {
            Ok(_) => Ok(()),
            Err(error) => {
                execution_error = Some(error);
                Err(ObjectError::TypeError)
            }
        }
    });
    if let Some(error) = execution_error {
        return Err(error);
    }
    state.previous_is_one = nested_previous_is_one;
    rooted.map_err(|_| FormatError::at(directive, FormatErrorKind::ImproperList))
}

fn execute_logical_control(
    state: &mut ExecutionState<'_>,
    control: &FormatControl,
    output: &mut FormatOutput<'_>,
    fill: bool,
) -> Result<Flow, FormatError> {
    for part in &control.parts {
        let flow = match part {
            ControlPart::Literal(text) if fill => {
                write_fill_literal(text, output)?;
                Flow::Continue
            }
            ControlPart::Literal(text) => {
                output.write_str(text)?;
                Flow::Continue
            }
            ControlPart::Directive(_) => execute_control(
                state,
                &FormatControl {
                    parts: vec![part.clone()],
                },
                output,
            )?,
        };
        if flow != Flow::Continue {
            return Ok(flow);
        }
    }
    Ok(Flow::Continue)
}

fn write_fill_literal(text: &str, output: &mut FormatOutput<'_>) -> Result<(), FormatError> {
    let mut remaining = text;
    while let Some(index) = remaining.find(' ') {
        let (word, tail) = remaining.split_at(index);
        output.write_str(word)?;
        let rest = &tail[1..];
        let next_word = rest
            .chars()
            .take_while(|character| !character.is_whitespace())
            .count();
        let width = effective_column(output)
            .saturating_add(1)
            .saturating_add(next_word);
        if !output.line_start && width > output.margin {
            output.write_char('\n')?;
        } else {
            output.write_char(' ')?;
        }
        remaining = rest;
    }
    output.write_str(remaining)
}

fn list_elements(
    ctx: &ncl_object::ThreadContext,
    list: Word,
    directive: &Directive,
) -> Result<Vec<Word>, FormatError> {
    let mut values = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = list;
    while cursor != Word::NIL {
        let ObjectRef::Cons(cons) = classify_object(ctx, cursor) else {
            return Err(FormatError::at(
                directive,
                if values.is_empty() {
                    FormatErrorKind::WrongArgumentType
                } else {
                    FormatErrorKind::ImproperList
                },
            ));
        };
        if !seen.insert(cons.bits()) {
            return Err(FormatError::at(directive, FormatErrorKind::ImproperList));
        }
        values.push(
            car(ctx, cons)
                .map_err(|_| FormatError::at(directive, FormatErrorKind::ImproperList))?,
        );
        cursor = cdr(ctx, cons)
            .map_err(|_| FormatError::at(directive, FormatErrorKind::ImproperList))?;
    }
    Ok(values)
}
