const GENERIC_REQUIRED_ARITY_KEY_NAME: &str = "%CLOS-REQUIRED-ARITY";
const GENERIC_FUNCTION_MAP_SYMBOL: &str = "%CLOS-GENERIC-FUNCTION-MAP";
const GENERIC_FUNCTION_MAP_KEY: &str = "%CLOS-GENERIC-FUNCTION-MAP-VALUE";

fn required_parameter_count(ctx: &ThreadContext, lambda_list: Word) -> Result<usize, ObjectError> {
    let mut count = 0;
    for parameter in form_elements(ctx, lambda_list)? {
        if matches!(classify_object(ctx, parameter), ObjectRef::Symbol(_))
            && symbol_name_string(ctx, parameter)?.starts_with('&')
        {
            break;
        }
        count += 1;
    }
    Ok(count)
}

fn generic_function_mapping<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let symbol = scope.intern(runtime, "NCL", GENERIC_FUNCTION_MAP_SYMBOL)?;
    let key = scope.intern(runtime, "NCL", GENERIC_FUNCTION_MAP_KEY)?;
    if let Some(mapping) = symbol_plist_property(scope, symbol, key)? {
        return Ok(mapping);
    }
    Ok(scope.root(Local::from_word(Word::NIL)))
}

fn set_generic_function_mapping<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    function: ncl_object::Handle<'ctx>,
    name: ncl_object::Handle<'ctx>,
) -> Result<(), ObjectError> {
    let function_word = scope.get(function).as_word();
    if !matches!(
        classify_object(scope.context(), function_word),
        ObjectRef::Function(_) | ObjectRef::Closure(_)
    ) {
        return Err(ObjectError::TypeError);
    }
    let mapping = generic_function_mapping(scope, runtime)?;
    let previous = dispatch_list_to_handles(scope, mapping)?;
    let mut next = scope.root_many(&[]);
    for entry in previous.iter().copied() {
        let fields = dispatch_list_to_handles(scope, entry)?;
        let old_function = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        let old_name = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
        if scope.get(old_function).as_word() != scope.get(function).as_word()
            && scope.get(old_name).as_word() != scope.get(name).as_word()
        {
            dispatch_push_handle(scope, &mut next, entry);
        }
    }
    let mut fields = scope.root_many(&[]);
    dispatch_push_handle(scope, &mut fields, function);
    dispatch_push_handle(scope, &mut fields, name);
    let entry = scope.make_list(runtime, &fields)?;
    dispatch_push_handle(scope, &mut next, entry);
    let registry = scope.make_list(runtime, &next)?;

    let symbol = scope.intern(runtime, "NCL", GENERIC_FUNCTION_MAP_SYMBOL)?;
    let key = scope.intern(runtime, "NCL", GENERIC_FUNCTION_MAP_KEY)?;
    set_symbol_plist_property(scope, runtime, symbol, key, registry)
}

fn generic_function_name<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    function: ncl_object::Handle<'ctx>,
) -> Result<Option<ncl_object::Handle<'ctx>>, ObjectError> {
    let mapping = generic_function_mapping(scope, runtime)?;
    let entries = dispatch_list_to_handles(scope, mapping)?;
    for entry in entries.iter().copied() {
        let fields = dispatch_list_to_handles(scope, entry)?;
        let registered_function = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        if scope.get(registered_function).as_word() == scope.get(function).as_word() {
            return fields
                .as_slice()
                .get(1)
                .copied()
                .map(Some)
                .ok_or(ObjectError::TypeError);
        }
    }
    Ok(None)
}

fn generic_function_name_for_word(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
) -> Result<Option<Word>, ObjectError> {
    let mut scope = Scope::new(ctx);
    let function = scope.root(Local::from_word(function));
    let Some(name) = generic_function_name(&mut scope, runtime, function)? else {
        return Ok(None);
    };
    Ok(Some(scope.get(name).as_word()))
}

fn generic_function_registered_for_name<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<bool, ObjectError> {
    let mapping = generic_function_mapping(scope, runtime)?;
    let entries = dispatch_list_to_handles(scope, mapping)?;
    for entry in entries.iter().copied() {
        let fields = dispatch_list_to_handles(scope, entry)?;
        let registered_name = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
        if scope.get(registered_name).as_word() == scope.get(name).as_word() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn generic_required_arity<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<Option<usize>, ObjectError> {
    let key = scope.intern(runtime, "NCL", GENERIC_REQUIRED_ARITY_KEY_NAME)?;
    let Some(value) = symbol_plist_property(scope, name, key)? else {
        return Ok(None);
    };
    let fixnum = Fixnum::try_from_word(scope.get(value).as_word()).map_err(|_| ObjectError::Layout)?;
    usize::try_from(fixnum.value())
        .map(Some)
        .map_err(|_| ObjectError::Layout)
}

fn set_generic_required_arity<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    arity: usize,
) -> Result<(), ObjectError> {
    let key = scope.intern(runtime, "NCL", GENERIC_REQUIRED_ARITY_KEY_NAME)?;
    let arity = i64::try_from(arity).map_err(|_| ObjectError::Layout)?;
    let arity = scope.root(Local::from_word(Word::fixnum(arity)));
    set_symbol_plist_property(scope, runtime, name, key, arity)
}

fn clos_define_generic_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let lambda_list: ncl_object::Handle<'_, Word> = scope.root(Local::from_word(args.required(1)?));
    let name_word = scope.get(name).as_word();
    if ncl_object::symbol_is_macro(scope.context(), name_word)? {
        return Err(defgeneric_program_error(&mut scope));
    }
    let function = ncl_object::symbol_function(scope.context(), name_word)?;
    // check-added-lines: allow(unbound) absent function cells use this marker
    if function == Word::UNBOUND {
        return Err(ObjectError::UndefinedFunction);
    }
    let function = scope.root(Local::from_word(function));
    let arity = required_parameter_count(scope.context(), scope.get(lambda_list).as_word())?;
    let empty = scope.root(Local::from_word(Word::NIL));
    set_method_registry(&mut scope, runtime, name, empty)?;
    set_generic_required_arity(&mut scope, runtime, name, arity)?;
    set_generic_function_mapping(&mut scope, runtime, function, name)?;
    Ok(scope.get(name).as_word())
}

fn clos_redefine_generic_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    let lambda_list: ncl_object::Handle<'_, Word> = scope.root(Local::from_word(args.required(1)?));
    let name_word = scope.get(name).as_word();
    if ncl_object::symbol_is_macro(scope.context(), name_word)? {
        return Err(defgeneric_program_error(&mut scope));
    }
    let function_word = ncl_object::symbol_function(scope.context(), name_word)?;
    // check-added-lines: allow(unbound) absent function cells use this marker
    if function_word == Word::UNBOUND {
        return Err(defgeneric_program_error(&mut scope));
    }
    let function = scope.root(Local::from_word(function_word));
    let Some(mapped_name) = generic_function_name(&mut scope, runtime, function)? else {
        return Err(defgeneric_program_error(&mut scope));
    };
    if scope.get(mapped_name).as_word() != scope.get(name).as_word() {
        return Err(defgeneric_program_error(&mut scope));
    }

    let arity = required_parameter_count(scope.context(), scope.get(lambda_list).as_word())?;
    let old_registry = method_registry(&mut scope, runtime, name)?;
    let old_entries = dispatch_list_to_handles(&mut scope, old_registry)?;
    let mut retained_entries = scope.root_many(&[]);
    for entry in old_entries.iter().copied() {
        let fields = dispatch_list_to_handles(&mut scope, entry)?;
        let is_inline_method = fields
            .as_slice()
            .get(5)
            .is_some_and(|origin| scope.get(*origin).as_word() == Word::fixnum(1));
        if !is_inline_method {
            dispatch_push_handle(&mut scope, &mut retained_entries, entry);
        }
    }
    let retained_registry = scope.make_list(runtime, &retained_entries)?;
    set_method_registry(&mut scope, runtime, name, retained_registry)?;
    set_generic_required_arity(&mut scope, runtime, name, arity)?;
    Ok(scope.get(name).as_word())
}

fn clos_generic_function_registered_p_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let name = scope.root(Local::from_word(args.required(0)?));
    Ok(
        if generic_function_registered_for_name(&mut scope, runtime, name)? {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

const fn defgeneric_program_error_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    ctx.set_pending_lisp_error(ncl_object::LispError::ProgramError(
        ncl_object::ProgramError::UnknownKeyword,
    ));
    Err(ObjectError::TypeError)
}

const fn defgeneric_program_error(scope: &mut Scope<'_>) -> ObjectError {
    scope
        .context_mut()
        .set_pending_lisp_error(ncl_object::LispError::ProgramError(
            ncl_object::ProgramError::UnknownKeyword,
        ));
    ObjectError::TypeError
}
