fn clos_call_next_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let next_methods = scope.root(Local::from_word(args.required(0)?));
    let supplied = scope.root(Local::from_word(args.required(1)?));
    let argument_list = supplied;
    if scope.get(next_methods).as_word() == Word::NIL {
        return Err(ObjectError::UndefinedFunction);
    }
    let arguments = dispatch_list_to_handles(&mut scope, argument_list)?;
    let mut caller = BuiltinFunctionCaller;
    let result = invoke_continuation(
        &mut scope,
        runtime,
        next_methods,
        &arguments,
        argument_list,
        &mut caller,
        values,
    )?;
    Ok(scope.get(result).as_word())
}

fn clos_next_method_p_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(if args.required(0)? == Word::NIL {
        Word::NIL
    } else {
        Word::TRUE
    })
}

fn class_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
) -> Result<Word, ObjectError> {
    if matches!(classify_object(ctx, value), ObjectRef::SimpleVector(_)) {
        return Ok(value);
    }
    if matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
        let name = symbol_name_string(ctx, value)?;
        let qualified_name = runtime.structure_class_name(ctx, value).ok();
        return qualified_name
            .as_deref()
            .and_then(|qualified| runtime.class(ctx, qualified))
            .or_else(|| runtime.class(ctx, &name))
            .filter(|class| *class != Word::UNBOUND) // check-added-lines: allow(unbound) sentinel check
            .ok_or(ObjectError::TypeError);
    }
    Err(ObjectError::TypeError)
}

fn find_class_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_designator(ctx, runtime, args.required(0)?)
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "test setup")]
mod tests {
    include!("dispatch_tests.rs");
}
