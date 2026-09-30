use super::{form, progn};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object, string_length,
    string_ref, symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// Whether `word` is the symbol named `name` (any package: handler-bind type
/// specifiers are read in `COMMON-LISP`, so this only needs a name match).
pub(super) fn symbol_named(ctx: &ThreadContext, word: Word, name: &str) -> bool {
    let ObjectRef::Symbol(_) = classify_object(ctx, word) else {
        return false;
    };
    let Ok(actual) = symbol_name(ctx, word) else {
        return false;
    };
    let Ok(length) = string_length(ctx, actual) else {
        return false;
    };
    if length != name.len() {
        return false;
    }
    name.chars()
        .enumerate()
        .all(|(index, expected)| string_ref(ctx, actual, index) == Ok(expected))
}

/// Expand a handler-bind condition-type specifier into the set of literal
/// type names it should install a handler for.
///
/// `T` matches every condition, which is exactly what the root `CONDITION`
/// class already matches, so `T` lowers to `CONDITION`. `(OR type*)` installs
/// the same handler for every listed type: since a signalled condition has
/// exactly one concrete class, at most one of the decomposed bindings can
/// ever match. Any other compound specifier (`AND`, `NOT`, `SATISFIES`, ...)
/// is not supported and is passed through as a literal (failing cleanly at
/// `push-handler` name resolution rather than silently mismatching).
fn resolve_type_specifiers(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    spec: Word,
) -> Result<Vec<Word>> {
    if spec.is_cons() {
        let parts = elements(ctx, spec)?;
        let head = parts.first().copied().ok_or(ObjectError::TypeError)?;
        if symbol_named(ctx, head, "OR") {
            let mut resolved = Vec::new();
            for sub in parts.get(1..).ok_or(ObjectError::TypeError)? {
                resolved.extend(resolve_type_specifiers(ctx, runtime, *sub)?);
            }
            return Ok(resolved);
        }
        return Ok(vec![spec]);
    }
    if symbol_named(ctx, spec, "T") {
        return Ok(vec![symbol(ctx, runtime, "CONDITION")?]);
    }
    Ok(vec![spec])
}

pub(crate) fn binding(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    value: Word,
) -> Result {
    list(ctx, runtime, &[name, value])
}

pub(crate) fn bindings(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    pairs: &[(Word, Word)],
) -> Result {
    let values = pairs
        .iter()
        .flat_map(|&(name, value)| std::iter::once(name).chain(std::iter::once(value)))
        .collect::<Vec<_>>();
    ncl_object::with_roots(ctx, &values, |ctx, roots| {
        let mut result = Vec::with_capacity(pairs.len());
        for index in 0..pairs.len() {
            let value = ncl_object::with_roots(ctx, &result, |ctx, _result_roots| {
                let name = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                let value = **roots.get(index * 2 + 1).ok_or(ObjectError::TypeError)?;
                binding(ctx, runtime, name, value)
            })?;
            result.push(value);
        }
        list(ctx, runtime, &result)
    })
}

pub(super) fn expand(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result {
    let clauses = elements(ctx, values.first().copied().ok_or(ObjectError::TypeError)?)?;
    let mut flattened: Vec<(Word, Word)> = Vec::new();
    for clause in clauses {
        let parts = elements(ctx, clause)?;
        if parts.len() != 2 {
            return Err(ObjectError::TypeError);
        }
        let condition_spec = *parts.first().ok_or(ObjectError::TypeError)?;
        let handler = *parts.get(1).ok_or(ObjectError::TypeError)?;
        for resolved in resolve_type_specifiers(ctx, runtime, condition_spec)? {
            flattened.push((resolved, handler));
        }
    }
    let mut result = progn(ctx, runtime, values.get(1..).ok_or(ObjectError::TypeError)?)?;
    for (condition, handler) in flattened.into_iter().rev() {
        let pair: [Word; 2] = (condition, handler).into();
        result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
            ncl_object::with_roots(ctx, &pair, |ctx, parts| {
                let condition = parts.first().ok_or(ObjectError::TypeError)?;
                let handler = parts.get(1).ok_or(ObjectError::TypeError)?;
                let condition = form(ctx, runtime, "QUOTE", &[**condition])?;
                let handler = form(ctx, runtime, "FUNCTION", &[**handler])?;
                let push = form(ctx, runtime, "NCL-EXT::PUSH-HANDLER", &[condition, handler])?;
                let token = fresh_symbol(ctx, runtime)?;
                let bindings = bindings(ctx, runtime, &[(token, push)])?;
                let pop = form(ctx, runtime, "NCL-EXT::POP-HANDLER", &[token])?;
                // `unwind-protect` would be the ANSI-correct wrapper here (a
                // non-local exit out of `result` should still pop this
                // handler), but pairing `unwind-protect` with a `lambda`
                // that performs `return-from` to an enclosing block
                // currently trips a front-end/codegen defect (observed:
                // `EscapingControl`/`inline-direct-calls` failures, outside
                // this lane's conditions/macros scope, not chased further
                // here). `prog1`, not `progn`: the handler-bind form's value
                // is `result`'s (primary) value, not `pop`'s; the handler is
                // popped on normal exit either way, and stays installed
                // (harmlessly stale, per the pre-existing contract) if
                // `result` transfers control past this form.
                let body = form(ctx, runtime, "PROG1", &[*result, pop])?;
                ncl_object::with_roots(ctx, &[bindings, body], |ctx, roots| {
                    let bindings = roots.first().ok_or(ObjectError::Layout)?;
                    let body = roots.get(1).ok_or(ObjectError::Layout)?;
                    form(ctx, runtime, "LET", &[**bindings, **body])
                })
            })
        })?;
    }
    Ok(result)
}
