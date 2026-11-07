use ncl_printer::StringSink;

use crate::executor::{ControlFrame, ExecutionState, Flow, FormatOutput, execute_control};
use crate::model::{
    ControlSegment, Directive, DirectiveBody, DirectiveKind, FormatControl, Parameter,
};
use crate::{FormatError, FormatErrorKind};

use super::layout::parameter_usize;

#[derive(Clone, Copy)]
struct JustificationParameters {
    special_prefix: bool,
    mincol: usize,
    colinc: usize,
    minpad: usize,
    padchar: char,
}

pub(super) fn execute_justification(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
) -> Result<(), FormatError> {
    let DirectiveBody::Clauses(segments) = &directive.body else {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    };
    if segments.is_empty() {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidDirectiveUse,
        ));
    }
    let parameters = state.parameters(directive)?;
    if parameters.len() > 4 {
        return Err(FormatError::at(
            directive,
            FormatErrorKind::InvalidParameter,
        ));
    }
    let mincol = parameter_usize(&parameters, 0, 0, directive)?;
    let colinc = parameter_usize(&parameters, 1, 1, directive)?;
    let minpad = parameter_usize(&parameters, 2, 0, directive)?;
    let padchar = match parameters.get(3) {
        None | Some(Parameter::Unsupplied) => ' ',
        Some(Parameter::Character(character)) => *character,
        Some(Parameter::Integer(_) | Parameter::Relative | Parameter::Remaining) => {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidParameter,
            ));
        }
    };
    validate_separators(segments, directive)?;
    let special_prefix = segments
        .get(1)
        .and_then(|segment| segment.separator.as_ref())
        .is_some_and(|separator| separator.colon);
    let saved_frames = state.frames.clone();
    state.frames.push(ControlFrame::Justification);
    let result = render_justified(
        directive,
        state,
        output,
        segments,
        JustificationParameters {
            special_prefix,
            mincol,
            colinc,
            minpad,
            padchar,
        },
    );
    state.frames = saved_frames;
    result
}

fn validate_separators(
    segments: &[ControlSegment],
    directive: &Directive,
) -> Result<(), FormatError> {
    for (index, segment) in segments.iter().enumerate().skip(1) {
        let Some(separator) = &segment.separator else {
            continue;
        };
        let prefix_separator = index == 1 && separator.colon;
        if separator.at_sign
            || (!prefix_separator && separator.colon)
            || (!prefix_separator && !separator.parameters.is_empty())
            || (prefix_separator && separator.parameters.len() > 2)
        {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidDirectiveUse,
            ));
        }
    }
    Ok(())
}

fn render_justified(
    directive: &Directive,
    state: &mut ExecutionState<'_>,
    output: &mut FormatOutput<'_>,
    segments: &[ControlSegment],
    parameters: JustificationParameters,
) -> Result<(), FormatError> {
    let JustificationParameters {
        special_prefix,
        mincol,
        colinc,
        minpad,
        padchar,
    } = parameters;
    let first = usize::from(special_prefix);
    let mut prefix = String::new();
    let mut line_width = output.margin;
    let mut spare = 0;
    let mut prefix_escaped = false;
    if special_prefix {
        let Some(segment) = segments.first() else {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidDirectiveUse,
            ));
        };
        let (rendered_prefix, flow) = render_segment(state, &segment.control, output)?;
        prefix = rendered_prefix;
        prefix_escaped = flow != Flow::Continue;
        if !prefix_escaped {
            let separator = segments
                .get(1)
                .and_then(|segment| segment.separator.as_ref());
            let (minimum_spare, requested_width) =
                separator_parameters(state, separator, directive)?;
            spare = minimum_spare;
            line_width = requested_width.unwrap_or(output.margin);
        }
    }
    let mut pieces = Vec::new();
    if prefix_escaped {
        pieces.push(prefix.clone());
    } else {
        for segment in segments.iter().skip(first) {
            let (rendered, flow) = render_segment(state, &segment.control, output)?;
            pieces.push(rendered);
            if flow != Flow::Continue {
                break;
            }
        }
    }
    let field = justify(&pieces, mincol, colinc, minpad, padchar, directive)?;
    if special_prefix
        && !prefix_escaped
        && effective_column(output)
            .checked_add(spare)
            .and_then(|width| width.checked_add(display_width(&field)))
            .is_none_or(|width| width > line_width)
    {
        output.write_char('\n')?;
        output.write_str(&prefix)?;
    }
    output.write_str(&field)
}

fn render_segment(
    state: &mut ExecutionState<'_>,
    control: &FormatControl,
    layout: &FormatOutput<'_>,
) -> Result<(String, Flow), FormatError> {
    let mut sink = StringSink::new();
    let flow = {
        let mut buffered = FormatOutput::new(&mut sink);
        buffered.margin = layout.margin;
        buffered.indentation = layout.indentation;
        buffered.block_start = layout.block_start;
        buffered.pretty = layout.pretty;
        execute_control(state, control, &mut buffered)?
    };
    Ok((sink.into_string(), flow))
}

fn separator_parameters(
    state: &mut ExecutionState<'_>,
    separator: Option<&crate::model::DirectiveModifiers>,
    directive: &Directive,
) -> Result<(usize, Option<usize>), FormatError> {
    let Some(separator) = separator else {
        return Ok((0, None));
    };
    let synthetic = Directive {
        position: separator.position,
        parameters: separator.parameters.clone(),
        colon: true,
        at_sign: false,
        kind: DirectiveKind::Justification,
        function_name: None,
        body: DirectiveBody::None,
        terminator: None,
    };
    let parameters = state.parameters(&synthetic)?;
    Ok((
        parameter_usize(&parameters, 0, 0, directive)?,
        match parameters.get(1) {
            None | Some(Parameter::Unsupplied) => None,
            Some(
                Parameter::Integer(_)
                | Parameter::Character(_)
                | Parameter::Relative
                | Parameter::Remaining,
            ) => Some(parameter_usize(&parameters, 1, 0, directive)?),
        },
    ))
}

fn justify(
    pieces: &[String],
    mincol: usize,
    colinc: usize,
    minpad: usize,
    padchar: char,
    directive: &Directive,
) -> Result<String, FormatError> {
    if pieces.is_empty() {
        return Ok(String::new());
    }
    let widths = pieces
        .iter()
        .map(|piece| display_width(piece))
        .collect::<Vec<_>>();
    let text_width = widths
        .iter()
        .try_fold(0_usize, |sum, width| sum.checked_add(*width))
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    let gaps = pieces.len().saturating_sub(1);
    let minimum_padding = minpad
        .checked_mul(gaps)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    let required = text_width
        .checked_add(minimum_padding)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    let target = mincol.max(required);
    let width = if target <= mincol || colinc == 0 {
        target
    } else {
        let steps = (target - mincol)
            .checked_add(colinc - 1)
            .and_then(|distance| distance.checked_div(colinc))
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
        mincol
            .checked_add(
                steps
                    .checked_mul(colinc)
                    .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?,
            )
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?
    };
    let extra = width
        .checked_sub(text_width)
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    if pieces.len() == 1 && !directive.colon && !directive.at_sign {
        let Some(piece) = pieces.first() else {
            return Err(FormatError::at(
                directive,
                FormatErrorKind::InvalidParameter,
            ));
        };
        return Ok(format!("{}{}", pad(padchar, extra), piece));
    }
    let leading = usize::from(directive.colon);
    let trailing = usize::from(directive.at_sign);
    let slots = gaps
        .checked_add(leading)
        .and_then(|n| n.checked_add(trailing))
        .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
    if slots == 0 {
        return Ok(pieces.concat());
    }
    let distributable = extra.saturating_sub(minimum_padding);
    let share = distributable / slots;
    let remainder = distributable % slots;
    let mut padding = Vec::with_capacity(slots);
    for slot in 0..slots {
        let internal = slot >= leading && slot < leading + gaps;
        padding.push(usize::from(internal) * minpad + share + usize::from(slot < remainder));
    }
    let mut result = String::new();
    let mut slot = 0;
    if leading == 1 {
        let amount = padding
            .get(slot)
            .copied()
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
        result.push_str(&pad(padchar, amount));
        slot += 1;
    }
    for (index, piece) in pieces.iter().enumerate() {
        result.push_str(piece);
        if index < gaps {
            let amount = padding
                .get(slot)
                .copied()
                .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
            result.push_str(&pad(padchar, amount));
            slot += 1;
        }
    }
    if trailing == 1 {
        let amount = padding
            .get(slot)
            .copied()
            .ok_or_else(|| FormatError::at(directive, FormatErrorKind::InvalidParameter))?;
        result.push_str(&pad(padchar, amount));
    }
    Ok(result)
}

fn pad(character: char, count: usize) -> String {
    std::iter::repeat_n(character, count).collect()
}

fn display_width(text: &str) -> usize {
    text.rsplit('\n')
        .next()
        .map_or(0, |line| line.chars().count())
}

const fn effective_column(output: &FormatOutput<'_>) -> usize {
    if output.line_start {
        output.indentation
    } else {
        output.column
    }
}
