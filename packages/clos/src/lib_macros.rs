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
    let mut direct_superclasses = scope.root_many(&[]);
    for superclass in supers.iter().copied() {
        let superclass_word = scope.get(superclass).as_word();
        let class = class_designator(scope.context_mut(), runtime, superclass_word)?;
        let class = scope.root(ncl_object::Local::from_word(class));
        macro_push_handle(&mut scope, &mut direct_superclasses, class);
    }
    let superclass = if direct_superclasses.is_empty() {
        let standard_object = scope.intern(runtime, COMMON_LISP, "STANDARD-OBJECT")?;
        let standard_object_word = scope.get(standard_object).as_word();
        let standard_object_class =
            class_designator(scope.context_mut(), runtime, standard_object_word)?;
        scope.root(ncl_object::Local::from_word(standard_object_class))
    } else if direct_superclasses.len() == 1 {
        let parent_handle = *direct_superclasses
            .as_slice()
            .first()
            .ok_or(ObjectError::Layout)?;
        let parent = scope.get(parent_handle).as_word();
        scope.root(ncl_object::Local::from_word(parent))
    } else {
        scope.make_list(runtime, &direct_superclasses)?
    };
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
    let class: ncl_object::Handle<'_, Word> = scope.root(ncl_object::Local::from_word(class));
    let class_word = scope.get(class).as_word();
    runtime.define_class(scope.context_mut(), class_name, class_word)?;
    let mut definitions = Vec::new();
    for (accessor, slot) in accessors {
        definitions.push(make_accessor_definition(
            &mut scope, runtime, accessor, slot,
        )?);
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
    let Some(generic_lambda_list) = parts.as_slice().get(2).copied() else {
        return defgeneric_error_expansion(&mut scope, runtime, name);
    };
    if macro_list_to_handles(&mut scope, generic_lambda_list).is_err() {
        return defgeneric_error_expansion(&mut scope, runtime, name);
    }

    let (progn, quoted_name, ensure_generic) =
        defgeneric_setup(&mut scope, runtime, name, generic_lambda_list)?;

    let method_keyword = scope.intern(runtime, "KEYWORD", "METHOD")?;
    let standard_options = [
        "ARGUMENT-PRECEDENCE-ORDER",
        "DECLARE",
        "DOCUMENTATION",
        "GENERIC-FUNCTION-CLASS",
        "METHOD-CLASS",
        "METHOD-COMBINATION",
    ]
    .into_iter()
    .map(|option| scope.intern(runtime, "KEYWORD", option))
    .collect::<Result<Vec<_>, _>>()?;
    let defmethod = scope.intern(runtime, COMMON_LISP, "DEFMETHOD")?;
    let mut method_forms = scope.root_many(&[]);
    for option in parts.iter().copied().skip(3) {
        if !scope.get(option).as_word().is_cons() {
            return defgeneric_error_expansion(&mut scope, runtime, name);
        }
        let Ok(fields) = macro_list_to_handles(&mut scope, option) else {
            return defgeneric_error_expansion(&mut scope, runtime, name);
        };
        let Some(option_name) = fields.as_slice().first().copied() else {
            return defgeneric_error_expansion(&mut scope, runtime, name);
        };
        let option_word = scope.get(option_name).as_word();
        if option_word == scope.get(method_keyword).as_word() {
            let mut method_form_values = scope.root_many(&[]);
            macro_push_handle(&mut scope, &mut method_form_values, defmethod);
            macro_push_handle(&mut scope, &mut method_form_values, name);
            for field in fields.iter().copied().skip(1) {
                macro_push_handle(&mut scope, &mut method_form_values, field);
            }
            let Ok(method_form) = scope.make_list(runtime, &method_form_values) else {
                return defgeneric_error_expansion(&mut scope, runtime, name);
            };
            let Ok(expansion) = defmethod_macro_expansion(&mut scope, runtime, method_form, true)
            else {
                return defgeneric_error_expansion(&mut scope, runtime, name);
            };
            let Ok(expansion_forms) = macro_list_to_handles(&mut scope, expansion) else {
                return defgeneric_error_expansion(&mut scope, runtime, name);
            };
            for expanded_form in expansion_forms.iter().copied().skip(1) {
                macro_push_handle(&mut scope, &mut method_forms, expanded_form);
            }
        } else if !standard_options
            .iter()
            .any(|option| scope.get(*option).as_word() == option_word)
        {
            return defgeneric_error_expansion(&mut scope, runtime, name);
        }
    }

    let mut result = scope.root_many(&[]);
    macro_push_handle(&mut scope, &mut result, progn);
    // The trailing element is this `progn`'s (and so `defgeneric`'s own)
    // result: CLHS requires the generic function *name* back, so it must be
    // the quoted symbol, not a bare one (which would compile as a read of
    // the variable of that name, which `defgeneric` never binds).
    // check-added-lines: allow(index) fixed form fields
    macro_push_handle(&mut scope, &mut result, ensure_generic);
    for method_form in method_forms.iter().copied() {
        macro_push_handle(&mut scope, &mut result, method_form);
    }
    macro_push_handle(&mut scope, &mut result, quoted_name);
    let expansion = scope.make_list(runtime, &result)?;
    Ok(scope.get(expansion).as_word())
}

fn defgeneric_setup<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    generic_lambda_list: ncl_object::Handle<'ctx>,
) -> Result<
    (
        ncl_object::Handle<'ctx>,
        ncl_object::Handle<'ctx>,
        ncl_object::Handle<'ctx>,
    ),
    ObjectError,
> {
    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let progn = scope.intern(runtime, COMMON_LISP, "PROGN")?;
    let define = scope.intern(runtime, COMMON_LISP, "%CLOS-DEFINE-GENERIC")?;
    let redefine = scope.intern(runtime, COMMON_LISP, "%CLOS-REDEFINE-GENERIC")?;
    let dispatch = scope.intern(runtime, COMMON_LISP, "%CLOS-DISPATCH")?;
    let fboundp = scope.intern(runtime, COMMON_LISP, "FBOUNDP")?;
    let if_symbol = scope.intern(runtime, COMMON_LISP, "IF")?;
    let args_symbol = scope.intern(runtime, COMMON_LISP, "ARGS")?;
    let rest = scope.intern(runtime, COMMON_LISP, "&REST")?;
    let quoted_name = quoted(scope, runtime, name)?;
    let quoted_lambda_list = quoted(scope, runtime, generic_lambda_list)?;
    let clear = make_form(scope, runtime, &[define, quoted_name, quoted_lambda_list])?;
    let redefine_call = make_form(scope, runtime, &[redefine, quoted_name, quoted_lambda_list])?;
    let dispatch_call = make_form(scope, runtime, &[dispatch, quoted_name, args_symbol])?;
    let lambda_list = make_form(scope, runtime, &[rest, args_symbol])?;
    let function = make_form(scope, runtime, &[defun, name, lambda_list, dispatch_call])?;
    let new_generic = make_form(scope, runtime, &[progn, function, clear])?;
    let function_is_bound = make_form(scope, runtime, &[fboundp, quoted_name])?;
    let ensure_generic = make_form(
        scope,
        runtime,
        &[if_symbol, function_is_bound, redefine_call, new_generic],
    )?;
    Ok((progn, quoted_name, ensure_generic))
}

fn defgeneric_error_expansion<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<Word, ObjectError> {
    let error = scope.intern(runtime, COMMON_LISP, "%CLOS-DEFGENERIC-ERROR")?;
    let quoted_name = quoted(scope, runtime, name)?;
    let expansion = make_form(scope, runtime, &[error, quoted_name])?;
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
