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

fn unsupported_type_default(ctx: &mut ThreadContext, runtime: &Runtime) -> ObjectError {
    let message =
        "Unsupported LOOP type default: no supported initial value satisfies the type specifier";
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
    let direct_match = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let type_word = roots
            .get(type_spec)
            .map(|root| **root)
            .ok_or(ObjectError::TypeError)?;
        let specifier =
            ncl_types::parse_type_specifier(ctx, type_word).map_err(|_| ObjectError::TypeError)?;
        let mut candidate = None;
        for value in candidates.iter().copied() {
            if ncl_types::typep(ctx, value, &specifier).map_err(|_| ObjectError::TypeError)? {
                candidate = Some(value);
                break;
            }
        }
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((candidate, refreshed))
    });
    let (candidate, refreshed) = match direct_match {
        Ok(result) => result,
        Err(ObjectError::TypeError) => return Err(unsupported_type_default(ctx, runtime)),
        Err(error) => return Err(error),
    };
    *held = refreshed;
    if let Some(candidate) = candidate {
        held.push(candidate);
        return Ok(held.len() - 1);
    }

    let (float, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let float = ncl_object::make_double(ctx, runtime, 0.0)?.as_word();
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((float, refreshed))
    })?;
    *held = refreshed;
    let float_index = held.len();
    held.push(float);

    let float_match = ncl_object::with_roots(ctx, held, |ctx, roots| {
        let type_word = roots
            .get(type_spec)
            .map(|root| **root)
            .ok_or(ObjectError::TypeError)?;
        let specifier =
            ncl_types::parse_type_specifier(ctx, type_word).map_err(|_| ObjectError::TypeError)?;
        let value = roots
            .get(float_index)
            .map(|root| **root)
            .ok_or(ObjectError::TypeError)?;
        let matches =
            ncl_types::typep(ctx, value, &specifier).map_err(|_| ObjectError::TypeError)?;
        let refreshed = roots.iter().map(|root| **root).collect::<Vec<_>>();
        Ok((matches, refreshed))
    });
    let (matches, refreshed) = match float_match {
        Ok(result) => result,
        Err(ObjectError::TypeError) => return Err(unsupported_type_default(ctx, runtime)),
        Err(error) => return Err(error),
    };
    *held = refreshed;
    if matches {
        Ok(float_index)
    } else {
        Err(unsupported_type_default(ctx, runtime))
    }
}
