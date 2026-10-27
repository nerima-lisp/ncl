use super::held::{held_list, held_symbol};
use super::{ObjectError, Result, Runtime, ThreadContext, Word};
use ncl_object::LispError;

pub(super) fn type_declaration(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    type_spec: usize,
    variable: usize,
) -> Result<usize> {
    let type_operator = held_symbol(ctx, runtime, held, "COMMON-LISP::TYPE")?;
    let type_declaration = held_list(ctx, runtime, held, &[type_operator, type_spec, variable])?;
    let declare = held_symbol(ctx, runtime, held, "COMMON-LISP::DECLARE")?;
    held_list(ctx, runtime, held, &[declare, type_declaration])
}

fn loop_type_error(ctx: &mut ThreadContext, runtime: &Runtime, message: &str) -> ObjectError {
    let characters = message.chars().collect::<Vec<_>>();
    match ncl_object::make_string(ctx, runtime, &characters) {
        Ok(format_control) => {
            ctx.set_pending_lisp_error(LispError::SimpleError {
                format_control,
                format_arguments: Word::NIL,
            });
            ObjectError::TypeError
        }
        Err(error) => error,
    }
}

fn invalid_type_specifier(ctx: &mut ThreadContext, runtime: &Runtime) -> ObjectError {
    loop_type_error(ctx, runtime, "Invalid LOOP type specifier")
}

fn unsupported_type_default(ctx: &mut ThreadContext, runtime: &Runtime) -> ObjectError {
    loop_type_error(
        ctx,
        runtime,
        "Unsupported LOOP type default: no supported initial value satisfies the type specifier",
    )
}

pub(super) fn typed_placeholder(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    type_spec: usize,
) -> Result<usize> {
    typed_placeholder_with_candidates(ctx, runtime, held, type_spec, &[Word::NIL, Word::fixnum(0)])
}

pub(super) fn typed_numeric_placeholder(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    type_spec: usize,
) -> Result<usize> {
    typed_placeholder_with_candidates(ctx, runtime, held, type_spec, &[Word::fixnum(0)])
}

fn typed_placeholder_with_candidates(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    type_spec: usize,
    candidates: &[Word],
) -> Result<usize> {
    for candidate in candidates.iter().copied() {
        if let Some(candidate) = typed_candidate_matches(ctx, runtime, held, type_spec, candidate)?
        {
            held.push(candidate);
            return Ok(held.len() - 1);
        }
    }

    let (float, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let float = ncl_object::make_double(ctx, runtime, 0.0)?.as_word();
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((float, refreshed))
    })?;
    *held = refreshed;
    let float_index = held.len();
    held.push(float);

    let float = held
        .get(float_index)
        .copied()
        .ok_or(ObjectError::TypeError)?;
    if typed_candidate_matches(ctx, runtime, held, type_spec, float)?.is_some() {
        Ok(float_index)
    } else {
        Err(unsupported_type_default(ctx, runtime))
    }
}

fn typed_candidate_matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    type_spec: usize,
    candidate: Word,
) -> Result<Option<Word>> {
    let candidate_index = held.len();
    held.push(candidate);
    let mut invalid_type = false;
    let check = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let check = (|| {
            let type_word = roots
                .get(type_spec)
                .map(|root| **root)
                .ok_or(ObjectError::TypeError)?;
            let specifier = ncl_types::parse_type_specifier(ctx, type_word).map_err(|_| {
                invalid_type = true;
                ObjectError::TypeError
            })?;
            let candidate = roots
                .get(candidate_index)
                .map(|root| **root)
                .ok_or(ObjectError::TypeError)?;
            ncl_types::typep(ctx, candidate, &specifier).map_err(|_| ObjectError::TypeError)
        })();
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((check, refreshed))
    });
    let (check, refreshed) = match check {
        Ok(result) => result,
        Err(error) => {
            held.pop();
            return Err(error);
        }
    };
    *held = refreshed;
    let candidate = held.pop().ok_or(ObjectError::TypeError)?;
    match check {
        Ok(true) => Ok(Some(candidate)),
        Ok(false) => Ok(None),
        Err(ObjectError::TypeError) if invalid_type => Err(invalid_type_specifier(ctx, runtime)),
        Err(ObjectError::TypeError) => Err(unsupported_type_default(ctx, runtime)),
        Err(error) => Err(error),
    }
}
