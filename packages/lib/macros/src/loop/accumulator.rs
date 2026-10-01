use super::held::{held_form, held_fresh_symbol, held_list};
use super::{AccumulatorKind, Result, Runtime, ThreadContext, Word};

#[allow(clippy::too_many_arguments)]
pub(super) fn expand_accumulator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    kind: AccumulatorKind,
    value: usize,
    variable: Option<usize>,
    bindings: &mut Vec<usize>,
    body: &mut Vec<usize>,
    initialized_accumulators: &mut Vec<usize>,
) -> Result<(usize, AccumulatorKind)> {
    let accumulator = match variable {
        Some(variable) => variable,
        None => held_fresh_symbol(ctx, runtime, held)?,
    };
    if !initialized_accumulators
        .iter()
        .any(|index| held.get(*index) == held.get(accumulator))
    {
        initialized_accumulators.push(accumulator);
        let init = match kind {
            AccumulatorKind::Count | AccumulatorKind::Sum => Word::fixnum(0),
            AccumulatorKind::Collect
            | AccumulatorKind::Append
            | AccumulatorKind::Nconc
            | AccumulatorKind::Maximize
            | AccumulatorKind::Minimize => Word::NIL,
        };
        let init_index = held.len();
        held.push(init);
        bindings.push(held_list(ctx, runtime, held, &[accumulator, init_index])?);
    }
    match kind {
        AccumulatorKind::Collect => body.push(held_form(
            ctx,
            runtime,
            held,
            "PUSH",
            &[value, accumulator],
        )?),
        AccumulatorKind::Append => {
            let appended = held_form(ctx, runtime, held, "APPEND", &[accumulator, value])?;
            body.push(held_form(
                ctx,
                runtime,
                held,
                "SETQ",
                &[accumulator, appended],
            )?);
        }
        AccumulatorKind::Nconc => {
            let concatenated = held_form(ctx, runtime, held, "NCONC", &[accumulator, value])?;
            body.push(held_form(
                ctx,
                runtime,
                held,
                "SETQ",
                &[accumulator, concatenated],
            )?);
        }
        AccumulatorKind::Count => {
            let one = held.len();
            held.push(Word::fixnum(1));
            let increment = held_form(ctx, runtime, held, "INCF", &[accumulator, one])?;
            body.push(held_form(ctx, runtime, held, "WHEN", &[value, increment])?);
        }
        AccumulatorKind::Sum => body.push(held_form(
            ctx,
            runtime,
            held,
            "INCF",
            &[accumulator, value],
        )?),
        AccumulatorKind::Maximize | AccumulatorKind::Minimize => {
            let operator = if matches!(kind, AccumulatorKind::Maximize) {
                "MAX"
            } else {
                "MIN"
            };
            let selected = held_form(ctx, runtime, held, operator, &[accumulator, value])?;
            let selected = held_form(ctx, runtime, held, "SETQ", &[accumulator, selected])?;
            let set_first = held_form(ctx, runtime, held, "SETQ", &[accumulator, value])?;
            let uninitialized = held_form(ctx, runtime, held, "NULL", &[accumulator])?;
            body.push(held_form(
                ctx,
                runtime,
                held,
                "IF",
                &[uninitialized, set_first, selected],
            )?);
        }
    }
    Ok((accumulator, kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{elements, symbol};
    use ncl_object::ObjectError;

    #[test]
    fn maximize_uses_nil_accumulator_as_initialization_state()
    -> std::result::Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let accumulator = symbol(&mut ctx, &runtime, "RESULT")?;
        let mut held = vec![accumulator, Word::fixnum(3)];
        let mut bindings = Vec::new();
        let mut body = Vec::new();
        let mut initialized = Vec::new();

        expand_accumulator(
            &mut ctx,
            &runtime,
            &mut held,
            AccumulatorKind::Maximize,
            1,
            Some(0),
            &mut bindings,
            &mut body,
            &mut initialized,
        )?;

        let if_form = elements(&mut ctx, held[body[0]])?;
        assert_eq!(if_form[0], symbol(&mut ctx, &runtime, "IF")?);
        let null_form = elements(&mut ctx, if_form[1])?;
        assert_eq!(null_form[0], symbol(&mut ctx, &runtime, "NULL")?);
        assert!(!held.contains(&Word::TRUE));
        Ok(())
    }
}
