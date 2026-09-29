include!("lib_dispatch_helpers.rs");

fn clos_define_generic_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let empty = scope.root(Local::from_word(Word::NIL));
    set_method_registry(&mut scope, runtime, name, empty)?;
    Ok(scope.get(name).as_word())
}

fn clos_ensure_initialization_base_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let name_word = scope.get(name).as_word();
    let name_text = symbol_name_string(scope.context(), name_word)?;
    if name_text != "INITIALIZE-INSTANCE" && name_text != "SHARED-INITIALIZE" {
        return Ok(name_word);
    }
    if has_method_registry(&mut scope, runtime, name)? {
        return Ok(name_word);
    }
    let function = symbol_function(scope.context(), name_word)?;
    if function == Word::UNBOUND { // check-added-lines: allow(unbound) sentinel check
        return Err(ObjectError::UndefinedFunction);
    }
    let variable = scope.intern(runtime, "NCL", "INSTANCE")?;
    let class = scope.intern(runtime, COMMON_LISP, "T")?;
    let specializer = {
        let mut values = scope.root_many(&[]);
        dispatch_push_handle(&mut scope, &mut values, variable);
        dispatch_push_handle(&mut scope, &mut values, class);
        scope.make_list(runtime, &values)?
    };
    let specializers = {
        let mut values = scope.root_many(&[]);
        dispatch_push_handle(&mut scope, &mut values, specializer);
        scope.make_list(runtime, &values)?
    };
    let function = scope.root(Local::from_word(function));
    let qualifier = scope.root(Local::from_word(Word::fixnum(METHOD_QUALIFIER_PRIMARY)));
    let entry = method_registry_entry(&mut scope, runtime, specializers, qualifier, function)?;
    let registry = {
        let mut values = scope.root_many(&[]);
        dispatch_push_handle(&mut scope, &mut values, entry);
        scope.make_list(runtime, &values)?
    };
    set_method_registry(&mut scope, runtime, name, registry)?;
    Ok(name_word)
}

fn clos_add_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let encoded = scope.root(Local::from_word(args.required(1)?));
    let (specializers, qualifier) = method_definition_parts(&mut scope, runtime, encoded)?;
    let function = scope.root(Local::from_word(
        FunctionObject::try_from(args.required(2)?)?.as_word(),
    ));
    let old = method_registry(&mut scope, runtime, name)?;
    let old_entries = dispatch_list_to_handles(&mut scope, old)?;
    let mut entries = scope.root_many(&[]);
    for entry in old_entries.iter().copied() {
        let fields = dispatch_list_to_handles(&mut scope, entry)?;
        let entry_specializers = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        let entry_qualifier = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
        if scope.get(entry_specializers).as_word() != scope.get(specializers).as_word()
            || scope.get(entry_qualifier).as_word() != scope.get(qualifier).as_word()
        {
            dispatch_push_handle(&mut scope, &mut entries, entry);
        }
    }
    let entry = method_registry_entry(&mut scope, runtime, specializers, qualifier, function)?;
    let mut next_entries = scope.root_many(&[]);
    dispatch_push_handle(&mut scope, &mut next_entries, entry);
    for item in entries.iter().copied() {
        dispatch_push_handle(&mut scope, &mut next_entries, item);
    }
    let registry = scope.make_list(runtime, &next_entries)?;
    set_method_registry(&mut scope, runtime, name, registry)?;
    Ok(scope.get(name).as_word())
}

#[allow(clippy::too_many_arguments)]
fn call_method<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    function: ncl_object::Handle<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    _argument_list: ncl_object::Handle<'ctx>,
    next: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let function_word = scope.get(function).as_word();
    let function_object = FunctionObject::try_from(function_word)?;
    let mut call_args = scope.root_many(&[]);
    if runtime.builtin_descriptor(function_object).is_none() {
        dispatch_push_handle(scope, &mut call_args, next);
    }
    for argument in arguments.iter().copied() {
        dispatch_push_handle(scope, &mut call_args, argument);
    }
    scope.call_function(runtime, function, &call_args, caller, values)
}

fn invoke_core<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    effective_method: EffectiveMethod<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let method_word = scope.get(effective_method).as_word();
    let before = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        1,
    )?));
    let primary = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        2,
    )?));
    let after = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        3,
    )?));
    let before = dispatch_list_to_handles(scope, before)?;
    let primary = dispatch_list_to_handles(scope, primary)?;
    let after = dispatch_list_to_handles(scope, after)?;
    for function in before.iter().copied() {
        let nil = scope.root(Local::from_word(Word::NIL));
        call_method(
            scope,
            runtime,
            function,
            arguments,
            argument_list,
            nil,
            caller,
            values,
        )?;
    }
    let first = *primary
        .as_slice()
        .first()
        .ok_or(ObjectError::UndefinedFunction)?;
    let rest = tail_handles(scope, &primary);
    let next = scope.make_list(runtime, &rest)?;
    let result = call_method(
        scope,
        runtime,
        first,
        arguments,
        argument_list,
        next,
        caller,
        values,
    )?;
    let returned = values.as_slice().to_vec();
    for function in after.iter().copied() {
        let nil = scope.root(Local::from_word(Word::NIL));
        call_method(
            scope,
            runtime,
            function,
            arguments,
            argument_list,
            nil,
            caller,
            values,
        )?;
    }
    values.set(&returned);
    Ok(result)
}

fn invoke_continuation<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    continuation: ncl_object::Handle<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let value = scope.get(continuation).as_word();
    if matches!(
        classify_object(scope.context(), value),
        ObjectRef::SimpleVector(_)
    ) {
        return invoke_core(
            scope,
            runtime,
            continuation,
            arguments,
            argument_list,
            caller,
            values,
        );
    }
    let next = dispatch_list_to_handles(scope, continuation)?;
    let function = *next
        .as_slice()
        .first()
        .ok_or(ObjectError::UndefinedFunction)?;
    if matches!(
        classify_object(scope.context(), scope.get(function).as_word()),
        ObjectRef::SimpleVector(_)
    ) {
        let marker = scope.root(Local::from_word(scope.get(function).as_word()));
        return invoke_core(
            scope,
            runtime,
            marker,
            arguments,
            argument_list,
            caller,
            values,
        );
    }
    let rest = tail_handles(scope, &next);
    let next = scope.make_list(runtime, &rest)?;
    call_method(
        scope,
        runtime,
        function,
        arguments,
        argument_list,
        next,
        caller,
        values,
    )
}

#[allow(clippy::too_many_arguments)]
fn invoke_dispatch<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    before: &ncl_object::HandleVec<'ctx>,
    primary: &ncl_object::HandleVec<'ctx>,
    after: &ncl_object::HandleVec<'ctx>,
    around: &ncl_object::HandleVec<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let before_list = scope.make_list(runtime, before)?;
    let primary_list = scope.make_list(runtime, primary)?;
    let after_list = scope.make_list(runtime, after)?;
    let marker_values = {
        let mut values = scope.root_many(&[]);
        let tag = scope.intern(runtime, "KEYWORD", "CLOS-CORE")?;
        dispatch_push_handle(scope, &mut values, tag);
        dispatch_push_handle(scope, &mut values, before_list);
        dispatch_push_handle(scope, &mut values, primary_list);
        dispatch_push_handle(scope, &mut values, after_list);
        values
    };
    let marker = scope.make_simple_vector(runtime, &marker_values)?;
    if around.is_empty() {
        return invoke_core(
            scope,
            runtime,
            marker,
            arguments,
            argument_list,
            caller,
            values,
        );
    }
    let mut tail_values = scope.root_many(&[]);
    for function in around.as_slice().get(1..).unwrap_or(&[]) {
        dispatch_push_handle(scope, &mut tail_values, *function);
    }
    dispatch_push_handle(scope, &mut tail_values, marker);
    let next_tail = scope.make_list(runtime, &tail_values)?;
    let first = *around.as_slice().first().ok_or(ObjectError::Layout)?;
    call_method(
        scope,
        runtime,
        first,
        arguments,
        argument_list,
        next_tail,
        caller,
        values,
    )
}

fn clos_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let argument_list = scope.root(Local::from_word(args.required(1)?));
    let arguments = dispatch_list_to_handles(&mut scope, argument_list)?;
    if !has_method_registry(&mut scope, runtime, name)? {
        return Err(ObjectError::UndefinedFunction);
    }
    let argument_words = arguments
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let registry = method_registry(&mut scope, runtime, name)?;
    let entries = dispatch_list_to_handles(&mut scope, registry)?;
    let mut matches = Vec::new();
    for entry in entries.iter().copied() {
        let fields = dispatch_list_to_handles(&mut scope, entry)?;
        let specializers = scope
            .get(*fields.as_slice().first().ok_or(ObjectError::TypeError)?)
            .as_word();
        let qualifier = scope
            .get(*fields.as_slice().get(1).ok_or(ObjectError::TypeError)?)
            .as_word();
        let function = scope
            .get(*fields.as_slice().get(2).ok_or(ObjectError::TypeError)?)
            .as_word();
        if let Some(score) =
            method_match(scope.context_mut(), runtime, specializers, &argument_words)?
        {
            matches.push((score, qualifier, function));
        }
    }
    matches.sort_by_key(|left| std::cmp::Reverse(left.0));
    let mut before_words = Vec::new();
    let mut primary_words = Vec::new();
    let mut after_words = Vec::new();
    let mut around_words = Vec::new();
    for (_, qualifier, function) in matches {
        match qualifier.as_fixnum() {
            Some(METHOD_QUALIFIER_AROUND) => around_words.push(function),
            Some(METHOD_QUALIFIER_BEFORE) => before_words.push(function),
            Some(METHOD_QUALIFIER_AFTER) => after_words.push(function),
            _ => primary_words.push(function), // check-added-lines: allow(wildcard) primary fallback
        }
    }
    after_words.reverse();
    if primary_words.is_empty() {
        return Err(ObjectError::UndefinedFunction);
    }
    let before = scope.root_many(
        &before_words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let primary = scope.root_many(
        &primary_words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let after = scope.root_many(
        &after_words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let around = scope.root_many(
        &around_words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let mut caller = BuiltinFunctionCaller;
    let result = invoke_dispatch(
        &mut scope,
        runtime,
        &before,
        &primary,
        &after,
        &around,
        &arguments,
        argument_list,
        &mut caller,
        values,
    )?;
    Ok(scope.get(result).as_word())
}

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
        return runtime
            .class(ctx, &name)
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
