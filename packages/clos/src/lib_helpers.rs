fn form_elements(ctx: &ThreadContext, mut form: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while form != Word::NIL {
        if !form.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(ncl_object::car(ctx, form)?);
        form = ncl_object::cdr(ctx, form)?;
    }
    Ok(result)
}

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = ncl_object::symbol_name(ctx, symbol)?;
    let length = ncl_object::string_length(ctx, name)?;
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect()
}

fn push_helper_handle<'ctx>(
    scope: &mut Scope<'ctx>,
    values: &mut ncl_object::HandleVec<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) {
    let local = Local::from_word(scope.get(handle).as_word());
    values.push(scope, local);
}

fn quoted<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    value: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let quote = scope.intern(runtime, COMMON_LISP, "QUOTE")?;
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, quote);
    push_helper_handle(scope, &mut values, value);
    scope.make_list(runtime, &values)
}

fn make_accessor_definition<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    slot: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let instance = scope.intern(runtime, "NCL", "INSTANCE")?;
    let mut lambda_values = scope.root_many(&[]);
    push_helper_handle(scope, &mut lambda_values, instance);
    let lambda = scope.make_list(runtime, &lambda_values)?;
    let slot_value = scope.intern(runtime, COMMON_LISP, "SLOT-VALUE")?;
    let quoted_slot = quoted(scope, runtime, slot)?;
    let mut body_values = scope.root_many(&[]);
    push_helper_handle(scope, &mut body_values, slot_value);
    push_helper_handle(scope, &mut body_values, instance);
    push_helper_handle(scope, &mut body_values, quoted_slot);
    let body = scope.make_list(runtime, &body_values)?;
    let mut definition = scope.root_many(&[]);
    push_helper_handle(scope, &mut definition, defun);
    push_helper_handle(scope, &mut definition, name);
    push_helper_handle(scope, &mut definition, lambda);
    push_helper_handle(scope, &mut definition, body);
    scope.make_list(runtime, &definition)
}

fn make_progn<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    forms: &ncl_object::HandleVec<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let progn = scope.intern(runtime, COMMON_LISP, "PROGN")?;
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, progn);
    for form in forms.iter().copied() {
        push_helper_handle(scope, &mut values, form);
    }
    scope.make_list(runtime, &values)
}

#[allow(clippy::too_many_arguments)]
fn method_registry_entry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    specializers: ncl_object::Handle<'ctx>,
    qualifier: ncl_object::Handle<'ctx>,
    function: ncl_object::Handle<'ctx>,
    method: ncl_object::Handle<'ctx>,
    qualifier_list: ncl_object::Handle<'ctx>,
    origin: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, specializers);
    push_helper_handle(scope, &mut values, qualifier);
    push_helper_handle(scope, &mut values, function);
    push_helper_handle(scope, &mut values, method);
    push_helper_handle(scope, &mut values, qualifier_list);
    push_helper_handle(scope, &mut values, origin);
    scope.make_list(runtime, &values)
}

fn symbol_plist_property<'ctx>(
    scope: &mut Scope<'ctx>,
    name: ncl_object::Handle<'ctx>,
    key: ncl_object::Handle<'ctx>,
) -> Result<Option<ncl_object::Handle<'ctx>>, ObjectError> {
    let name_word = scope.get(name).as_word();
    let key_word = scope.get(key).as_word();
    let mut plist = ncl_object::symbol_plist(scope.context(), name_word)?;
    while plist != Word::NIL {
        let indicator = ncl_object::car(scope.context(), plist)?;
        let value_cell = ncl_object::cdr(scope.context(), plist)?;
        if !value_cell.is_cons() {
            return Err(ObjectError::TypeError);
        }
        if indicator == key_word {
            let value = ncl_object::car(scope.context(), value_cell)?;
            return Ok(Some(scope.root(Local::from_word(value))));
        }
        plist = ncl_object::cdr(scope.context(), value_cell)?;
    }
    Ok(None)
}

fn set_symbol_plist_property<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    key: ncl_object::Handle<'ctx>,
    value: ncl_object::Handle<'ctx>,
) -> Result<(), ObjectError> {
    let name_word = scope.get(name).as_word();
    let key_word = scope.get(key).as_word();
    let value_word = scope.get(value).as_word();
    let old_plist = scope.root(Local::from_word(ncl_object::symbol_plist(
        scope.context(),
        name_word,
    )?));
    let mut plist = scope.get(old_plist).as_word();
    while plist != Word::NIL {
        let indicator = ncl_object::car(scope.context(), plist)?;
        let value_cell = ncl_object::cdr(scope.context(), plist)?;
        if !value_cell.is_cons() {
            return Err(ObjectError::TypeError);
        }
        if indicator == key_word {
            return ncl_object::rplaca(scope.context_mut(), value_cell, value_word).map(|_| ());
        }
        plist = ncl_object::cdr(scope.context(), value_cell)?;
    }
    let value_cell = scope.make_cons(runtime, value, old_plist)?;
    let new_plist = scope.make_cons(runtime, key, value_cell)?;
    let name_word = scope.get(name).as_word();
    let plist_word = scope.get(new_plist).as_word();
    ncl_object::set_symbol_plist(scope.context_mut(), name_word, plist_word)
}

fn method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    if let Some(registry) = symbol_plist_property(scope, name, key)? {
        return Ok(registry);
    }
    Ok(scope.root(Local::from_word(Word::NIL)))
}

fn has_method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<bool, ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    Ok(symbol_plist_property(scope, name, key)?.is_some())
}

fn set_method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    registry: ncl_object::Handle<'ctx>,
) -> Result<(), ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    set_symbol_plist_property(scope, runtime, name, key, registry)
}

fn method_registry_key<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    scope.intern(runtime, "NCL", METHOD_REGISTRY_KEY_NAME)
}

fn method_definition_parts<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    encoded: ncl_object::Handle<'ctx>,
) -> Result<
    (
        ncl_object::Handle<'ctx>,
        ncl_object::Handle<'ctx>,
        ncl_object::Handle<'ctx>,
        ncl_object::Handle<'ctx>,
    ),
    ObjectError,
> {
    let encoded_local = Local::from_word(scope.get(encoded).as_word());
    let fields = scope.list_to_handle_vec(encoded_local)?;
    let tag = scope.intern(runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
    if fields
        .as_slice()
        .first()
        .is_some_and(|field| scope.get(*field).as_word() == scope.get(tag).as_word())
    {
        let qualifier = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
        let qualifier_list = *fields.as_slice().get(2).ok_or(ObjectError::TypeError)?;
        let specializers = *fields.as_slice().get(3).ok_or(ObjectError::TypeError)?;
        let default_origin = scope.root(Local::from_word(Word::NIL));
        let origin = fields.as_slice().get(4).copied().unwrap_or(default_origin);
        return Ok((specializers, qualifier, qualifier_list, origin));
    }
    let qualifier = scope.root(Local::from_word(Word::fixnum(METHOD_QUALIFIER_PRIMARY)));
    let qualifier_list = scope.root(Local::from_word(Word::NIL));
    let origin = scope.root(Local::from_word(Word::NIL));
    Ok((encoded, qualifier, qualifier_list, origin))
}

fn method_qualifier(ctx: &ThreadContext, value: Word) -> Result<Option<Word>, ObjectError> {
    if !matches!(
        ncl_object::classify_object(ctx, value),
        ObjectRef::Symbol(_)
    ) {
        return Ok(None);
    }
    let name = symbol_name_string(ctx, value)?;
    let qualifier = if matches!(name.as_str(), ":BEFORE" | "BEFORE") {
        METHOD_QUALIFIER_BEFORE
    } else if matches!(name.as_str(), ":AFTER" | "AFTER") {
        METHOD_QUALIFIER_AFTER
    } else if matches!(name.as_str(), ":AROUND" | "AROUND") {
        METHOD_QUALIFIER_AROUND
    } else {
        return Ok(None);
    };
    Ok(Some(Word::fixnum(qualifier)))
}
