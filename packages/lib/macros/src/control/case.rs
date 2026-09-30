//! `case`/`ecase`/`ccase` (CLHS 7.6, "Data and Control Flow").

use super::{bindings, form, is_otherwise, or, progn};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// Expand `case`, and, when `errorp` (`ecase`/`ccase`), signal a
/// `type-error` naming the accepted keys as a `(member ...)` type instead of
/// silently returning `nil` when nothing matches.
pub(super) fn case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
    errorp: bool,
) -> Result {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let value = roots.first().copied().ok_or(ObjectError::TypeError)?;
        let mut temporary = fresh_symbol(ctx, runtime)?;
        ncl_object::with_root(ctx, &mut temporary, |ctx, temporary| {
            let mut let_bindings = bindings(ctx, runtime, &[(*temporary, *value)])?;
            ncl_object::with_root(ctx, &mut let_bindings, |ctx, let_bindings| {
                // Pre-scan clauses to collect every key named, so a `case`
                // compiled with `errorp` can report the accepted values as a
                // `(member ...)` expected type when nothing matches. This
                // pass performs no allocation, so it is safe to read `roots`
                // directly without copying them out of the root table first.
                let mut all_keys: Vec<Word> = Vec::new();
                let mut has_otherwise = false;
                for clause in roots.get(1..).ok_or(ObjectError::TypeError)? {
                    let parts = elements(ctx, **clause)?;
                    let keys = parts.first().copied().ok_or(ObjectError::TypeError)?;
                    if is_otherwise(ctx, keys)? {
                        has_otherwise = true;
                        continue;
                    }
                    if keys.is_cons() {
                        all_keys.extend(elements(ctx, keys)?);
                    } else {
                        all_keys.push(keys);
                    }
                }
                let mut branches = if errorp && !has_otherwise {
                    ncl_object::with_roots(ctx, &all_keys, |ctx, all_keys| {
                        let key_values = all_keys.iter().map(|key| **key).collect::<Vec<_>>();
                        let mut expected_types = vec![symbol(ctx, runtime, "MEMBER")?];
                        expected_types.extend(key_values);
                        let expected_type = list(ctx, runtime, &expected_types)?;
                        let expected = form(ctx, runtime, "QUOTE", &[expected_type])?;
                        let type_error = symbol(ctx, runtime, "TYPE-ERROR")?;
                        let error_type = form(ctx, runtime, "QUOTE", &[type_error])?;
                        let datum = symbol(ctx, runtime, ":DATUM")?;
                        let expected_type_kw = symbol(ctx, runtime, ":EXPECTED-TYPE")?;
                        form(
                            ctx,
                            runtime,
                            "ERROR",
                            &[error_type, datum, *temporary, expected_type_kw, expected],
                        )
                    })?
                } else {
                    Word::NIL
                };
                for clause in roots.get(1..).ok_or(ObjectError::TypeError)?.iter().rev() {
                    let parts = elements(ctx, **clause)?;
                    ncl_object::with_roots(ctx, &parts, |ctx, parts| {
                        let keys = parts.first().copied().ok_or(ObjectError::TypeError)?;
                        let body_values = parts
                            .get(1..)
                            .ok_or(ObjectError::TypeError)?
                            .iter()
                            .map(|part| **part)
                            .collect::<Vec<_>>();
                        let mut body = progn(ctx, runtime, &body_values)?;
                        ncl_object::with_root(ctx, &mut body, |ctx, body| {
                            if is_otherwise(ctx, *keys)? {
                                let true_symbol = symbol(ctx, runtime, "T")?;
                                branches =
                                    ncl_object::with_root(ctx, &mut branches, |ctx, branches| {
                                        form(ctx, runtime, "IF", &[true_symbol, *body, *branches])
                                    })?;
                                return Ok(());
                            }
                            let key_values = if keys.is_cons() {
                                elements(ctx, *keys)?
                            } else {
                                vec![*keys]
                            };
                            ncl_object::with_roots(ctx, &key_values, |ctx, key_values| {
                                let mut tests = Vec::with_capacity(key_values.len());
                                for key in key_values {
                                    let quote = form(ctx, runtime, "QUOTE", &[**key])?;
                                    tests.push(form(ctx, runtime, "EQL", &[*temporary, quote])?);
                                }
                                ncl_object::with_roots(ctx, &tests, |ctx, tests| {
                                    let test_values =
                                        tests.iter().map(|test| **test).collect::<Vec<_>>();
                                    let mut test = or(ctx, runtime, &test_values)?;
                                    ncl_object::with_root(ctx, &mut test, |ctx, test| {
                                        branches = ncl_object::with_root(
                                            ctx,
                                            &mut branches,
                                            |ctx, branches| {
                                                form(ctx, runtime, "IF", &[*test, *body, *branches])
                                            },
                                        )?;
                                        Ok(())
                                    })
                                })
                            })
                        })
                    })?;
                }
                ncl_object::with_root(ctx, &mut branches, |ctx, branches| {
                    form(ctx, runtime, "LET", &[*let_bindings, *branches])
                })
            })
        })
    })
}
