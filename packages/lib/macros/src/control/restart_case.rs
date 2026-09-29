//! Structural expansion of `RESTART-CASE`.
//!
//! The expansion deliberately targets `RESTART-BIND`.  The conditions crate
//! currently records a pending restart exit, but does not provide a second
//! runtime entry point for calling a generated restart function.  Keeping the
//! transfer in the generated `TAGBODY` therefore leaves that runtime boundary
//! explicit instead of inventing an extension builtin here.

use super::{args, form};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, string_length, string_ref, symbol_name, symbol_package,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

#[derive(Clone, Copy)]
enum OptionKind {
    Interactive,
    Report,
    Test,
}

#[derive(Clone, Copy)]
struct OptionSpec {
    kind: OptionKind,
    value: Word,
}

fn symbol_named(ctx: &ThreadContext, word: Word, expected: &str) -> Result<bool> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Ok(false);
    };
    let name = symbol_name(ctx, symbol)?;
    if string_length(ctx, name)? != expected.chars().count() {
        return Ok(false);
    }
    expected
        .chars()
        .enumerate()
        .map(|(index, character)| Ok(string_ref(ctx, name, index)? == character))
        .collect::<Result<Vec<_>>>()
        .map(|matches| matches.into_iter().all(|matched| matched))
}

fn keyword_kind(ctx: &ThreadContext, runtime: &Runtime, word: Word) -> Result<Option<OptionKind>> {
    let ObjectRef::Symbol(symbol) = classify_object(ctx, word) else {
        return Ok(None);
    };
    let Some(keyword_package) = runtime.find_package(ctx, "KEYWORD") else {
        return Ok(None);
    };
    if symbol_package(ctx, symbol)? != keyword_package {
        return Ok(None);
    }
    if symbol_named(ctx, word, "INTERACTIVE")? {
        return Ok(Some(OptionKind::Interactive));
    }
    if symbol_named(ctx, word, "REPORT")? {
        return Ok(Some(OptionKind::Report));
    }
    if symbol_named(ctx, word, "TEST")? {
        return Ok(Some(OptionKind::Test));
    }
    Ok(None)
}

fn bind_keyword(ctx: &mut ThreadContext, runtime: &Runtime, kind: OptionKind) -> Result<Word> {
    let name = match kind {
        OptionKind::Interactive => "KEYWORD::INTERACTIVE-FUNCTION",
        OptionKind::Report => "KEYWORD::REPORT-FUNCTION",
        OptionKind::Test => "KEYWORD::TEST-FUNCTION",
    };
    symbol(ctx, runtime, name)
}

fn valid_lambda_form(ctx: &mut ThreadContext, value: Word) -> Result<bool> {
    let parts = match elements(ctx, value) {
        Ok(parts) => parts,
        Err(ObjectError::TypeError) => return Ok(false),
        Err(error) => return Err(error),
    };
    let Some(head) = parts.first().copied() else {
        return Ok(false);
    };
    let Some(lambda_list) = parts.get(1).copied() else {
        return Ok(false);
    };
    Ok(symbol_named(ctx, head, "LAMBDA")? && elements(ctx, lambda_list).is_ok())
}

fn is_function_parts(ctx: &ThreadContext, parts: &[Word]) -> Result<bool> {
    if parts.len() != 2 {
        return Ok(false);
    }
    symbol_named(
        ctx,
        parts.first().copied().ok_or(ObjectError::TypeError)?,
        "FUNCTION",
    )
}

fn function_form(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word> {
    if matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
        return form(ctx, runtime, "FUNCTION", &[value]);
    }
    let parts = elements(ctx, value)?;
    if valid_lambda_form(ctx, value)? {
        return form(ctx, runtime, "FUNCTION", &[value]);
    }
    if parts.len() == 2
        && symbol_named(
            ctx,
            parts.first().copied().ok_or(ObjectError::TypeError)?,
            "FUNCTION",
        )?
    {
        let designator = *parts.get(1).ok_or(ObjectError::TypeError)?;
        if matches!(classify_object(ctx, designator), ObjectRef::Symbol(_))
            || valid_lambda_form(ctx, designator)?
        {
            return Ok(value);
        }
    }
    Err(ObjectError::TypeError)
}

fn report_function(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word> {
    if !matches!(classify_object(ctx, value), ObjectRef::String(_)) {
        return function_form(ctx, runtime, value);
    }
    let stream = fresh_symbol(ctx, runtime)?;
    ncl_object::with_root(ctx, &mut stream.clone(), |ctx, stream| {
        let lambda_list = list(ctx, runtime, &[*stream])?;
        ncl_object::with_root(ctx, &mut lambda_list.clone(), |ctx, lambda_list| {
            let body = form(ctx, runtime, "WRITE-STRING", &[value, *stream])?;
            ncl_object::with_root(ctx, &mut body.clone(), |ctx, body| {
                let lambda = form(ctx, runtime, "LAMBDA", &[*lambda_list, *body])?;
                ncl_object::with_root(ctx, &mut lambda.clone(), |ctx, lambda| {
                    form(ctx, runtime, "FUNCTION", &[*lambda])
                })
            })
        })
    })
}

fn option_value(ctx: &mut ThreadContext, runtime: &Runtime, option: OptionSpec) -> Result<Word> {
    match option.kind {
        OptionKind::Report => report_function(ctx, runtime, option.value),
        OptionKind::Interactive | OptionKind::Test => function_form(ctx, runtime, option.value),
    }
}

fn parse_clause(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    parts: &[Word],
) -> Result<(Word, Vec<OptionSpec>, usize)> {
    let name = *parts.first().ok_or(ObjectError::TypeError)?;
    let lambda_list = *parts.get(1).ok_or(ObjectError::TypeError)?;
    if !matches!(classify_object(ctx, name), ObjectRef::Symbol(_)) {
        return Err(ObjectError::TypeError);
    }
    elements(ctx, lambda_list)?;
    let mut options: Vec<OptionSpec> = Vec::new();
    let mut cursor = 2;
    while let Some(keyword) = parts.get(cursor).copied() {
        let Some(kind) = keyword_kind(ctx, runtime, keyword)? else {
            break;
        };
        if options
            .iter()
            .any(|option| std::mem::discriminant(&option.kind) == std::mem::discriminant(&kind))
        {
            return Err(ObjectError::TypeError);
        }
        let value = parts
            .get(cursor + 1)
            .copied()
            .ok_or(ObjectError::TypeError)?;
        let function_parts = elements(ctx, value).ok();
        let is_function_form = match function_parts.as_deref() {
            Some(parts) => is_function_parts(ctx, parts)?,
            None => false,
        };
        if !matches!(kind, OptionKind::Report)
            && !matches!(classify_object(ctx, value), ObjectRef::Symbol(_))
            && !valid_lambda_form(ctx, value)?
            && !is_function_form
        {
            return Err(ObjectError::TypeError);
        }
        if matches!(kind, OptionKind::Report)
            && !matches!(classify_object(ctx, value), ObjectRef::String(_))
            && !matches!(classify_object(ctx, value), ObjectRef::Symbol(_))
            && !valid_lambda_form(ctx, value)?
            && !is_function_form
        {
            return Err(ObjectError::TypeError);
        }
        options.push(OptionSpec { kind, value });
        cursor += 2;
    }
    Ok((name, options, cursor))
}

#[allow(clippy::too_many_lines)]
fn expand_clause(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    clause: Word,
    arguments: Word,
    clause_tag: Word,
) -> Result<(Word, Word)> {
    let parts = elements(ctx, clause)?;
    ncl_object::with_roots(ctx, &parts, |ctx, roots| {
        let parts = roots.iter().map(|root| **root).collect::<Vec<_>>();
        let (name, options, body_start) = parse_clause(ctx, runtime, &parts)?;
        ncl_object::with_roots(ctx, &[arguments, clause_tag], |ctx, roots| {
            let arguments = **roots.first().ok_or(ObjectError::Layout)?;
            let clause_tag = **roots.get(1).ok_or(ObjectError::Layout)?;
            let rest_variable = fresh_symbol(ctx, runtime)?;
            ncl_object::with_root(ctx, &mut rest_variable.clone(), |ctx, rest_variable| {
                let rest = symbol(ctx, runtime, "&REST")?;
                let restart_lambda_list = list(ctx, runtime, &[rest, *rest_variable])?;
                ncl_object::with_root(
                    ctx,
                    &mut restart_lambda_list.clone(),
                    |ctx, restart_lambda_list| {
                        let set_arguments =
                            form(ctx, runtime, "SETQ", &[arguments, *rest_variable])?;
                        ncl_object::with_root(
                            ctx,
                            &mut set_arguments.clone(),
                            |ctx, set_arguments| {
                                let go = form(ctx, runtime, "GO", &[clause_tag])?;
                                ncl_object::with_root(ctx, &mut go.clone(), |ctx, go| {
                                    let restart_lambda = form(
                                        ctx,
                                        runtime,
                                        "LAMBDA",
                                        &[*restart_lambda_list, *set_arguments, *go],
                                    )?;
                                    ncl_object::with_root(
                                        ctx,
                                        &mut restart_lambda.clone(),
                                        |ctx, restart_lambda| {
                                            let restart_function =
                                                form(ctx, runtime, "FUNCTION", &[*restart_lambda])?;
                                            ncl_object::with_root(
                                                ctx,
                                                &mut restart_function.clone(),
                                                |ctx, restart_function| {
                                                    let mut body_values = Vec::with_capacity(
                                                        parts.len() - body_start + 1,
                                                    );
                                                    body_values.push(
                                                        *parts.get(1).ok_or(ObjectError::Layout)?,
                                                    );
                                                    body_values.extend_from_slice(
                                                        parts
                                                            .get(body_start..)
                                                            .ok_or(ObjectError::Layout)?,
                                                    );
                                                    let body_lambda =
                                                        form(ctx, runtime, "LAMBDA", &body_values)?;
                                                    ncl_object::with_root(
                                                        ctx,
                                                        &mut body_lambda.clone(),
                                                        |ctx, body_lambda| {
                                                            let body_function = form(
                                                                ctx,
                                                                runtime,
                                                                "FUNCTION",
                                                                &[*body_lambda],
                                                            )?;
                                                            ncl_object::with_root(
                                                                ctx,
                                                                &mut body_function.clone(),
                                                                |ctx, body_function| {
                                                                    let option_words = vec![
                                                                        Word::NIL;
                                                                        options.len() * 2
                                                                    ];
                                                                    ncl_object::with_rooted_slice(
                                                                        ctx,
                                                                        &option_words,
                                                                        |ctx, option_words| {
                                                                            for (index, option) in
                                                                                options
                                                                                    .iter()
                                                                                    .enumerate()
                                                                            {
                                                                                *option_words
                                                                                    .get_mut(index * 2)
                                                                                    .ok_or(ObjectError::Layout)? = bind_keyword(
                                                                                        ctx,
                                                                                        runtime,
                                                                                        option.kind,
                                                                                    )?;
                                                                                *option_words
                                                                                    .get_mut(index * 2 + 1)
                                                                                    .ok_or(ObjectError::Layout)? = option_value(
                                                                                        ctx,
                                                                                        runtime,
                                                                                        *option,
                                                                                    )?;
                                                                            }
                                                                            let mut clause_values = vec![
                                                                                name,
                                                                                *restart_function,
                                                                            ];
                                                                            clause_values
                                                                                .extend_from_slice(
                                                                                    option_words,
                                                                                );
                                                                            let restart_clause =
                                                                                list(
                                                                                    ctx,
                                                                                    runtime,
                                                                                    &clause_values,
                                                                                )?;
                                                                            Ok((
                                                                                restart_clause,
                                                                                *body_function,
                                                                            ))
                                                                        },
                                                                    )
                                                                },
                                                            )
                                                        },
                                                    )
                                                },
                                            )
                                        },
                                    )
                                })
                            },
                        )
                    },
                )
            })
        })
    })
}

/// Expand the arguments of `RESTART-CASE` into a `RESTART-BIND` transfer.
#[allow(clippy::too_many_lines)]
pub(crate) fn expand_restart_case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    if values.is_empty() {
        return Err(ObjectError::TypeError);
    }
    let clauses = values.get(1..).ok_or(ObjectError::TypeError)?;
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let protected = **roots.first().ok_or(ObjectError::Layout)?;
        let block_tag = fresh_symbol(ctx, runtime)?;
        ncl_object::with_root(ctx, &mut block_tag.clone(), |ctx, block_tag| {
            let arguments = fresh_symbol(ctx, runtime)?;
            ncl_object::with_root(ctx, &mut arguments.clone(), |ctx, arguments| {
                let clause_tags = vec![Word::NIL; clauses.len()];
                ncl_object::with_rooted_slice(ctx, &clause_tags, |ctx, clause_tags| {
                    for tag in clause_tags.iter_mut() {
                        *tag = fresh_symbol(ctx, runtime)?;
                    }
                    let restart_clauses = vec![Word::NIL; clauses.len()];
                    let body_functions = vec![Word::NIL; clauses.len()];
                    ncl_object::with_rooted_slice(ctx, &restart_clauses, |ctx, restart_clauses| {
                        ncl_object::with_rooted_slice(
                            ctx,
                            &body_functions,
                            |ctx, body_functions| {
                                for (index, _clause) in clauses.iter().enumerate() {
                                    let (restart_clause, body_function) = expand_clause(
                                        ctx,
                                        runtime,
                                        **roots.get(index + 1).ok_or(ObjectError::Layout)?,
                                        *arguments,
                                        *clause_tags.get(index).ok_or(ObjectError::Layout)?,
                                    )?;
                                    *restart_clauses.get_mut(index).ok_or(ObjectError::Layout)? =
                                        restart_clause;
                                    *body_functions.get_mut(index).ok_or(ObjectError::Layout)? =
                                        body_function;
                                }
                                let restart_clause_list = list(ctx, runtime, restart_clauses)?;
                                ncl_object::with_root(
                                    ctx,
                                    &mut restart_clause_list.clone(),
                                    |ctx, restart_clause_list| {
                                        let normal = form(
                                            ctx,
                                            runtime,
                                            "RETURN-FROM",
                                            &[*block_tag, protected],
                                        )?;
                                        ncl_object::with_root(
                                            ctx,
                                            &mut normal.clone(),
                                            |ctx, normal| {
                                                let restart_bind = form(
                                                    ctx,
                                                    runtime,
                                                    "RESTART-BIND",
                                                    &[*restart_clause_list, *normal],
                                                )?;
                                                ncl_object::with_root(
                                                    ctx,
                                                    &mut restart_bind.clone(),
                                                    |ctx, restart_bind| {
                                                        let dispatch =
                                                            vec![Word::NIL; clauses.len()];
                                                        ncl_object::with_rooted_slice(
                                                            ctx,
                                                            &dispatch,
                                                            |ctx, dispatch| {
                                                                for (index, dispatch_form) in
                                                                    dispatch.iter_mut().enumerate()
                                                                {
                                                                    *dispatch_form = form(
                                                                        ctx,
                                                                        runtime,
                                                                        "APPLY",
                                                                        &[
                                                                            *body_functions.get(index).ok_or(ObjectError::Layout)?,
                                                                            *arguments,
                                                                        ],
                                                                    )?;
                                                                }
                                                                let mut tagbody =
                                                                    Vec::with_capacity(
                                                                        clauses.len() * 2 + 1,
                                                                    );
                                                                tagbody
                                                                    .extend_from_slice(clause_tags);
                                                                tagbody.push(*restart_bind);
                                                                for dispatch_form in dispatch {
                                                                    tagbody.push(*dispatch_form);
                                                                }
                                                                let tagbody = form(
                                                                    ctx, runtime, "TAGBODY",
                                                                    &tagbody,
                                                                )?;
                                                                ncl_object::with_root(
                                                                    ctx,
                                                                    &mut tagbody.clone(),
                                                                    |ctx, tagbody| {
                                                                        form(
                                                                            ctx,
                                                                            runtime,
                                                                            "BLOCK",
                                                                            &[*block_tag, *tagbody],
                                                                        )
                                                                    },
                                                                )
                                                            },
                                                        )
                                                    },
                                                )
                                            },
                                        )
                                    },
                                )
                            },
                        )
                    })
                })
            })
        })
    })
}

pub(crate) fn expand_restart_case_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let arguments = args(ctx, form)?;
    expand_restart_case(ctx, runtime, &arguments)
}
