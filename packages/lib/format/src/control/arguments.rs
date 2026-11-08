use crate::executor::{ControlFrame, Flow};
use crate::model::Directive;

use super::{FlowResult, State, integer_parameter, invalid_parameter, invalid_use};

pub(super) fn execute_star(directive: &Directive, state: &mut State<'_>) -> FlowResult {
    let parameters = state.parameters(directive)?;
    if parameters.len() > 1 || (directive.colon && directive.at_sign) {
        return Err(invalid_parameter(directive));
    }
    let default = i64::from(!directive.at_sign);
    let count = usize::try_from(integer_parameter(&parameters, 0, default, directive)?)
        .map_err(|_| invalid_parameter(directive))?;
    if directive.colon {
        if count > state.index {
            return Err(invalid_parameter(directive));
        }
        state.index -= count;
    } else if directive.at_sign {
        if count > state.arguments.len() {
            return Err(invalid_parameter(directive));
        }
        state.index = count;
    } else {
        state.index = state
            .index
            .checked_add(count)
            .filter(|index| *index <= state.arguments.len())
            .ok_or_else(|| invalid_parameter(directive))?;
    }
    Ok(Flow::Continue)
}

pub(super) fn execute_up_arrow(directive: &Directive, state: &mut State<'_>) -> FlowResult {
    let parameters = state.parameters(directive)?;
    if directive.at_sign {
        return Err(invalid_use(directive));
    }
    if parameters.len() > 3 {
        return Err(invalid_parameter(directive));
    }
    let first = integer_parameter(&parameters, 0, 0, directive)?;
    let second = integer_parameter(&parameters, 1, 0, directive)?;
    let third = integer_parameter(&parameters, 2, 0, directive)?;
    let scope = colon_up_arrow_scope(&state.frames);
    if directive.colon && scope.is_none() {
        return Err(invalid_use(directive));
    }
    let escape = if parameters.is_empty() {
        directive.colon && matches!(scope, Some(UpArrowScope::Iteration { .. }))
            || state.remaining() == 0
    } else if parameters.len() == 1 {
        first == 0
    } else if parameters.len() == 2 {
        first == second
    } else {
        first <= second && second <= third
    };
    if !escape {
        return Ok(Flow::Continue);
    }
    if !directive.colon {
        return Ok(Flow::Escape);
    }
    match scope {
        Some(UpArrowScope::Iteration { last_sublist: true }) => Ok(Flow::EscapeIterations),
        Some(UpArrowScope::Iteration {
            last_sublist: false,
        }) => Ok(Flow::Continue),
        Some(UpArrowScope::Block) | None => Ok(Flow::Escape),
    }
}

#[derive(Clone, Copy)]
enum UpArrowScope {
    Iteration { last_sublist: bool },
    Block,
}

fn colon_up_arrow_scope(frames: &[ControlFrame]) -> Option<UpArrowScope> {
    let (index, frame) = frames.iter().enumerate().rev().find(|(_, frame)| {
        matches!(
            frame,
            ControlFrame::Iteration { .. }
                | ControlFrame::LogicalBlock
                | ControlFrame::Justification
        )
    })?;
    match frame {
        ControlFrame::Iteration {
            colon: true,
            last_sublist,
            ..
        } => Some(UpArrowScope::Iteration {
            last_sublist: *last_sublist,
        }),
        ControlFrame::Iteration { .. } => None,
        ControlFrame::LogicalBlock | ControlFrame::Justification => {
            let outer_frames = frames.get(..index)?;
            let outer_iteration = outer_frames
                .iter()
                .rev()
                .find(|frame| matches!(frame, ControlFrame::Iteration { .. }));
            match outer_iteration {
                Some(ControlFrame::Iteration { colon: true, .. }) => Some(UpArrowScope::Block),
                Some(
                    ControlFrame::Iteration { colon: false, .. }
                    | ControlFrame::LogicalBlock
                    | ControlFrame::Justification,
                )
                | None => None,
            }
        }
    }
}
