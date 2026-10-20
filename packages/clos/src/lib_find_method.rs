const ERRORP_ARGUMENT: ncl_object::Parameter = ncl_object::Parameter {
    name: BuiltinName::new("ERRORP"),
    ty: ncl_object::ParameterType::Any,
};

const FIND_METHOD_ARGUMENTS: &[ncl_object::Parameter] = &[
    ncl_object::Parameter {
        name: BuiltinName::new("GENERIC-FUNCTION"),
        ty: ncl_object::ParameterType::Any,
    },
    ncl_object::Parameter {
        name: BuiltinName::new("METHOD-QUALIFIERS"),
        ty: ncl_object::ParameterType::Any,
    },
    ncl_object::Parameter {
        name: BuiltinName::new("SPECIALIZERS"),
        ty: ncl_object::ParameterType::Any,
    },
];

fn allocate_standard_method<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let class = runtime
        .class(scope.context_mut(), "STANDARD-METHOD")
        .filter(|class| *class != Word::UNBOUND && *class != Word::fixnum(1)) // check-added-lines: allow(unbound) class registry placeholders
        .ok_or(ObjectError::Layout)?;
    let class: ncl_object::Handle<'_, Word> = scope.root(Local::from_word(class));
    let class_word = scope.get(class).as_word();
    let method = allocate_instance(scope.context_mut(), runtime, class_word, &[])?;
    Ok(scope.root(Local::from_word(method.as_word())))
}

fn find_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let function = scope.root(Local::from_word(args.required(0)?));
    let qualifiers: ncl_object::Handle<'_, Word> = scope.root(Local::from_word(args.required(1)?));
    let specializers: ncl_object::Handle<'_, Word> =
        scope.root(Local::from_word(args.required(2)?));
    let errorp: ncl_object::Handle<'_, Word> =
        scope.root(Local::from_word(args.get(3).unwrap_or(Word::TRUE)));
    let function_word = scope.get(function).as_word();
    if !matches!(
        classify_object(scope.context(), function_word),
        ObjectRef::Function(_) | ObjectRef::Closure(_)
    ) {
        return find_method_type_error(
            scope.context_mut(),
            runtime,
            function_word,
            "GENERIC-FUNCTION",
        );
    }
    let Some(name) = generic_function_name(&mut scope, runtime, function)? else {
        return find_method_type_error(
            scope.context_mut(),
            runtime,
            function_word,
            "GENERIC-FUNCTION",
        );
    };
    if !has_method_registry(&mut scope, runtime, name)? {
        return find_method_type_error(
            scope.context_mut(),
            runtime,
            function_word,
            "GENERIC-FUNCTION",
        );
    }

    let supplied_specializer_list = scope.get(specializers).as_word();
    let supplied_specializers =
        scope.list_to_handle_vec(Local::from_word(supplied_specializer_list))?;
    let required_arity =
        generic_required_arity(&mut scope, runtime, name)?.ok_or(ObjectError::Layout)?;
    if supplied_specializers.len() != required_arity {
        return Err(wrong_specializer_count(scope.context_mut(), required_arity));
    }
    let supplied_words = supplied_specializers
        .iter()
        .map(|specializer| scope.get(*specializer).as_word())
        .collect::<Vec<_>>();
    let registry = method_registry(&mut scope, runtime, name)?;
    let entries = dispatch_list_to_handles(&mut scope, registry)?;
    for entry in entries.iter().copied() {
        let fields = dispatch_list_to_handles(&mut scope, entry)?;
        let method_specializers = *fields.as_slice().first().ok_or(ObjectError::Layout)?;
        let method_qualifiers = *fields.as_slice().get(4).ok_or(ObjectError::Layout)?;
        let supplied_qualifiers = scope.get(qualifiers).as_word();
        let method_qualifier_list = scope.get(method_qualifiers).as_word();
        if !lisp_equal_word(
            scope.context(),
            supplied_qualifiers,
            method_qualifier_list,
            0,
        )? {
            continue;
        }
        let method_specializers_word = scope.get(method_specializers).as_word();
        if specializers_match(
            scope.context_mut(),
            runtime,
            method_specializers_word,
            &supplied_words,
        )? {
            let method = *fields.as_slice().get(3).ok_or(ObjectError::Layout)?;
            return Ok(scope.get(method).as_word());
        }
    }
    if scope.get(errorp).as_word() == Word::NIL {
        Ok(Word::NIL)
    } else {
        Err(ObjectError::UndefinedFunction)
    }
}

const fn wrong_specializer_count(ctx: &mut ThreadContext, expected: usize) -> ObjectError {
    ctx.set_pending_lisp_error(ncl_object::LispError::ProgramError(
        ncl_object::ProgramError::WrongNumberOfArguments {
            minimum: expected,
            maximum: Some(expected),
        },
    ));
    ObjectError::TypeError
}

fn specializers_match(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    stored: Word,
    supplied: &[Word],
) -> Result<bool, ObjectError> {
    let stored = form_elements(ctx, stored)?;
    if stored.len() != supplied.len() {
        return Ok(false);
    }
    for (stored, supplied) in stored.iter().zip(supplied.iter().copied()) {
        let fields = form_elements(ctx, *stored)?;
        let designator = *fields.get(1).ok_or(ObjectError::TypeError)?;
        if let Some(stored_value) = dispatch_eql_specializer(ctx, designator)? {
            let Some(supplied_value) = eql_specializer_object(ctx, supplied)? else {
                return Ok(false);
            };
            if !ncl_lib_numbers::eql_words(ctx, stored_value, supplied_value) {
                return Ok(false);
            }
        } else {
            let stored_class = class_designator(ctx, runtime, designator)?;
            let supplied_class = class_designator(ctx, runtime, supplied)?;
            if stored_class != supplied_class {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn method_specializers_equal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    left: Word,
    right: Word,
) -> Result<bool, ObjectError> {
    let left = form_elements(ctx, left)?;
    let right = form_elements(ctx, right)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (left, right) in left.iter().zip(&right) {
        let left = form_elements(ctx, *left)?;
        let right = form_elements(ctx, *right)?;
        let left_designator = *left.get(1).ok_or(ObjectError::TypeError)?;
        let right_designator = *right.get(1).ok_or(ObjectError::TypeError)?;
        match (
            dispatch_eql_specializer(ctx, left_designator)?,
            dispatch_eql_specializer(ctx, right_designator)?,
        ) {
            (Some(left), Some(right)) if ncl_lib_numbers::eql_words(ctx, left, right) => {}
            (Some(_), _) | (_, Some(_)) => return Ok(false),
            (None, None) => {
                if class_designator(ctx, runtime, left_designator)?
                    != class_designator(ctx, runtime, right_designator)?
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn eql_specializer_object(ctx: &ThreadContext, value: Word) -> Result<Option<Word>, ObjectError> {
    if !matches!(classify_object(ctx, value), ObjectRef::SimpleVector(_))
        || simple_vector_length(ctx, value)? != 2
        || simple_vector_ref(ctx, value, 0)? != Word::fixnum(1)
    {
        return Ok(None);
    }
    Ok(Some(simple_vector_ref(ctx, value, 1)?))
}

fn lisp_equal_word(
    ctx: &ThreadContext,
    left: Word,
    right: Word,
    depth: usize,
) -> Result<bool, ObjectError> {
    if ncl_lib_numbers::eql_words(ctx, left, right) {
        return Ok(true);
    }
    if depth >= 64 {
        return Ok(false);
    }
    match (classify_object(ctx, left), classify_object(ctx, right)) {
        (ObjectRef::Cons(_), ObjectRef::Cons(_)) => {
            Ok(
                lisp_equal_word(ctx, car(ctx, left)?, car(ctx, right)?, depth + 1)?
                    && lisp_equal_word(ctx, cdr(ctx, left)?, cdr(ctx, right)?, depth + 1)?,
            )
        }
        (ObjectRef::String(_), ObjectRef::String(_)) => {
            let length = ncl_object::string_length(ctx, left)?;
            if length != ncl_object::string_length(ctx, right)? {
                return Ok(false);
            }
            for index in 0..length {
                if ncl_object::string_ref(ctx, left, index)?
                    != ncl_object::string_ref(ctx, right, index)?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false), // check-added-lines: allow(wildcard) non-string objects cannot be names
    }
}

fn find_method_type_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut datum: Word,
    expected_name: &str,
) -> Result<Word, ObjectError> {
    ncl_object::with_root(ctx, &mut datum, |ctx, datum| {
        let mut package = runtime.ensure_package(ctx, COMMON_LISP)?;
        ncl_object::with_root(ctx, &mut package, |ctx, package| {
            let (mut expected, _) =
                Package::from_word(package.get()).intern(ctx, runtime, expected_name)?;
            ncl_object::with_root(ctx, &mut expected, |ctx, expected| {
                let slots = [
                    ncl_conditions::ConditionSlotValue::from_word(datum.get()),
                    ncl_conditions::ConditionSlotValue::from_word(expected.get()),
                ];
                let mut condition = ncl_conditions::make_typed_condition(
                    ctx,
                    runtime,
                    ncl_conditions::ConditionIdentifier::TypeError,
                    &slots,
                )
                .map_err(|error| match error {
                    ncl_conditions::ConditionError::Object(error) => error,
                    _ => ObjectError::Layout, // check-added-lines: allow(wildcard) non-object condition failures cannot build this condition
                })?
                .as_word();
                ncl_object::with_root(ctx, &mut condition, |ctx, condition| {
                    ctx.set_pending_condition(condition.get());
                    Err(ObjectError::TypeError)
                })
            })
        })
    })
}
