use super::{form, progn};
use crate::{elements, fresh_symbol, list, symbol};
use ncl_object::{
    HeapRootedWord, HeapRootedWords, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    classify_object, string_length, string_ref, symbol_name,
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
) -> Result<HeapRootedWords> {
    if spec.is_cons() {
        let parts = elements(ctx, spec)?;
        let parts = HeapRootedWords::new(ctx, &parts)?;
        let head = parts.first().ok_or(ObjectError::TypeError)?;
        if symbol_named(ctx, head.get(), "OR") {
            let mut resolved = HeapRootedWords::default();
            for sub in parts.as_slice().get(1..).ok_or(ObjectError::TypeError)? {
                let nested = resolve_type_specifiers(ctx, runtime, sub.get())?;
                for resolved_type in nested.iter() {
                    resolved.push(ctx, resolved_type.get())?;
                }
            }
            return Ok(resolved);
        }
        return HeapRootedWords::new(ctx, &[spec]);
    }
    if symbol_named(ctx, spec, "T") {
        let condition = symbol(ctx, runtime, "CONDITION")?;
        return HeapRootedWords::new(ctx, &[condition]);
    }
    HeapRootedWords::new(ctx, &[spec])
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
        let mut result = HeapRootedWords::default();
        for index in 0..pairs.len() {
            let value = {
                let name = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                let value = **roots.get(index * 2 + 1).ok_or(ObjectError::TypeError)?;
                binding(ctx, runtime, name, value)
            }?;
            result.push(ctx, value)?;
        }
        let values = result.iter().map(HeapRootedWord::get).collect::<Vec<_>>();
        list(ctx, runtime, &values)
    })
}

pub(super) fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[HeapRootedWord],
) -> Result {
    let clauses = elements(ctx, values.first().ok_or(ObjectError::TypeError)?.get())?;
    let clauses = HeapRootedWords::new(ctx, &clauses)?;
    let mut flattened = HeapRootedWords::default();
    for clause in clauses.iter() {
        let parts = elements(ctx, clause.get())?;
        let parts = HeapRootedWords::new(ctx, &parts)?;
        if parts.len() != 2 {
            return Err(ObjectError::TypeError);
        }
        let condition_spec = parts.first().ok_or(ObjectError::TypeError)?;
        let handler = parts.get(1).ok_or(ObjectError::TypeError)?;
        let resolved = resolve_type_specifiers(ctx, runtime, condition_spec.get())?;
        for resolved_type in resolved.iter() {
            flattened.push(ctx, resolved_type.get())?;
            flattened.push(ctx, handler.get())?;
        }
    }
    let body_values = values
        .get(1..)
        .ok_or(ObjectError::TypeError)?
        .iter()
        .map(HeapRootedWord::get)
        .collect::<Vec<_>>();
    let result_value = progn(ctx, runtime, &body_values)?;
    let result = HeapRootedWord::new(ctx, result_value)?;
    for offset in (0..flattened.len()).step_by(2).rev() {
        let condition = flattened.get(offset).ok_or(ObjectError::Layout)?;
        let handler = flattened.get(offset + 1).ok_or(ObjectError::Layout)?;
        let quoted_condition = form(ctx, runtime, "QUOTE", &[condition.get()])?;
        let quoted_condition = HeapRootedWord::new(ctx, quoted_condition)?;
        let push = form(
            ctx,
            runtime,
            "NCL-EXT::PUSH-HANDLER",
            &[quoted_condition.get(), handler.get()],
        )?;
        let push = HeapRootedWord::new(ctx, push)?;
        let token_word = fresh_symbol(ctx, runtime)?;
        let token = HeapRootedWord::new(ctx, token_word)?;
        let bindings_word = bindings(ctx, runtime, &[(token.get(), push.get())])?;
        let bindings = HeapRootedWord::new(ctx, bindings_word)?;
        let pop = form(ctx, runtime, "NCL-EXT::POP-HANDLER", &[token.get()])?;
        let body = form(ctx, runtime, "UNWIND-PROTECT", &[result.get(), pop])?;
        result.set(form(ctx, runtime, "LET", &[bindings.get(), body])?);
    }
    Ok(result.get())
}
