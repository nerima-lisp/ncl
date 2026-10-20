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
    if value == Word::TRUE || value == Word::NIL {
        let name = if value == Word::TRUE { "T" } else { "NULL" };
        return runtime
            .class(ctx, name)
            .filter(|class| !is_class_placeholder(*class))
            .ok_or(ObjectError::TypeError);
    }
    if matches!(classify_object(ctx, value), ObjectRef::SimpleVector(_)) {
        return if is_class_metaobject(ctx, runtime, value)? {
            Ok(value)
        } else {
            Err(ObjectError::TypeError)
        };
    }
    if matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
        let name = symbol_name_string(ctx, value)?;
        let qualified_name = runtime.structure_class_name(ctx, value).ok();
        let package = Package::from_word(ncl_object::symbol_package(ctx, value)?).name(ctx)?;
        let package_name = string_word_to_string(ctx, package)?;
        let package_qualified_name = format!("{package_name}::{name}");
        return qualified_name
            .as_deref()
            .and_then(|qualified| runtime.class(ctx, qualified))
            .or_else(|| runtime.class(ctx, &package_qualified_name))
            .or_else(|| runtime.class(ctx, &name))
            .filter(|class| !is_class_placeholder(*class)) // check-added-lines: allow(unbound) sentinel check
            .ok_or(ObjectError::TypeError);
    }
    Err(ObjectError::TypeError)
}

fn is_class_placeholder(class: Word) -> bool {
    class == Word::UNBOUND || class == Word::fixnum(1) // check-added-lines: allow(unbound) class registry placeholders
}

fn string_word_to_string(ctx: &ThreadContext, word: Word) -> Result<String, ObjectError> {
    let length = ncl_object::string_length(ctx, word)?;
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, word, index))
        .collect()
}

fn find_class_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    if name != Word::TRUE
        && name != Word::NIL
        && !matches!(classify_object(ctx, name), ObjectRef::Symbol(_))
    {
        ctx.set_pending_lisp_error(ncl_object::LispError::TypeError {
            datum: name,
            expected: ObjectType::Symbol,
        });
        return Err(ObjectError::TypeError);
    }
    class_designator(ctx, runtime, name)
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "test setup")]
mod tests {
    include!("dispatch_tests.rs");
}
