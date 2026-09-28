use super::{args, form, progn};
use crate::{elements, fresh_symbol, list};
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};

type Result<T = Word> = std::result::Result<T, ObjectError>;

fn handler_clause(ctx: &mut ThreadContext, runtime: &Runtime, tag: Word, clause: Word) -> Result {
    let parts = elements(ctx, clause)?;
    let condition = parts.first().copied().ok_or(ObjectError::TypeError)?;
    let variables = elements(ctx, parts.get(1).copied().ok_or(ObjectError::TypeError)?)?;
    if variables.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    ncl_object::symbol_name(
        ctx,
        variables.first().copied().ok_or(ObjectError::TypeError)?,
    )?;
    let body = progn(ctx, runtime, parts.get(2..).ok_or(ObjectError::TypeError)?)?;
    let return_form = form(ctx, runtime, "RETURN-FROM", &[tag, body])?;
    let lambda_list = parts.get(1).copied().ok_or(ObjectError::TypeError)?;
    let lambda = form(ctx, runtime, "LAMBDA", &[lambda_list, return_form])?;
    list(ctx, runtime, &[condition, lambda])
}

fn expand_handler_case_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let protected = values.first().copied().ok_or(ObjectError::TypeError)?;
    let clauses = values.get(1..).ok_or(ObjectError::TypeError)?;
    if clauses.is_empty() {
        return Err(ObjectError::TypeError);
    }
    let tag = fresh_symbol(ctx, runtime)?;
    ncl_object::with_roots(ctx, &[tag, protected], |ctx, roots| {
        let tag = **roots.first().ok_or(ObjectError::Layout)?;
        let protected = **roots.get(1).ok_or(ObjectError::Layout)?;
        let mut expanded_clauses = Vec::with_capacity(clauses.len());
        for clause in clauses {
            let expanded = handler_clause(ctx, runtime, tag, *clause)?;
            expanded_clauses.push(expanded);
        }
        ncl_object::with_roots(ctx, &expanded_clauses, |ctx, roots| {
            let clause_words = roots.iter().map(|root| **root).collect::<Vec<_>>();
            let clause_list = list(ctx, runtime, &clause_words)?;
            ncl_object::with_roots(ctx, &[clause_list], |ctx, roots| {
                let clauses = **roots.first().ok_or(ObjectError::Layout)?;
                let handlers = form(ctx, runtime, "HANDLER-BIND", &[clauses, protected])?;
                form(ctx, runtime, "BLOCK", &[tag, handlers])
            })
        })
    })
}

pub(crate) fn expand_handler_case(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    expand_handler_case_values(ctx, runtime, values)
}

pub(crate) fn expand_ignore_errors(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    let protected = progn(ctx, runtime, values)?;
    let tag = fresh_symbol(ctx, runtime)?;
    let condition = crate::symbol(ctx, runtime, "ERROR")?;
    let variable = fresh_symbol(ctx, runtime)?;
    let nil = Word::NIL;
    let values_form = form(ctx, runtime, "VALUES", &[nil, variable])?;
    let return_form = form(ctx, runtime, "RETURN-FROM", &[tag, values_form])?;
    let lambda_list = list(ctx, runtime, &[variable])?;
    let lambda = form(ctx, runtime, "LAMBDA", &[lambda_list, return_form])?;
    let clause = list(ctx, runtime, &[condition, lambda])?;
    let clauses = list(ctx, runtime, &[clause])?;
    let handlers = form(ctx, runtime, "HANDLER-BIND", &[clauses, protected])?;
    form(ctx, runtime, "BLOCK", &[tag, handlers])
}

pub(crate) fn expand_handler_case_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &ncl_object::BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let arguments = args(ctx, form)?;
    expand_handler_case(ctx, runtime, &arguments)
}

pub(crate) fn expand_ignore_errors_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &ncl_object::BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let arguments = args(ctx, form)?;
    expand_ignore_errors(ctx, runtime, &arguments)
}
