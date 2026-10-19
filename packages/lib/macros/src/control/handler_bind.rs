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
                let name = roots.get(index * 2).ok_or(ObjectError::TypeError)?.get();
                let value = roots
                    .get(index * 2 + 1)
                    .ok_or(ObjectError::TypeError)?
                    .get();
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
                let condition = form(ctx, runtime, "QUOTE", &[condition.get()])?;
                let push = form(
                    ctx,
                    runtime,
                    "NCL-EXT::PUSH-HANDLER",
                    &[condition, handler.get()],
                )?;
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
                let body = form(ctx, runtime, "PROG1", &[result.get(), pop])?;
                ncl_object::with_roots(ctx, &[bindings, body], |ctx, roots| {
                    let bindings = roots.first().ok_or(ObjectError::Layout)?;
                    let body = roots.get(1).ok_or(ObjectError::Layout)?;
                    form(ctx, runtime, "LET", &[bindings.get(), body.get()])
                })
            })
        })?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::form as control_form;
    use super::expand;
    use crate::{elements, list, symbol};
    use ncl_object::{
        FunctionObject, ObjectError, ObjectRef, Package, Runtime, ThreadContext, Word,
        classify_object,
    };

    fn count_form(ctx: &mut ThreadContext, tree: Word, form: Word) -> Result<usize, ObjectError> {
        if tree == form {
            return Ok(1);
        }
        if !matches!(classify_object(ctx, tree), ObjectRef::Cons(_)) {
            return Ok(0);
        }
        let mut count = 0;
        for child in elements(ctx, tree)? {
            count += count_form(ctx, child, form)?;
        }
        Ok(count)
    }

    #[test]
    fn handler_bind_preserves_function_designator_expression() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let function = symbol(&mut ctx, &runtime, "FUNCTION")?;
        let lambda = symbol(&mut ctx, &runtime, "LAMBDA")?;
        let parameter = symbol(&mut ctx, &runtime, "CONDITION")?;
        let warning = symbol(&mut ctx, &runtime, "WARNING")?;
        let function_body = list(&mut ctx, &runtime, &[parameter])?;
        let lambda_list = list(&mut ctx, &runtime, &[parameter])?;
        let lambda_form = list(&mut ctx, &runtime, &[lambda, lambda_list, function_body])?;
        let handler = list(&mut ctx, &runtime, &[function, lambda_form])?;
        let clause = list(&mut ctx, &runtime, &[warning, handler])?;
        let clauses = list(&mut ctx, &runtime, &[clause])?;

        let expansion = expand(&mut ctx, &runtime, &[clauses])?;

        assert_eq!(count_form(&mut ctx, expansion, function)?, 1);
        Ok(())
    }

    #[test]
    fn handler_bind_accepts_an_empty_clause_list() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;

        let expansion = expand(&mut ctx, &runtime, &[Word::NIL, Word::fixnum(1)])?;

        let progn = symbol(&mut ctx, &runtime, "PROGN")?;
        assert_eq!(elements(&mut ctx, expansion)?, [progn, Word::fixnum(1)]);
        Ok(())
    }

    #[test]
    fn handler_bind_empty_clause_list_passes_through_adapter() -> Result<(), ObjectError> {
        let runtime = Runtime::new()?;
        crate::register(&runtime)?;
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)?;
        let package = runtime
            .find_package(&ctx, "COMMON-LISP")
            .ok_or(ObjectError::TypeError)?;
        let operator = Package::from_word(package)
            .intern(&mut ctx, &runtime, "HANDLER-BIND")?
            .0;
        let function = FunctionObject::try_from(ncl_object::symbol_function(&ctx, operator)?)?;
        let form = control_form(
            &mut ctx,
            &runtime,
            "HANDLER-BIND",
            &[Word::NIL, Word::fixnum(1)],
        )?;
        let expansion = runtime.call_builtin(&mut ctx, function, &[form])?;
        let progn = symbol(&mut ctx, &runtime, "PROGN")?;
        assert_eq!(elements(&mut ctx, expansion)?, [progn, Word::fixnum(1)]);
        Ok(())
    }
}
