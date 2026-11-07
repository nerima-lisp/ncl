use crate::executor::{ExecutionState, Flow, FormatError, FormatErrorKind, execute_control};
use crate::model::Directive;

use super::{
    FlowResult, Output, State, invalid_use, list_elements, object_error, string_value,
    with_rooted_words,
};

pub(super) fn execute(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    if directive.colon || !directive.parameters.is_empty() {
        return Err(invalid_use(directive));
    }
    let control = state.take_argument(directive)?;
    let control =
        string_value(state.ctx, control).map_err(|error| object_error(directive, error))?;
    let parsed = crate::parser::parse(&control).map_err(|error| {
        FormatError::new(
            error.offset,
            error.directive,
            FormatErrorKind::Parse(error.kind),
        )
    })?;
    let flow = if directive.at_sign {
        execute_control(state, &parsed, output)?
    } else {
        let list = state.take_argument(directive)?;
        let arguments = list_elements(state.ctx, list, directive)?;
        let previous_is_one = state.previous_is_one;
        let runtime = state.runtime;
        let frames = state.frames.clone();
        let (flow, previous_is_one) =
            with_rooted_words(state.ctx, &arguments, directive, |ctx, rooted| {
                let mut nested = ExecutionState {
                    arguments: rooted,
                    index: 0,
                    previous_is_one,
                    frames,
                    ctx,
                    runtime,
                };
                let flow = execute_control(&mut nested, &parsed, output)?;
                Ok((flow, nested.previous_is_one))
            })?;
        state.previous_is_one = previous_is_one;
        flow
    };
    Ok(if flow == Flow::Escape {
        Flow::Continue
    } else {
        flow
    })
}
