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

fn method_registry_entry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    specializers: ncl_object::Handle<'ctx>,
    qualifier: ncl_object::Handle<'ctx>,
    function: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, specializers);
    push_helper_handle(scope, &mut values, qualifier);
    push_helper_handle(scope, &mut values, function);
    scope.make_list(runtime, &values)
}

fn method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    let mut plist = ncl_object::symbol_plist(scope.context(), scope.get(name).as_word())?;
    while plist != Word::NIL {
        let property = ncl_object::car(scope.context(), plist)?;
        if ncl_object::car(scope.context(), property)? == scope.get(key).as_word() {
            return Ok(scope.root(Local::from_word(ncl_object::cdr(
                scope.context(),
                property,
            )?)));
        }
        plist = ncl_object::cdr(scope.context(), plist)?;
    }
    Ok(scope.root(Local::from_word(Word::NIL)))
}

fn has_method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<bool, ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    let mut plist = ncl_object::symbol_plist(scope.context(), scope.get(name).as_word())?;
    while plist != Word::NIL {
        let property = ncl_object::car(scope.context(), plist)?;
        if ncl_object::car(scope.context(), property)? == scope.get(key).as_word() {
            return Ok(true);
        }
        plist = ncl_object::cdr(scope.context(), plist)?;
    }
    Ok(false)
}

fn set_method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    registry: ncl_object::Handle<'ctx>,
) -> Result<(), ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    let old_plist = scope.root(Local::from_word(ncl_object::symbol_plist(
        scope.context(),
        scope.get(name).as_word(),
    )?));
    let property = scope.make_cons(runtime, key, registry)?;
    let plist = scope.make_cons(runtime, property, old_plist)?;
    let name_word = scope.get(name).as_word();
    let plist_word = scope.get(plist).as_word();
    ncl_object::set_symbol_plist(scope.context_mut(), name_word, plist_word)
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
) -> Result<(ncl_object::Handle<'ctx>, ncl_object::Handle<'ctx>), ObjectError> {
    let encoded_local = Local::from_word(scope.get(encoded).as_word());
    let fields = scope.list_to_handle_vec(encoded_local)?;
    let tag = scope.intern(runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
    if fields
        .as_slice()
        .first()
        .is_some_and(|field| scope.get(*field).as_word() == scope.get(tag).as_word())
    {
        let qualifier = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
        let specializers = *fields.as_slice().get(2).ok_or(ObjectError::TypeError)?;
        return Ok((specializers, qualifier));
    }
    let qualifier = scope.root(Local::from_word(Word::fixnum(METHOD_QUALIFIER_PRIMARY)));
    Ok((encoded, qualifier))
}

fn method_qualifier(ctx: &ThreadContext, value: Word) -> Result<Option<Word>, ObjectError> {
    if !matches!(ncl_object::classify_object(ctx, value), ObjectRef::Symbol(_)) {
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

fn eql_word(ctx: &ThreadContext, left: Word, right: Word) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (ncl_object::classify_object(ctx, left), ncl_object::classify_object(ctx, right)),
        (ObjectRef::DoubleFloat(left), ObjectRef::DoubleFloat(right)) if left == right
    )
}

fn class_depth(ctx: &ThreadContext, class: Word) -> Result<usize, ObjectError> {
    let superclass = ncl_object::simple_vector_ref(ctx, class, CLASS_DIRECT_SUPERCLASS)?;
    if superclass == Word::NIL {
        Ok(0)
    } else {
        Ok(class_depth(ctx, superclass)? + 1)
    }
}

fn method_match(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializers: Word,
    arguments: &[Word],
) -> Result<Option<usize>, ObjectError> {
    let specializers = form_elements(ctx, specializers)?;
    if specializers.len() != arguments.len() {
        return Ok(None);
    }
    let mut score = 0;
    for (specializer, argument) in specializers.iter().zip(arguments) {
        let fields = form_elements(ctx, *specializer)?;
        let designator = *fields.get(1).ok_or(ObjectError::TypeError)?;
        if designator.is_cons() {
            let eql_fields = form_elements(ctx, designator)?;
            let head = eql_fields.first().copied().ok_or(ObjectError::TypeError)?;
            let head_name = symbol_name_string(ctx, head)?;
            if head_name != "EQL"
                || !eql_word(
                    ctx,
                    *argument,
                    *eql_fields.get(1).ok_or(ObjectError::TypeError)?,
                )
            {
                return Ok(None);
            }
            score += 10_000;
            continue;
        }
        if symbol_name_string(ctx, designator)? == "T" {
            continue;
        }
        let expected = class_designator(ctx, runtime, designator)?;
        let actual = class_of(ctx, runtime, *argument)?;
        if !class_is_subclass(ctx, actual, expected)? {
            return Ok(None);
        }
        score += class_depth(ctx, expected)?;
    }
    Ok(Some(score))
}
