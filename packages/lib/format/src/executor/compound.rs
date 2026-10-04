use ncl_printer::StringSink;

use crate::{ControlPart, Directive, DirectiveKind};

use super::parameters::{parameter_usize, parameter_width};
use super::{ExecutionState, FormatError, execute_parts};

pub(super) fn matching(
    parts: &[ControlPart],
    start: usize,
    end: usize,
    opening: DirectiveKind,
    closing: DirectiveKind,
) -> Result<usize, FormatError> {
    let mut depth = 0usize;
    for (offset, part) in parts
        .iter()
        .enumerate()
        .skip(start + 1)
        .take(end.saturating_sub(start + 1))
    {
        if let ControlPart::Directive(directive) = part {
            if directive.kind == opening {
                depth += 1;
            }
            if directive.kind == closing {
                if depth == 0 {
                    return Ok(offset);
                }
                depth -= 1;
            }
        }
    }
    Err(FormatError::InvalidParameter { directive: opening })
}

pub(super) fn execute_justification(
    parts: &[ControlPart],
    index: usize,
    end: usize,
    directive: &Directive,
    state: &mut ExecutionState<'_>,
) -> Result<Option<usize>, FormatError> {
    let close = matching(
        parts,
        index,
        end,
        DirectiveKind::Less,
        DirectiveKind::Greater,
    )?;
    let segments = split_justification(parts, index + 1, close);
    let mut rendered = Vec::with_capacity(segments.len());
    for (start, segment_end) in segments {
        let mut local = StringSink::new();
        let mut nested = ExecutionState {
            arguments: state.arguments,
            argument_index: state.argument_index,
            ctx: state.ctx,
            runtime: state.runtime,
            sink: &mut local,
            line_start: state.line_start,
            column: state.column,
            escape: state.escape,
            remaining_override: state.remaining_override,
        };
        execute_parts(parts, start, segment_end, &mut nested)?;
        rendered.push(local.into_string());
    }
    let mincol = parameter_width(directive.parameters.first(), directive.kind)?;
    let colinc = parameter_usize(directive.parameters.get(1), directive.kind)?
        .unwrap_or(1)
        .max(1);
    let minpad = parameter_usize(directive.parameters.get(2), directive.kind)?.unwrap_or(0);
    let padchar = directive
        .parameters
        .get(3)
        .and_then(|parameter| match parameter {
            crate::Parameter::Character(value) => Some(*value),
            crate::Parameter::Integer(_)
            | crate::Parameter::Relative
            | crate::Parameter::ArgumentCount
            | crate::Parameter::Unsupplied => None,
        })
        .unwrap_or(' ');
    let content_width: usize = rendered.iter().map(|text| text.chars().count()).sum();
    let required = content_width + minpad.saturating_mul(rendered.len().saturating_sub(1));
    let width = if required <= mincol {
        mincol
    } else {
        required + (colinc - required % colinc) % colinc
    };
    let padding = width.saturating_sub(content_width);
    if rendered.len() == 1 {
        if !directive.at_sign {
            write_padding(state, padding, padchar)?;
        }
        let text = rendered.first().ok_or(FormatError::InvalidParameter {
            directive: directive.kind,
        })?;
        state.sink.write_str(text).map_err(FormatError::from)?;
        if directive.at_sign {
            write_padding(state, padding, padchar)?;
        }
    } else {
        let gaps = rendered.len() - 1;
        let mut gap_sizes = vec![minpad; gaps];
        let extra = padding.saturating_sub(minpad * gaps);
        for (index, gap) in gap_sizes.iter_mut().enumerate() {
            *gap += extra / gaps + usize::from(index < extra % gaps);
        }
        if directive.colon {
            write_padding(state, padding.saturating_sub(extra), padchar)?;
        }
        for (index, text) in rendered.iter().enumerate() {
            state.sink.write_str(text).map_err(FormatError::from)?;
            if let Some(gap) = gap_sizes.get(index) {
                write_padding(state, *gap, padchar)?;
            }
        }
        if directive.at_sign {
            write_padding(state, padding.saturating_sub(extra), padchar)?;
        }
    }
    Ok(Some(close + 1))
}

fn write_padding(
    state: &mut ExecutionState<'_>,
    count: usize,
    character: char,
) -> Result<(), FormatError> {
    for _ in 0..count {
        state
            .sink
            .write_char(character)
            .map_err(FormatError::from)?;
    }
    Ok(())
}

pub(super) fn split_justification(
    parts: &[ControlPart],
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut segments = Vec::new();
    let mut segment_start = start;
    let mut depth = 0;
    for index in start..end {
        if let Some(ControlPart::Directive(directive)) = parts.get(index) {
            match directive.kind {
                DirectiveKind::BracketOpen
                | DirectiveKind::BraceOpen
                | DirectiveKind::ParenOpen
                | DirectiveKind::Less => depth += 1,
                DirectiveKind::BracketClose
                | DirectiveKind::BraceClose
                | DirectiveKind::ParenClose
                | DirectiveKind::Greater
                    if depth > 0 =>
                {
                    depth -= 1;
                }
                DirectiveKind::Semicolon if depth == 0 => {
                    segments.push((segment_start, index));
                    segment_start = index + 1;
                }
                DirectiveKind::A
                | DirectiveKind::S
                | DirectiveKind::C
                | DirectiveKind::R
                | DirectiveKind::D
                | DirectiveKind::B
                | DirectiveKind::O
                | DirectiveKind::X
                | DirectiveKind::F
                | DirectiveKind::E
                | DirectiveKind::G
                | DirectiveKind::Dollar
                | DirectiveKind::W
                | DirectiveKind::Underscore
                | DirectiveKind::ColonGreater
                | DirectiveKind::I
                | DirectiveKind::Slash
                | DirectiveKind::T
                | DirectiveKind::Star
                | DirectiveKind::Question
                | DirectiveKind::P
                | DirectiveKind::Bar
                | DirectiveKind::UpArrow
                | DirectiveKind::Newline
                | DirectiveKind::Percent
                | DirectiveKind::Ampersand
                | DirectiveKind::Tilde
                | DirectiveKind::Greater
                | DirectiveKind::BracketClose
                | DirectiveKind::BraceClose
                | DirectiveKind::ParenClose
                | DirectiveKind::Semicolon => {}
            }
        }
    }
    segments.push((segment_start, end));
    segments
}

pub(super) fn split_branches(
    parts: &[ControlPart],
    start: usize,
    end: usize,
) -> Vec<(usize, usize, bool)> {
    let mut result = Vec::new();
    let mut branch_start = start;
    let mut depth = 0;
    let mut default = false;
    for index in start..end {
        if let Some(ControlPart::Directive(directive)) = parts.get(index) {
            match directive.kind {
                DirectiveKind::BracketOpen => depth += 1,
                DirectiveKind::BracketClose if depth > 0 => depth -= 1,
                DirectiveKind::Semicolon if depth == 0 => {
                    result.push((branch_start, index, default));
                    branch_start = index + 1;
                    default = directive.colon;
                }
                DirectiveKind::A
                | DirectiveKind::S
                | DirectiveKind::C
                | DirectiveKind::R
                | DirectiveKind::D
                | DirectiveKind::B
                | DirectiveKind::O
                | DirectiveKind::X
                | DirectiveKind::F
                | DirectiveKind::E
                | DirectiveKind::G
                | DirectiveKind::Dollar
                | DirectiveKind::W
                | DirectiveKind::Underscore
                | DirectiveKind::Less
                | DirectiveKind::Greater
                | DirectiveKind::ColonGreater
                | DirectiveKind::I
                | DirectiveKind::Slash
                | DirectiveKind::T
                | DirectiveKind::Star
                | DirectiveKind::BraceOpen
                | DirectiveKind::BraceClose
                | DirectiveKind::Question
                | DirectiveKind::ParenOpen
                | DirectiveKind::ParenClose
                | DirectiveKind::P
                | DirectiveKind::Bar
                | DirectiveKind::UpArrow
                | DirectiveKind::Newline
                | DirectiveKind::Percent
                | DirectiveKind::Ampersand
                | DirectiveKind::Tilde
                | DirectiveKind::BracketClose
                | DirectiveKind::Semicolon => {}
            }
        }
    }
    result.push((branch_start, end, default));
    result
}
