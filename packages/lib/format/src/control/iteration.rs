use ncl_object::{Runtime, ThreadContext, Word};

use crate::executor::{ControlFrame, ExecutionState, Flow, FormatError, execute_control};
use crate::model::{Directive, DirectiveBody, FormatControl, TerminatorKind};

use super::{
    FlowResult, Output, State, invalid_use, iteration_limit, list_elements, with_rooted_words,
};

pub(super) fn execute(
    directive: &Directive,
    state: &mut State<'_>,
    output: &mut Output<'_>,
) -> FlowResult {
    let parameters = state.parameters(directive)?;
    if parameters.len() > 1 {
        return Err(crate::executor::FormatError::at(
            directive,
            crate::executor::FormatErrorKind::InvalidParameter,
        ));
    }
    let limit = iteration_limit(&parameters, directive)?;
    let Some(terminator) = directive.terminator else {
        return Err(invalid_use(directive));
    };
    if terminator.kind != TerminatorKind::RightBrace || terminator.at_sign {
        return Err(invalid_use(directive));
    }
    let DirectiveBody::Sequence(body) = &directive.body else {
        return Err(invalid_use(directive));
    };
    let force_once = terminator.colon && limit != Some(0);
    match (directive.colon, directive.at_sign) {
        (false, false) => list_iteration(directive, state, output, body, limit, force_once),
        (false, true) => rest_iteration(directive, state, output, body, limit, force_once),
        (true, false) => sublist_iteration(directive, state, output, body, limit, force_once),
        (true, true) => rest_sublist_iteration(directive, state, output, body, limit, force_once),
    }
}

fn list_iteration(
    d: &Directive,
    s: &mut State<'_>,
    o: &mut Output<'_>,
    body: &FormatControl,
    limit: Option<usize>,
    force: bool,
) -> FlowResult {
    let list = s.take_argument(d)?;
    let values = list_elements(s.ctx, list, d)?;
    let previous_is_one = s.previous_is_one;
    let runtime = s.runtime;
    let mut frames = s.frames.clone();
    frames.push(ControlFrame::Iteration {
        colon: false,
        at_sign: false,
        last_sublist: false,
    });
    let (flow, previous_is_one) = with_rooted_words(s.ctx, &values, d, |ctx, rooted| {
        let mut local = ExecutionState {
            arguments: rooted,
            index: 0,
            previous_is_one,
            frames,
            ctx,
            runtime,
        };
        let mut repetitions = 0;
        loop {
            if limit.is_some_and(|max| repetitions >= max)
                || (local.remaining() == 0 && !(force && repetitions == 0))
            {
                break Ok((Flow::Continue, local.previous_is_one));
            }
            let index = local.index;
            repetitions += 1;
            match execute_control(&mut local, body, o)? {
                Flow::Continue if local.remaining() == 0 => {
                    break Ok((Flow::Continue, local.previous_is_one));
                }
                Flow::Continue if local.index == index => break Err(invalid_use(d)),
                Flow::Continue => {}
                Flow::Escape => break Ok((Flow::Continue, local.previous_is_one)),
                Flow::EscapeIterations => {
                    break Ok((Flow::EscapeIterations, local.previous_is_one));
                }
            }
        }
    })?;
    s.previous_is_one = previous_is_one;
    Ok(flow)
}

fn rest_iteration(
    d: &Directive,
    s: &mut State<'_>,
    o: &mut Output<'_>,
    body: &FormatControl,
    limit: Option<usize>,
    force: bool,
) -> FlowResult {
    s.frames.push(ControlFrame::Iteration {
        colon: false,
        at_sign: true,
        last_sublist: false,
    });
    let result = (|| {
        let mut repetitions = 0;
        loop {
            if limit.is_some_and(|max| repetitions >= max)
                || (s.remaining() == 0 && !(force && repetitions == 0))
            {
                break Ok(Flow::Continue);
            }
            let index = s.index;
            repetitions += 1;
            match execute_control(s, body, o)? {
                Flow::Continue if s.remaining() == 0 => break Ok(Flow::Continue),
                Flow::Continue if s.index == index => break Err(invalid_use(d)),
                Flow::Continue => {}
                Flow::Escape => break Ok(Flow::Continue),
                Flow::EscapeIterations => break Ok(Flow::EscapeIterations),
            }
        }
    })();
    let _ = s.frames.pop();
    result
}

fn sublist_iteration(
    d: &Directive,
    s: &mut State<'_>,
    o: &mut Output<'_>,
    body: &FormatControl,
    limit: Option<usize>,
    force: bool,
) -> FlowResult {
    let list = s.take_argument(d)?;
    let sublists = list_elements(s.ctx, list, d)?;
    let previous_is_one = s.previous_is_one;
    let runtime = s.runtime;
    let frames = s.frames.clone();
    let (_, previous_is_one) = with_rooted_words(s.ctx, &sublists, d, |ctx, rooted| {
        let mut previous_is_one = previous_is_one;
        let mut repetitions = 0;
        for (index, sublist) in rooted.iter().copied().enumerate() {
            if limit.is_some_and(|max| repetitions >= max) {
                break;
            }
            let args = list_elements(ctx, sublist, d)?;
            let (flow, next_previous_is_one) = sublist_body(
                ctx,
                runtime,
                &args,
                previous_is_one,
                &frames,
                body,
                d,
                o,
                false,
                index + 1 == rooted.len(),
            )?;
            previous_is_one = next_previous_is_one;
            repetitions += 1;
            if flow == Flow::EscapeIterations {
                break;
            }
        }
        if rooted.is_empty() && force && limit != Some(0) {
            let (_, next_previous_is_one) = sublist_body(
                ctx,
                runtime,
                &[],
                previous_is_one,
                &frames,
                body,
                d,
                o,
                false,
                true,
            )?;
            previous_is_one = next_previous_is_one;
        }
        Ok((Flow::Continue, previous_is_one))
    })?;
    s.previous_is_one = previous_is_one;
    Ok(Flow::Continue)
}

fn rest_sublist_iteration(
    d: &Directive,
    s: &mut State<'_>,
    o: &mut Output<'_>,
    body: &FormatControl,
    limit: Option<usize>,
    force: bool,
) -> FlowResult {
    let mut repetitions = 0;
    loop {
        if limit.is_some_and(|max| repetitions >= max) {
            break;
        }
        if s.remaining() == 0 {
            if force && repetitions == 0 {
                sublist_body(
                    s.ctx,
                    s.runtime,
                    &[],
                    s.previous_is_one,
                    &s.frames,
                    body,
                    d,
                    o,
                    true,
                    true,
                )?;
            }
            break;
        }
        let sublist = s.take_argument(d)?;
        let last_sublist = s.remaining() == 0;
        let args = list_elements(s.ctx, sublist, d)?;
        let flow = rest_sublist_body(d, s, body, o, &args, last_sublist)?;
        repetitions += 1;
        if flow == Flow::EscapeIterations {
            break;
        }
    }
    Ok(Flow::Continue)
}

fn rest_sublist_body(
    d: &Directive,
    s: &mut State<'_>,
    body: &FormatControl,
    o: &mut Output<'_>,
    args: &[Word],
    last_sublist: bool,
) -> FlowResult {
    let (flow, previous_is_one) = sublist_body(
        s.ctx,
        s.runtime,
        args,
        s.previous_is_one,
        &s.frames,
        body,
        d,
        o,
        true,
        last_sublist,
    )?;
    s.previous_is_one = previous_is_one;
    Ok(flow)
}

fn sublist_body(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &[Word],
    previous_is_one: Option<bool>,
    frames: &[ControlFrame],
    body: &FormatControl,
    d: &Directive,
    o: &mut Output<'_>,
    at_sign: bool,
    last_sublist: bool,
) -> Result<(Flow, Option<bool>), FormatError> {
    let mut frames = frames.to_vec();
    frames.push(ControlFrame::Iteration {
        colon: true,
        at_sign,
        last_sublist,
    });
    with_rooted_words(ctx, args, d, |ctx, rooted| {
        let mut local = ExecutionState {
            arguments: rooted,
            index: 0,
            previous_is_one,
            frames,
            ctx,
            runtime,
        };
        let flow = execute_control(&mut local, body, o)?;
        Ok((flow, local.previous_is_one))
    })
}
