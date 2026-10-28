include!("lib_dispatch_helpers.rs");

fn clos_ensure_initialization_base_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let name_word = scope.get(name).as_word();
    let initialize_name = scope.intern(runtime, COMMON_LISP, "INITIALIZE-INSTANCE")?;
    let shared_name = scope.intern(runtime, COMMON_LISP, "SHARED-INITIALIZE")?;
    let is_shared_initialize = name_word == scope.get(shared_name).as_word();
    if name_word != scope.get(initialize_name).as_word() && !is_shared_initialize {
        return Ok(name_word);
    }
    if has_method_registry(&mut scope, runtime, name)? {
        return Ok(name_word);
    }
    let function = symbol_function(scope.context(), name_word)?;
    // check-added-lines: allow(unbound) absent function cells use this marker
    if function == Word::UNBOUND {
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
        if is_shared_initialize {
            let slot_names = scope.intern(runtime, "NCL", "SLOT-NAMES")?;
            let slot_names_specializer = {
                let mut fields = scope.root_many(&[]);
                dispatch_push_handle(&mut scope, &mut fields, slot_names);
                dispatch_push_handle(&mut scope, &mut fields, class);
                scope.make_list(runtime, &fields)?
            };
            dispatch_push_handle(&mut scope, &mut values, slot_names_specializer);
        }
        scope.make_list(runtime, &values)?
    };
    let function = scope.root(Local::from_word(function));
    let qualifier = scope.root(Local::from_word(Word::NIL));
    let method = allocate_standard_method(&mut scope, runtime)?;
    let qualifier_list = scope.root(Local::from_word(Word::NIL));
    let origin = scope.root(Local::from_word(Word::NIL));
    let entry = method_registry_entry(
        &mut scope,
        runtime,
        specializers,
        qualifier,
        function,
        method,
        qualifier_list,
        origin,
    )?;
    let registry = {
        let mut values = scope.root_many(&[]);
        dispatch_push_handle(&mut scope, &mut values, entry);
        scope.make_list(runtime, &values)?
    };
    set_method_registry(&mut scope, runtime, name, registry)?;
    let generic_function_word = symbol_function(scope.context(), scope.get(name).as_word())?;
    let generic_function = scope.root(Local::from_word(generic_function_word));
    set_generic_function_mapping(&mut scope, runtime, generic_function, name)?;
    set_generic_required_arity(
        &mut scope,
        runtime,
        name,
        if is_shared_initialize { 2 } else { 1 },
    )?;
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
    let (specializers, qualifier, qualifier_list, origin) =
        method_definition_parts(&mut scope, runtime, encoded)?;
    let function = scope.root(Local::from_word(
        FunctionObject::try_from(args.required(2)?)?.as_word(),
    ));
    let generic_function_word = symbol_function(scope.context(), scope.get(name).as_word())?;
    let generic_function = scope.root(Local::from_word(generic_function_word));
    set_generic_function_mapping(&mut scope, runtime, generic_function, name)?;
    if generic_required_arity(&mut scope, runtime, name)?.is_none() {
        let arity = dispatch_list_to_handles(&mut scope, specializers)?.len();
        set_generic_required_arity(&mut scope, runtime, name, arity)?;
    }
    let old = method_registry(&mut scope, runtime, name)?;
    let old_entries = dispatch_list_to_handles(&mut scope, old)?;
    let mut entries = scope.root_many(&[]);
    for entry in old_entries.iter().copied() {
        let fields = dispatch_list_to_handles(&mut scope, entry)?;
        let entry_specializers = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        let entry_qualifier_list = *fields.as_slice().get(4).ok_or(ObjectError::TypeError)?;
        let entry_specializers_word = scope.get(entry_specializers).as_word();
        let specializers_word = scope.get(specializers).as_word();
        let same_specializers = method_specializers_equal(
            scope.context_mut(),
            runtime,
            entry_specializers_word,
            specializers_word,
        )?;
        let same_qualifiers = lisp_equal_word(
            scope.context(),
            scope.get(entry_qualifier_list).as_word(),
            scope.get(qualifier_list).as_word(),
            0,
        )?;
        if !same_specializers || !same_qualifiers {
            dispatch_push_handle(&mut scope, &mut entries, entry);
        }
    }
    let method = allocate_standard_method(&mut scope, runtime)?;
    let entry = method_registry_entry(
        &mut scope,
        runtime,
        specializers,
        qualifier,
        function,
        method,
        qualifier_list,
        origin,
    )?;
    let mut next_entries = scope.root_many(&[]);
    dispatch_push_handle(&mut scope, &mut next_entries, entry);
    for item in entries.iter().copied() {
        dispatch_push_handle(&mut scope, &mut next_entries, item);
    }
    let registry = scope.make_list(runtime, &next_entries)?;
    set_method_registry(&mut scope, runtime, name, registry)?;
    Ok(scope.get(method).as_word())
}

#[allow(clippy::too_many_arguments)]
fn call_method<'ctx, C: ncl_object::FunctionCaller>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    function: ncl_object::Handle<'ctx>,
    arguments: &ncl_object::HandleVec<'ctx>,
    argument_list: ncl_object::Handle<'ctx>,
    next: ncl_object::Handle<'ctx>,
    caller: &mut C,
    values: &mut MultipleValues,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let function_word = scope.get(function).as_word();
    let function_object = FunctionObject::try_from(function_word)?;
    let mut call_args = scope.root_many(&[]);
    if runtime
        .builtin_descriptor(scope.context(), function_object)
        .is_none()
    {
        dispatch_push_handle(scope, &mut call_args, next);
        dispatch_push_handle(scope, &mut call_args, argument_list);
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
    let combination = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        1,
    )?));
    let before = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        2,
    )?));
    let primary = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        3,
    )?));
    let after = scope.root(Local::from_word(simple_vector_ref(
        scope.context(),
        method_word,
        4,
    )?));
    let combination_kind =
        method_combination_kind(scope.context(), scope.get(combination).as_word())?;
    let before = dispatch_list_to_handles(scope, before)?;
    let primary = dispatch_list_to_handles(scope, primary)?;
    let after = dispatch_list_to_handles(scope, after)?;
    if combination_kind == MethodCombination::Standard {
        return lib_dispatch_effective_methods::invoke_standard_effective_method(
            scope,
            runtime,
            &before,
            &primary,
            &after,
            arguments,
            argument_list,
            caller,
            values,
        );
    }
    lib_dispatch_effective_methods::invoke_short_effective_method(
        scope,
        runtime,
        combination_kind,
        combination,
        &primary,
        arguments,
        argument_list,
        caller,
        values,
    )
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
    combination: ncl_object::Handle<'ctx>,
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
        dispatch_push_handle(scope, &mut values, combination);
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
    let (combination, order) = generic_method_combination(&mut scope, runtime, name)?;
    let combination_kind =
        method_combination_kind(scope.context(), scope.get(combination).as_word())?;
    let most_specific_first = scope.intern(runtime, "KEYWORD", "MOST-SPECIFIC-FIRST")?;
    let most_specific_last = scope.intern(runtime, "KEYWORD", "MOST-SPECIFIC-LAST")?;
    if combination_kind != MethodCombination::Standard
        && scope.get(order).as_word() != scope.get(most_specific_first).as_word()
        && scope.get(order).as_word() != scope.get(most_specific_last).as_word()
    {
        return Err(ObjectError::TypeError);
    }
    let registry = method_registry(&mut scope, runtime, name)?;
    let entries = dispatch_list_to_handles(&mut scope, registry)?;
    let groups = lib_dispatch_selection::collect_applicable_methods(
        &mut scope,
        runtime,
        combination,
        combination_kind,
        order,
        &arguments,
        &entries,
    )?;
    if groups.primary.is_empty() {
        return Err(ObjectError::UndefinedFunction);
    }
    let before = scope.root_many(
        &groups
            .before
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let primary = scope.root_many(
        &groups
            .primary
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let after = scope.root_many(
        &groups
            .after
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let around = scope.root_many(
        &groups
            .around
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>(),
    );
    let mut caller = BuiltinFunctionCaller;
    let result = invoke_dispatch(
        &mut scope,
        runtime,
        combination,
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

include!("lib_dispatch_terminal.rs");
