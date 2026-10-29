//! Building a macro function's `(lambda (whole &optional environment) ...)` body (CLHS 3.4.4).

use ncl_object::{ObjectError, Runtime, ThreadContext, Word, car, cdr, make_cons};

use super::{progn_with_definition, quote};
use crate::destructuring::{
    expand_destructuring_bind, held_call, held_fresh, held_get, held_push, held_raw_list,
    held_symbol,
};
use crate::form::{list, symbol};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// Wrap an already-built `(lambda ...)` form as `(setf (accessor 'name)
/// (function lambda))`, shared by ordinary functions and macro functions
/// (whose lambda lists are built differently; see
/// [`macro_function_definition`]).
pub(super) fn finish_function_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    accessor: &str,
    lambda: Word,
) -> Result {
    ncl_object::with_roots(ctx, &[name, lambda], |ctx, roots| {
        let mut function_symbol = symbol(ctx, runtime, "FUNCTION")?;
        ncl_object::with_root(ctx, &mut function_symbol, |ctx, function_symbol| {
            let mut function = list(
                ctx,
                runtime,
                &[
                    *function_symbol,
                    roots.get(1).ok_or(ObjectError::TypeError)?.get(),
                ],
            )?;
            ncl_object::with_root(ctx, &mut function, |ctx, function| {
                let mut accessor_symbol = symbol(ctx, runtime, accessor)?;
                ncl_object::with_root(ctx, &mut accessor_symbol, |ctx, accessor_symbol| {
                    let mut quoted_name = quote(
                        ctx,
                        runtime,
                        roots.first().ok_or(ObjectError::TypeError)?.get(),
                    )?;
                    ncl_object::with_root(ctx, &mut quoted_name, |ctx, quoted_name| {
                        let mut place = list(ctx, runtime, &[*accessor_symbol, *quoted_name])?;
                        ncl_object::with_root(ctx, &mut place, |ctx, place| {
                            let mut setf_symbol = symbol(ctx, runtime, "SETF")?;
                            ncl_object::with_root(ctx, &mut setf_symbol, |ctx, setf_symbol| {
                                let operation =
                                    list(ctx, runtime, &[*setf_symbol, *place, *function])?;
                                progn_with_definition(
                                    ctx,
                                    runtime,
                                    roots.first().ok_or(ObjectError::TypeError)?.get(),
                                    operation,
                                )
                            })
                        })
                    })
                })
            })
        })
    })
}

/// Split a leading `&whole var` off a macro lambda list, per CLHS 3.4.4.
///
/// Returns the `&whole` variable (if present) and the remaining pattern.
fn strip_leading_whole(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    lambda_list: Word,
) -> std::result::Result<(Option<Word>, Word), ObjectError> {
    if !lambda_list.is_cons() {
        return Ok((None, lambda_list));
    }
    let head = car(ctx, lambda_list)?;
    if head != symbol(ctx, runtime, "&WHOLE")? {
        return Ok((None, lambda_list));
    }
    let rest = cdr(ctx, lambda_list)?;
    if !rest.is_cons() {
        return Err(ObjectError::TypeError);
    }
    let whole_variable = car(ctx, rest)?;
    let pattern = cdr(ctx, rest)?;
    Ok((Some(whole_variable), pattern))
}

fn strip_environment(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    held: &mut Vec<Word>,
    pattern: usize,
) -> Result<Option<usize>> {
    if !held_get(held, pattern)?.is_cons() {
        return Ok(None);
    }
    let environment_marker = held_symbol(ctx, runtime, held, "&ENVIRONMENT")?;
    let mut environment_variable = None;
    let mut cursor = held_push(held, held_get(held, pattern)?);
    let mut kept = Vec::new();
    while held_get(held, cursor)?.is_cons() {
        let cell = held_get(held, cursor)?;
        let field = held_push(held, car(ctx, cell)?);
        let rest = held_push(held, cdr(ctx, cell)?);
        if held_get(held, field)? == held_get(held, environment_marker)? {
            if environment_variable.is_some() {
                return Err(ObjectError::TypeError);
            }
            if !held_get(held, rest)?.is_cons() {
                return Err(ObjectError::TypeError);
            }
            let environment_cell = held_get(held, rest)?;
            environment_variable = Some(held_push(held, car(ctx, environment_cell)?));
            cursor = held_push(held, cdr(ctx, environment_cell)?);
        } else {
            kept.push(field);
            cursor = rest;
        }
    }
    let Some(environment_variable) = environment_variable else {
        return Ok(None);
    };
    let mut result = held_push(held, held_get(held, cursor)?);
    for field in kept.into_iter().rev() {
        let (pair, refreshed) = ncl_object::with_roots(ctx, held, |ctx, roots| {
            let car = roots.get(field).ok_or(ObjectError::TypeError)?.get();
            let cdr = roots.get(result).ok_or(ObjectError::TypeError)?.get();
            let pair = make_cons(ctx, runtime, car, cdr)?;
            let refreshed = roots.iter().map(|root| root.get()).collect::<Vec<_>>();
            Ok((pair, refreshed))
        })?;
        *held = refreshed;
        result = held_push(held, pair);
    }
    let replacement = held_get(held, result)?;
    *held.get_mut(pattern).ok_or(ObjectError::Layout)? = replacement;
    Ok(Some(environment_variable))
}

/// Build a macro function's `(lambda (whole &optional environment) body...)`, where `body` first
/// destructures `(cdr whole)` against `lambda_list` (see
/// [`crate::destructuring`]) and, if the pattern led with `&whole var`,
/// binds `var` to the entire call form (operator included) rather than just
/// the arguments, matching CLHS 3.4.4.
///
/// Calling the compiled function with the whole call form and its lexical
/// environment (see `ncl-runtime`'s `RuntimeMacroCaller`) replaces
/// the previous convention of spreading each argument across registers,
/// which also fixes a reported non-deterministic crash: the register/rest
/// marshalling path in `call_macro_function` did not keep arguments beyond
/// the fourth rooted consistently, which `&rest`/`&body` macros with wide
/// argument lists could trip under GC pressure.
///
/// Every `Word` this function receives is copied into `held` before any
/// allocation, and every later use re-reads it from `held` rather than
/// keeping the original binding, so a garbage collection triggered by one
/// of the intermediate `list`/`symbol` calls cannot leave a stale reference
/// (see the `held_*` helpers in [`crate::destructuring`]).
pub(super) fn macro_function_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    accessor: &str,
    lambda_list: Word,
    body: &[Word],
) -> Result {
    let lambda = macro_function_lambda(ctx, runtime, name, lambda_list, body)?;
    finish_function_definition(ctx, runtime, name, accessor, lambda)
}

/// Build the lambda used by a macro function without choosing its storage place.
pub(super) fn macro_function_lambda(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    lambda_list: Word,
    body: &[Word],
) -> Result {
    let (whole_variable, pattern) = strip_leading_whole(ctx, runtime, lambda_list)?;
    let mut held = vec![name, pattern, whole_variable.unwrap_or(Word::NIL)];
    held.extend_from_slice(body);
    let pattern_index = 1;
    let whole_variable_index = 2;
    let body_start = 3;
    let environment_variable = strip_environment(ctx, runtime, &mut held, pattern_index)?;
    let whole_form = held_fresh(ctx, runtime, &mut held)?;
    let environment_parameter = if let Some(variable) = environment_variable {
        variable
    } else {
        held_fresh(ctx, runtime, &mut held)?
    };
    let cdr_call = held_call(ctx, runtime, &mut held, "CDR", &[whole_form])?;
    let value_form = held_get(&held, cdr_call)?;
    let pattern_word = held_get(&held, pattern_index)?;
    let body_words = (body_start..body_start + body.len())
        .map(|index| held_get(&held, index))
        .collect::<std::result::Result<Vec<_>, ObjectError>>()?;
    let destructured =
        expand_destructuring_bind(ctx, runtime, pattern_word, value_form, &body_words)?;
    let destructured_index = held_push(&mut held, destructured);
    let mut binding_pairs = Vec::new();
    if whole_variable.is_some() {
        binding_pairs.push(held_raw_list(
            ctx,
            runtime,
            &mut held,
            &[whole_variable_index, whole_form],
        )?);
    }
    let inner_body = if binding_pairs.is_empty() {
        destructured_index
    } else {
        let bindings = held_raw_list(ctx, runtime, &mut held, &binding_pairs)?;
        held_call(
            ctx,
            runtime,
            &mut held,
            "LET",
            &[bindings, destructured_index],
        )?
    };
    let optional = held_symbol(ctx, runtime, &mut held, "&OPTIONAL")?;
    let params = held_raw_list(
        ctx,
        runtime,
        &mut held,
        &[whole_form, optional, environment_parameter],
    )?;
    let lambda = held_call(ctx, runtime, &mut held, "LAMBDA", &[params, inner_body])?;
    held_get(&held, lambda)
}
