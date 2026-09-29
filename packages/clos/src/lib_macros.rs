#[allow(clippy::too_many_lines)]
fn defclass_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let form = scope.root(ncl_object::Local::from_word(args.required(0)?));
    let parts = macro_list_to_handles(&mut scope, form)?;
    let name = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let supers = parts
        .as_slice()
        .get(2)
        .copied()
        .map(|handle| macro_list_to_handles(&mut scope, handle))
        .transpose()?
        .unwrap_or_else(|| scope.root_many(&[])); // check-added-lines: allow(panic) empty default
    let superclass_word = supers
        .as_slice()
        .first()
        .map_or(Word::NIL, |handle| scope.get(*handle).as_word());
    let superclass = supers
        .as_slice()
        .first()
        .map(|_| class_designator(scope.context_mut(), runtime, superclass_word))
        .transpose()?
        .unwrap_or(Word::NIL);
    let superclass: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(superclass));
    let class_name = symbol_name_string(scope.context(), scope.get(name).as_word())?;
    let slot_forms = parts.as_slice().get(3).copied();
    let slot_forms = slot_forms
        .map(|handle| macro_list_to_handles(&mut scope, handle))
        .transpose()?
        .unwrap_or_else(|| scope.root_many(&[])); // check-added-lines: allow(panic) empty default
    let mut slot_specs = Vec::new();
    let mut accessors = Vec::new();
    for slot_form in slot_forms.iter().copied() {
        let fields = macro_list_to_handles(&mut scope, slot_form)?;
        let slot_name = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
        let mut initarg = scope.root(ncl_object::Local::from_word(Word::NIL));
        let mut initform = scope.root(ncl_object::Local::from_word(Word::UNBOUND)); // check-added-lines: allow(unbound) sentinel check
        let mut index = 1;
        while index + 1 < fields.len() {
            let key = fields.as_slice()[index]; // check-added-lines: allow(index) validated pair
            let key_name = symbol_name_string(scope.context(), scope.get(key).as_word())?;
            let value = fields.as_slice()[index + 1]; // check-added-lines: allow(index) validated pair
            if key_name == ":INITARG" || key_name == "INITARG" {
                initarg = value;
            }
            if key_name == ":INITFORM" || key_name == "INITFORM" {
                initform = value;
            }
            if (key_name == ":ACCESSOR"
                || key_name == "ACCESSOR"
                || key_name == ":READER"
                || key_name == "READER")
                && scope.get(value).as_word() != Word::NIL
            {
                accessors.push((value, slot_name));
            }
            index += 2;
        }
        slot_specs.push((slot_name, initarg, initform));
    }
    let mut descriptors = Vec::new();
    for (slot_name, initarg, initform) in slot_specs {
        let mut values = scope.root_many(&[]);
        macro_push_handle(&mut scope, &mut values, slot_name);
        macro_push_handle(&mut scope, &mut values, initarg);
        macro_push_handle(&mut scope, &mut values, initform);
        descriptors.push(scope.make_simple_vector(runtime, &values)?);
    }
    let mut descriptor_values = scope.root_many(&[]);
    for descriptor in descriptors {
        macro_push_handle(&mut scope, &mut descriptor_values, descriptor);
    }
    let slots = scope.make_simple_vector(runtime, &descriptor_values)?;
    let name_word = scope.get(name).as_word();
    let slots_word = scope.get(slots).as_word();
    let superclass_word = scope.get(superclass).as_word();
    let class = make_class(
        scope.context_mut(),
        runtime,
        name_word,
        superclass_word,
        slots_word,
        Word::NIL,
    )?;
    let class: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(class));
    let class_word = scope.get(class).as_word();
    runtime.define_class(scope.context_mut(), class_name, class_word)?;
    let mut definitions = Vec::new();
    for (accessor, slot) in accessors {
        definitions.push(make_accessor_definition(&mut scope, runtime, accessor, slot)?);
    }
    if definitions.is_empty() {
        return Ok(Word::NIL);
    }
    let definition_words = definitions
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let definitions = scope.root_many(
        &definition_words
            .iter()
            .copied()
            .map(ncl_object::Local::from_word)
            .collect::<Vec<_>>(),
    );
    let result = make_progn(&mut scope, runtime, &definitions)?;
    Ok(scope.get(result).as_word())
}

fn defstruct_slot_spec<'a>(
    scope: &ncl_object::Scope<'a>,
    fields: &ncl_object::HandleVec<'a, Word>,
) -> Result<(Word, Word, bool), ObjectError> {
    let slot = scope
        .get(*fields.as_slice().first().ok_or(ObjectError::TypeError)?)
        .as_word();
    let initform = fields
        .as_slice()
        .get(1)
        .map_or(Word::NIL, |handle| scope.get(*handle).as_word());
    let mut read_only = false;
    let mut index = 2;
    while index + 1 < fields.len() {
        let key = *fields.as_slice().get(index).ok_or(ObjectError::TypeError)?;
        let value = *fields.as_slice().get(index + 1).ok_or(ObjectError::TypeError)?;
        let key_name = symbol_name_string(scope.context(), scope.get(key).as_word())?;
        if key_name == ":READ-ONLY" || key_name == "READ-ONLY" {
            read_only = !defstruct_is_nil(scope.context(), scope.get(value).as_word())?;
        }
        index += 2;
    }
    Ok((slot, initform, read_only))
}

fn defstruct_is_nil(ctx: &ThreadContext, value: Word) -> Result<bool, ObjectError> {
    if value == Word::NIL {
        return Ok(true);
    }
    Ok(matches!(ncl_object::classify_object(ctx, value), ObjectRef::Symbol(_))
        && symbol_name_string(ctx, value)? == "NIL")
}

#[allow(clippy::too_many_lines)]
fn defstruct_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let form = scope.root(ncl_object::Local::from_word(args.required(0)?));
    let parts = macro_list_to_handles(&mut scope, form)?;
    let name_form = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let mut include = Word::NIL;
    let mut include_overrides = Vec::new();
    let mut conc_name = None;
    let mut predicate_name = None;
    let mut copier_name = None;
    let mut predicate_disabled = false;
    let mut copier_disabled = false;
    let (name, mut option_index) = if scope.get(name_form).as_word().is_cons() {
        let header = macro_list_to_handles(&mut scope, name_form)?;
        let name = *header.as_slice().first().ok_or(ObjectError::TypeError)?;
        for option in header.as_slice().iter().copied().skip(1) {
            let fields = macro_list_to_handles(&mut scope, option)?;
            let key = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
            let value = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
            let key_name = symbol_name_string(scope.context(), scope.get(key).as_word())?;
            if key_name == ":INCLUDE" || key_name == "INCLUDE" {
                include = scope.get(value).as_word();
                if include != Word::NIL && include.is_cons() {
                    let include_fields = macro_list_to_handles(&mut scope, value)?;
                    include = scope.get(
                        *include_fields.as_slice().first().ok_or(ObjectError::TypeError)?,
                    ).as_word();
                    for override_form in include_fields.as_slice().iter().skip(1).copied() {
                        let fields = macro_list_to_handles(&mut scope, override_form)?;
                        include_overrides.push(defstruct_slot_spec(&scope, &fields)?);
                    }
                } else {
                    for override_form in fields.as_slice().iter().skip(2).copied() {
                        let fields = macro_list_to_handles(&mut scope, override_form)?;
                        include_overrides.push(defstruct_slot_spec(&scope, &fields)?);
                    }
                }
            } else if key_name == ":PREDICATE" || key_name == "PREDICATE" {
                predicate_name = if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    predicate_disabled = true;
                    None
                } else {
                    Some(symbol_name_string(scope.context(), scope.get(value).as_word())?)
                };
            } else if key_name == ":COPIER" || key_name == "COPIER" {
                copier_name = if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    copier_disabled = true;
                    None
                } else {
                    Some(symbol_name_string(scope.context(), scope.get(value).as_word())?)
                };
            } else if key_name == ":CONC-NAME" || key_name == "CONC-NAME" {
                conc_name = Some(symbol_name_string(scope.context(), scope.get(value).as_word())?);
            } else {
                return Err(ObjectError::TypeError);
            }
        }
        (name, 2)
    } else {
        (name_form, 2)
    };
    let name_word = scope.get(name).as_word();
    let name_text = symbol_name_string(scope.context(), name_word)?;
    let definition_package = {
        let package = ncl_object::symbol_package(scope.context(), name_word)?;
        let package_name = ncl_object::Package::from_word(package).name(scope.context())?;
        let length = ncl_object::string_length(scope.context(), package_name)?;
        (0..length)
            .map(|index| ncl_object::string_ref(scope.context(), package_name, index))
            .collect::<Result<String, _>>()?
    };
    let mut slot_specs = Vec::new();
    while let Some(slot) = parts.as_slice().get(option_index).copied() {
        let slot_word = scope.get(slot).as_word();
        let is_option = matches!(ncl_object::classify_object(scope.context(), slot_word), ObjectRef::Symbol(_))
            && symbol_name_string(scope.context(), slot_word)?.starts_with(':');
        if is_option { break; }
        if slot_word.is_cons() {
            let fields = macro_list_to_handles(&mut scope, slot)?;
            slot_specs.push(defstruct_slot_spec(&scope, &fields)?);
        } else {
            slot_specs.push((scope.get(slot).as_word(), Word::NIL, false));
        }
        option_index += 1;
    }
    let mut conc_name = conc_name.map_or_else(|| format!("{name_text}-"), |value| value);
    let mut predicate_name = predicate_name.map_or_else(|| format!("{name_text}-P"), |value| value);
    let mut copier_name = copier_name.map_or_else(|| format!("COPY-{name_text}"), |value| value);
    let mut index = option_index;
    while let Some(option) = parts.as_slice().get(index).copied() {
        let option_name = symbol_name_string(scope.context(), scope.get(option).as_word())?;
        let value = parts.as_slice().get(index + 1).copied().ok_or(ObjectError::TypeError)?;
        match option_name.as_str() {
            ":CONC-NAME" | "CONC-NAME" => conc_name = symbol_name_string(scope.context(), scope.get(value).as_word())?,
            ":PREDICATE" | "PREDICATE" => {
                if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    predicate_disabled = true;
                } else {
                    predicate_name = symbol_name_string(scope.context(), scope.get(value).as_word())?;
                }
            }
            ":COPIER" | "COPIER" => {
                if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    copier_disabled = true;
                } else {
                    copier_name = symbol_name_string(scope.context(), scope.get(value).as_word())?;
                }
            }
            ":INCLUDE" | "INCLUDE" => {
                include = scope.get(value).as_word();
                if include != Word::NIL && include.is_cons() {
                    let include_fields = macro_list_to_handles(&mut scope, value)?;
                    include = scope.get(
                        *include_fields.as_slice().first().ok_or(ObjectError::TypeError)?,
                    ).as_word();
                    for override_form in include_fields.as_slice().iter().skip(1).copied() {
                        let fields = macro_list_to_handles(&mut scope, override_form)?;
                        include_overrides.push(defstruct_slot_spec(&scope, &fields)?);
                    }
                }
            }
            ":TYPE" | "TYPE" => {
                let type_name = symbol_name_string(scope.context(), scope.get(value).as_word())?;
                if type_name != "STRUCTURE" { return Err(ObjectError::TypeError); }
            }
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown options
        }
        index += 2;
    }
    let include_name = if include == Word::NIL { None } else { Some(symbol_name_string(scope.context(), include)?) };
    let parent_layout = include_name
        .as_deref()
        .and_then(|name| runtime.structure_layout_for_name(name));
    let superclass = if let Some(include_name) = include_name.as_deref() {
        runtime.class(scope.context_mut(), include_name).ok_or(ObjectError::TypeError)?
    } else {
        runtime.class(scope.context_mut(), "STRUCTURE-OBJECT").unwrap_or(Word::NIL)
    };
    let mut direct_specs = slot_specs.clone();
    direct_specs.extend(include_overrides);
    let mut descriptors = Vec::new();
    for (slot, initform, read_only) in &direct_specs {
        let values = scope.root_many(&[
            ncl_object::Local::from_word(*slot),
            ncl_object::Local::from_word(*initform),
            ncl_object::Local::from_word(if *read_only { Word::TRUE } else { Word::NIL }),
        ]);
        descriptors.push(scope.make_simple_vector(runtime, &values)?);
    }
    let descriptor_values = descriptors.iter().map(|handle| ncl_object::Local::from_word(scope.get(*handle).as_word())).collect::<Vec<_>>();
    let rooted_descriptors = scope.root_many(&descriptor_values);
    let direct_slots = scope.make_simple_vector(runtime, &rooted_descriptors)?;
    let direct_slots_word = scope.get(direct_slots).as_word();
    let class = make_class(scope.context_mut(), runtime, name_word, superclass, direct_slots_word, Word::fixnum(1))?;
    let class: ncl_object::Handle<'_, Word> = scope.root(ncl_object::Local::from_word(class));
    let class_word = scope.get(class).as_word();
    runtime.define_class(scope.context_mut(), name_text.clone(), class_word)?;
    let effective = ncl_object::simple_vector_ref(scope.context(), class_word, CLASS_EFFECTIVE_SLOTS)?;
    let effective_count = ncl_object::simple_vector_length(scope.context(), effective)?;
    let layout = runtime.register_structure_layout(effective_count)?;
    runtime.register_structure_class_with_parent(layout, parent_layout, name_text.clone())?;
    let layout_word = Word::fixnum(i64::from(layout.as_u32()));

    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let defsetf = scope.intern(runtime, COMMON_LISP, "DEFSETF")?;
    let key_marker = scope.intern(runtime, COMMON_LISP, "&KEY")?;
    let make = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-MAKE")?;
    let predicate = (!predicate_disabled)
        .then(|| scope.intern(runtime, &definition_package, &predicate_name))
        .transpose()?;
    let copier = (!copier_disabled)
        .then(|| scope.intern(runtime, &definition_package, &copier_name))
        .transpose()?;
    let mut effective_names = Vec::new();
    for slot in 0..effective_count {
        let descriptor = ncl_object::simple_vector_ref(scope.context(), effective, slot)?;
        effective_names.push(ncl_object::simple_vector_ref(scope.context(), descriptor, 0)?);
    }
    let mut forms = scope.root_many(&[]);
    let mut lambda = scope.root_many(&[ncl_object::Local::from_word(scope.get(key_marker).as_word())]);
    for slot in &effective_names {
        let descriptor = ncl_object::simple_vector_ref(scope.context(), effective, effective_names.iter().position(|name| name == slot).ok_or(ObjectError::Layout)?)?;
        let initform = ncl_object::simple_vector_ref(scope.context(), descriptor, 1)?;
        if initform == Word::NIL {
            lambda.push(&mut scope, ncl_object::Local::from_word(*slot));
        } else {
            let binding_values = scope.root_many(&[
                ncl_object::Local::from_word(*slot),
                ncl_object::Local::from_word(initform),
            ]);
            let binding = scope.make_list(runtime, &binding_values)?;
            macro_push_handle(&mut scope, &mut lambda, binding);
        }
    }
    let mut body = scope.root_many(&[ncl_object::Local::from_word(scope.get(make).as_word()), ncl_object::Local::from_word(layout_word)]);
    for slot in &effective_names { body.push(&mut scope, ncl_object::Local::from_word(*slot)); }
    let constructor = scope.intern(runtime, &definition_package, &format!("MAKE-{name_text}"))?;
    let lambda_form = scope.make_list(runtime, &lambda)?;
    let body_form = scope.make_list(runtime, &body)?;
    let definition = make_form(&mut scope, runtime, &[defun, constructor, lambda_form, body_form])?;
    macro_push_handle(&mut scope, &mut forms, definition);
    let object = scope.intern(runtime, "NCL", "OBJECT")?;
    let structure_p = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-P")?;
    let layout_handle = scope.root(ncl_object::Local::from_word(layout_word));
    let object_word = scope.get(object).as_word();
    if let Some(predicate) = predicate {
        let predicate_args = scope.root_many(&[ncl_object::Local::from_word(object_word)]);
        let predicate_lambda = scope.make_list(runtime, &predicate_args)?;
        let predicate_body = make_form(&mut scope, runtime, &[structure_p, object, layout_handle])?;
        let predicate_definition = make_form(&mut scope, runtime, &[defun, predicate, predicate_lambda, predicate_body])?;
        macro_push_handle(&mut scope, &mut forms, predicate_definition);
    }
    if let Some(copier) = copier {
        let copier_args = scope.root_many(&[ncl_object::Local::from_word(object_word)]);
        let copier_lambda = scope.make_list(runtime, &copier_args)?;
        let structure_copy = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-COPY")?;
        let copier_body = make_form(&mut scope, runtime, &[structure_copy, object])?;
        let copier_definition = make_form(&mut scope, runtime, &[defun, copier, copier_lambda, copier_body])?;
        macro_push_handle(&mut scope, &mut forms, copier_definition);
    }
    for (index, slot) in effective_names.iter().enumerate() {
        let slot_name = symbol_name_string(scope.context(), *slot)?;
        let accessor = scope.intern(runtime, &definition_package, &format!("{conc_name}{slot_name}"))?;
        let object = scope.intern(runtime, "NCL", "OBJECT")?;
        let structure_ref = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-REF")?;
        let index_value = i64::try_from(index).map_err(|_| ObjectError::TypeError)?;
        let index_handle = scope.root(ncl_object::Local::from_word(Word::fixnum(index_value)));
        let accessor_body = make_form(&mut scope, runtime, &[structure_ref, object, index_handle])?;
        let accessor_args = scope.root_many(&[ncl_object::Local::from_word(object_word)]);
        let accessor_lambda = scope.make_list(runtime, &accessor_args)?;
        let accessor_definition = make_form(&mut scope, runtime, &[defun, accessor, accessor_lambda, accessor_body])?;
        macro_push_handle(&mut scope, &mut forms, accessor_definition);
        let descriptor = ncl_object::simple_vector_ref(scope.context(), effective, index)?;
        let read_only = ncl_object::simple_vector_ref(scope.context(), descriptor, 2)? != Word::NIL;
        if read_only {
            continue;
        }
        let store = scope.intern(runtime, "NCL", "STORE")?;
        let place_args = scope.root_many(&[ncl_object::Local::from_word(object_word)]);
        let place_lambda = scope.make_list(runtime, &place_args)?;
        let store_word = scope.get(store).as_word();
        let store_args = scope.root_many(&[ncl_object::Local::from_word(store_word)]);
        let store_lambda = scope.make_list(runtime, &store_args)?;
        let structure_set = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-SET")?;
        let set_index = scope.root(ncl_object::Local::from_word(Word::fixnum(index_value)));
        let set_body = make_form(&mut scope, runtime, &[structure_set, object, set_index, store])?;
        let set_definition = make_form(&mut scope, runtime, &[defsetf, accessor, place_lambda, store_lambda, set_body])?;
        macro_push_handle(&mut scope, &mut forms, set_definition);
    }
    macro_push_handle(&mut scope, &mut forms, name);
    make_progn(&mut scope, runtime, &forms).map(|result| scope.get(result).as_word())
}

fn defgeneric_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let form = scope.root(ncl_object::Local::from_word(args.required(0)?));
    let parts = macro_list_to_handles(&mut scope, form)?;
    let name = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let progn = scope.intern(runtime, COMMON_LISP, "PROGN")?;
    let define = scope.intern(runtime, COMMON_LISP, "%CLOS-DEFINE-GENERIC")?;
    let dispatch = scope.intern(runtime, COMMON_LISP, "%CLOS-DISPATCH")?;
    let args_symbol = scope.intern(runtime, COMMON_LISP, "ARGS")?;
    let rest = scope.intern(runtime, COMMON_LISP, "&REST")?;
    let quoted_name = quoted(&mut scope, runtime, name)?;
    let clear = make_form(&mut scope, runtime, &[define, quoted_name])?;
    let dispatch_call = make_form(&mut scope, runtime, &[dispatch, quoted_name, args_symbol])?;
    let lambda_list = make_form(&mut scope, runtime, &[rest, args_symbol])?;
    let function = make_form(&mut scope, runtime, &[defun, name, lambda_list, dispatch_call])?;
    let mut result = scope.root_many(&[]);
    macro_push_handle(&mut scope, &mut result, progn);
    for value in [clear, function, name] { // check-added-lines: allow(index) fixed form fields
        macro_push_handle(&mut scope, &mut result, value);
    }
    let expansion = scope.make_list(runtime, &result)?;
    Ok(scope.get(expansion).as_word())
}

static METHOD_FUNCTION_COUNTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[allow(clippy::too_many_lines)]
fn defmethod_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let form = scope.root(ncl_object::Local::from_word(args.required(0)?));
    let parts = macro_list_to_handles(&mut scope, form)?;
    let name = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let (qualifier_word, specializer_index) = match parts.as_slice().get(2).copied() {
        Some(value) => method_qualifier(scope.context(), scope.get(value).as_word())?.map_or(
            (Word::fixnum(METHOD_QUALIFIER_PRIMARY), 2),
            |qualifier| (qualifier, 3),
        ),
        None => return Err(ObjectError::TypeError),
    };
    let qualifier = scope.root(ncl_object::Local::from_word(qualifier_word));
    let specializer_form = *parts
        .as_slice()
        .get(specializer_index)
        .ok_or(ObjectError::TypeError)?;
    let specializer_fields = macro_list_to_handles(&mut scope, specializer_form)?;
    let mut user_lambda = scope.root_many(&[]);
    let mut lambda_parameters = scope.root_many(&[]);
    for field in specializer_fields.iter().copied() {
        if scope.get(field).as_word().is_cons() {
            let fields = macro_list_to_handles(&mut scope, field)?;
            let parameter = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
            macro_push_handle(&mut scope, &mut user_lambda, parameter);
            macro_push_handle(&mut scope, &mut lambda_parameters, parameter);
        } else if symbol_name_string(scope.context(), scope.get(field).as_word())? == "&REST" {
            macro_push_handle(&mut scope, &mut user_lambda, field);
            let parameter = *specializer_fields
                .as_slice()
                .get(user_lambda.len())
                .ok_or(ObjectError::TypeError)?;
            macro_push_handle(&mut scope, &mut user_lambda, parameter);
            macro_push_handle(&mut scope, &mut lambda_parameters, parameter);
        } else {
            return Err(ObjectError::TypeError);
        }
    }
    let dispatch_specializers = scope.make_list(runtime, &specializer_fields)?;
    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let dispatch = scope.intern(runtime, COMMON_LISP, "%CLOS-DISPATCH")?;
    let args_symbol = scope.intern(runtime, COMMON_LISP, "ARGS")?;
    let rest = scope.intern(runtime, COMMON_LISP, "&REST")?;
    let body_start = specializer_index + 1;
    let body_forms = scope.root_many(
        &parts
            .as_slice()
            .get(body_start..)
            .ok_or(ObjectError::TypeError)?
            .iter()
            .map(|handle| ncl_object::Local::from_word(scope.get(*handle).as_word()))
            .collect::<Vec<_>>(),
    );
    let method_name = {
        let id = METHOD_FUNCTION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        scope.intern(runtime, "NCL", &format!("%CLOS-METHOD-{id}"))?
    };
    let function_operator = scope.intern(runtime, COMMON_LISP, "FUNCTION")?;
    let method_function = make_form(&mut scope, runtime, &[function_operator, method_name])?;
    let tag = scope.intern(runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
    let definition = make_form(
        &mut scope,
        runtime,
        &[tag, qualifier, dispatch_specializers],
    )?;
    let quoted_specializers = quoted(&mut scope, runtime, definition)?;
    let add_method = scope.intern(runtime, COMMON_LISP, "%CLOS-ADD-METHOD")?;
    let quoted_name = quoted(&mut scope, runtime, name)?;
    let registration = make_form(
        &mut scope,
        runtime,
        &[add_method, quoted_name, quoted_specializers, method_function],
    )?;
    let next_methods = gensym(&mut scope, runtime)?;
    let method_body = rewrite_method_body(
        &mut scope,
        runtime,
        &body_forms,
        next_methods,
        &lambda_parameters,
    )?;
    let method_lambda_list = method_lambda_list(&mut scope, runtime, next_methods, &user_lambda)?;
    let method_definition = make_form(
        &mut scope,
        runtime,
        &[defun, method_name, method_lambda_list, method_body],
    )?;
    let dispatch_call = make_form(&mut scope, runtime, &[dispatch, quoted_name, args_symbol])?;
    let wrapper_lambda = make_form(&mut scope, runtime, &[rest, args_symbol])?;
    let wrapper = make_form(&mut scope, runtime, &[defun, name, wrapper_lambda, dispatch_call])?;
    let initialization_base = initialization_base(&mut scope, runtime, name, quoted_name)?;
    let progn = scope.intern(runtime, COMMON_LISP, "PROGN")?;
    let mut result = scope.root_many(&[]);
    macro_push_handle(&mut scope, &mut result, progn);
    if let Some(base) = initialization_base { // check-added-lines: allow(index)
        macro_push_handle(&mut scope, &mut result, base);
    }
    for value in [wrapper, method_definition, registration, name] { // check-added-lines: allow(index) fixed form fields
        macro_push_handle(&mut scope, &mut result, value);
    }
    let expansion = scope.make_list(runtime, &result)?;
    Ok(scope.get(expansion).as_word())
}

fn make_form<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    values: &[ncl_object::Handle<'ctx>],
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let mut roots = scope.root_many(&[]);
    for value in values {
        macro_push_handle(scope, &mut roots, *value);
    }
    scope.make_list(runtime, &roots)
}

fn macro_list_to_handles<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::HandleVec<'ctx>, ObjectError> {
    let word = scope.get(handle).as_word();
    scope.list_to_handle_vec(ncl_object::Local::from_word(word))
}

fn macro_push_handle<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    values: &mut ncl_object::HandleVec<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) {
    let word = scope.get(handle).as_word();
    values.push(scope, ncl_object::Local::from_word(word));
}

fn gensym<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let package = runtime.ensure_package(scope.context_mut(), "NCL")?;
    let symbol = ncl_object::Package::from_word(package).gensym(scope.context_mut(), runtime)?;
    Ok(scope.root(ncl_object::Local::from_word(symbol)))
}

fn initialization_base<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    quoted_name: ncl_object::Handle<'ctx>,
) -> Result<Option<ncl_object::Handle<'ctx>>, ObjectError> {
    let name_text = symbol_name_string(scope.context(), scope.get(name).as_word())?;
    if name_text != "INITIALIZE-INSTANCE" && name_text != "SHARED-INITIALIZE" {
        return Ok(None);
    }
    let ensure = scope.intern(runtime, COMMON_LISP, "%CLOS-ENSURE-INITIALIZATION-BASE")?;
    Ok(Some(make_form(scope, runtime, &[ensure, quoted_name])?))
}

fn method_lambda_list<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    next_methods: ncl_object::Handle<'ctx>,
    user_args: &ncl_object::HandleVec<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let mut values = scope.root_many(&[]);
    macro_push_handle(scope, &mut values, next_methods);
    for value in user_args.iter().copied() {
        macro_push_handle(scope, &mut values, value);
    }
    scope.make_list(runtime, &values)
}

struct MethodRewrite<'ctx> {
    next_methods: ncl_object::Handle<'ctx>,
    current_args: ncl_object::HandleVec<'ctx>,
    call_next_name: ncl_object::Handle<'ctx>,
    call_next_user_name: ncl_object::Handle<'ctx>,
    next_method_p_name: ncl_object::Handle<'ctx>,
    next_method_p_user_name: ncl_object::Handle<'ctx>,
    list_name: ncl_object::Handle<'ctx>,
    call_next: ncl_object::Handle<'ctx>,
    next_method_p: ncl_object::Handle<'ctx>,
    quote_name: ncl_object::Handle<'ctx>,
    defun_name: ncl_object::Handle<'ctx>,
    lambda_name: ncl_object::Handle<'ctx>,
}

fn rewrite_method_body<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    forms: &ncl_object::HandleVec<'ctx>,
    next_methods: ncl_object::Handle<'ctx>,
    current_args: &ncl_object::HandleVec<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let rewrite = MethodRewrite {
        next_methods,
        current_args: current_args.clone(),
        call_next_name: scope.intern(runtime, "COMMON-LISP-USER", "CALL-NEXT-METHOD")?,
        call_next_user_name: scope.intern(runtime, COMMON_LISP, "CALL-NEXT-METHOD")?,
        next_method_p_name: scope.intern(runtime, "COMMON-LISP-USER", "NEXT-METHOD-P")?,
        next_method_p_user_name: scope.intern(runtime, COMMON_LISP, "NEXT-METHOD-P")?,
        list_name: scope.intern(runtime, COMMON_LISP, "LIST")?,
        call_next: scope.intern(runtime, COMMON_LISP, "%CLOS-CALL-NEXT-METHOD")?,
        next_method_p: scope.intern(runtime, COMMON_LISP, "%CLOS-NEXT-METHOD-P")?,
        quote_name: scope.intern(runtime, COMMON_LISP, "QUOTE")?,
        defun_name: scope.intern(runtime, COMMON_LISP, "DEFUN")?,
        lambda_name: scope.intern(runtime, COMMON_LISP, "LAMBDA")?,
    };
    let mut rewritten = scope.root_many(&[]);
    for form in forms.iter().copied() {
        let value = rewrite_method_form(scope, runtime, form, &rewrite)?;
        macro_push_handle(scope, &mut rewritten, value);
    }
    make_progn(scope, runtime, &rewritten)
}

fn rewrite_method_form<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    form: ncl_object::Handle<'ctx>,
    rewrite: &MethodRewrite<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    if !scope.get(form).as_word().is_cons() {
        return Ok(form);
    }
    let fields = macro_list_to_handles(scope, form)?;
    let operator = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
    let operator_word = scope.get(operator).as_word();
    if operator_word == scope.get(rewrite.quote_name).as_word()
        || operator_word == scope.get(rewrite.defun_name).as_word()
        || operator_word == scope.get(rewrite.lambda_name).as_word()
    {
        return Ok(form);
    }
    if operator_word == scope.get(rewrite.call_next_name).as_word()
        || operator_word == scope.get(rewrite.call_next_user_name).as_word()
    {
        let mut supplied = scope.root_many(&[]);
        if fields.len() == 1 {
            for value in rewrite.current_args.iter().copied() {
                macro_push_handle(scope, &mut supplied, value);
            }
        } else {
            for value in fields.as_slice().iter().skip(1).copied() {
                let rewritten = rewrite_method_form(scope, runtime, value, rewrite)?;
                macro_push_handle(scope, &mut supplied, rewritten);
            }
        }
        let mut supplied_form = scope.root_many(&[]);
        macro_push_handle(scope, &mut supplied_form, rewrite.list_name);
        for value in supplied.iter().copied() {
            macro_push_handle(scope, &mut supplied_form, value);
        }
        let supplied = scope.make_list(runtime, &supplied_form)?;
        return make_form(scope, runtime, &[rewrite.call_next, rewrite.next_methods, supplied]);
    }
    if operator_word == scope.get(rewrite.next_method_p_name).as_word()
        || operator_word == scope.get(rewrite.next_method_p_user_name).as_word()
    {
        return make_form(scope, runtime, &[rewrite.next_method_p, rewrite.next_methods]);
    }
    let mut rewritten = scope.root_many(&[]);
    for field in fields.iter().copied() {
        let value = rewrite_method_form(scope, runtime, field, rewrite)?;
        macro_push_handle(scope, &mut rewritten, value);
    }
    scope.make_list(runtime, &rewritten)
}
