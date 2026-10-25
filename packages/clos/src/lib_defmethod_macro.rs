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
    let expansion = defmethod_macro_expansion(&mut scope, runtime, form, None, false)?;
    Ok(scope.get(expansion).as_word())
}

#[allow(clippy::too_many_lines)]
fn defmethod_macro_expansion<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    form: ncl_object::Handle<'ctx>,
    force_wrapper: Option<bool>,
    is_defgeneric_method: bool,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let parts = macro_list_to_handles(scope, form)?;
    let name = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let mut specializer_index = 2;
    let mut qualifier_handles = scope.root_many(&[]);
    while let Some(value) = parts.as_slice().get(specializer_index).copied() {
        if scope.get(value).as_word() == Word::NIL || scope.get(value).as_word().is_cons() {
            break;
        }
        macro_push_handle(scope, &mut qualifier_handles, value);
        specializer_index += 1;
    }
    if qualifier_handles.len() > 1 {
        return Err(ObjectError::TypeError);
    }
    let qualifier_word = match qualifier_handles.as_slice().first() {
        Some(value) => method_qualifier(scope.context(), scope.get(*value).as_word())?
            .ok_or(ObjectError::TypeError)?,
        None => Word::fixnum(METHOD_QUALIFIER_PRIMARY),
    };
    let qualifier = scope.root(ncl_object::Local::from_word(qualifier_word));
    let qualifier_list = scope.make_list(runtime, &qualifier_handles)?;
    let specializer_form = *parts
        .as_slice()
        .get(specializer_index)
        .ok_or(ObjectError::TypeError)?;
    let specializer_fields = macro_list_to_handles(scope, specializer_form)?;
    let rest = scope.intern(runtime, COMMON_LISP, "&REST")?;
    let optional = scope.intern(runtime, COMMON_LISP, "&OPTIONAL")?;
    let key = scope.intern(runtime, COMMON_LISP, "&KEY")?;
    let allow_other_keys = scope.intern(runtime, COMMON_LISP, "&ALLOW-OTHER-KEYS")?;
    let aux = scope.intern(runtime, COMMON_LISP, "&AUX")?;
    let mut user_lambda = scope.root_many(&[]);
    let mut dispatch_specializers = scope.root_many(&[]);
    let mut lambda_list_started = false;
    for field in specializer_fields.iter().copied() {
        let field_word = scope.get(field).as_word();
        if lambda_list_started {
            macro_push_handle(scope, &mut user_lambda, field);
        } else if field_word == scope.get(optional).as_word()
            || field_word == scope.get(rest).as_word()
            || field_word == scope.get(key).as_word()
            || field_word == scope.get(allow_other_keys).as_word()
            || field_word == scope.get(aux).as_word()
        {
            macro_push_handle(scope, &mut user_lambda, field);
            lambda_list_started = true;
        } else if field_word.is_cons() {
            let fields = macro_list_to_handles(scope, field)?;
            let parameter = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
            macro_push_handle(scope, &mut user_lambda, parameter);
            macro_push_handle(scope, &mut dispatch_specializers, field);
        } else {
            macro_push_handle(scope, &mut user_lambda, field);
            let t = scope.intern(runtime, COMMON_LISP, "T")?;
            let mut specializer = scope.root_many(&[]);
            macro_push_handle(scope, &mut specializer, field);
            macro_push_handle(scope, &mut specializer, t);
            let specializer = scope.make_list(runtime, &specializer)?;
            macro_push_handle(scope, &mut dispatch_specializers, specializer);
        }
    }
    let dispatch_specializers = scope.make_list(runtime, &dispatch_specializers)?;
    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let dispatch = scope.intern(runtime, COMMON_LISP, "%CLOS-DISPATCH")?;
    let args_symbol = scope.intern(runtime, COMMON_LISP, "ARGS")?;
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
    let method_function = make_form(scope, runtime, &[function_operator, method_name])?;
    let tag = scope.intern(runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
    let origin = scope.root(ncl_object::Local::from_word(if is_defgeneric_method {
        Word::fixnum(1)
    } else {
        Word::NIL
    }));
    let definition = make_form(
        scope,
        runtime,
        &[
            tag,
            qualifier,
            qualifier_list,
            dispatch_specializers,
            origin,
        ],
    )?;
    let quoted_specializers = quoted(scope, runtime, definition)?;
    let add_method = scope.intern(runtime, COMMON_LISP, "%CLOS-ADD-METHOD")?;
    let quoted_name = quoted(scope, runtime, name)?;
    let registration = make_form(
        scope,
        runtime,
        &[
            add_method,
            quoted_name,
            quoted_specializers,
            method_function,
        ],
    )?;
    let next_methods = gensym(scope, runtime)?;
    let current_arguments = gensym(scope, runtime)?;
    let method_body =
        rewrite_method_body(scope, runtime, &body_forms, next_methods, current_arguments)?;
    let method_lambda_list = method_lambda_list(
        scope,
        runtime,
        next_methods,
        current_arguments,
        &user_lambda,
    )?;
    let method_definition = make_form(
        scope,
        runtime,
        &[defun, method_name, method_lambda_list, method_body],
    )?;
    let dispatch_call = make_form(scope, runtime, &[dispatch, quoted_name, args_symbol])?;
    let wrapper_lambda = make_form(scope, runtime, &[rest, args_symbol])?;
    let wrapper = make_form(
        scope,
        runtime,
        &[defun, name, wrapper_lambda, dispatch_call],
    )?;
    let initialization_base = initialization_base(scope, runtime, name, quoted_name)?;
    let progn = scope.intern(runtime, COMMON_LISP, "PROGN")?;
    let generic_initialization = if force_wrapper.is_none() {
        let generic_lambda_list = scope.make_list(runtime, &user_lambda)?;
        let (_, _, ensure_generic) = defgeneric_setup(scope, runtime, name, generic_lambda_list)?;
        let registered =
            scope.intern(runtime, COMMON_LISP, "%CLOS-GENERIC-FUNCTION-REGISTERED-P")?;
        let if_symbol = scope.intern(runtime, COMMON_LISP, "IF")?;
        let is_registered = make_form(scope, runtime, &[registered, quoted_name])?;
        let nil = scope.root(ncl_object::Local::from_word(Word::NIL));
        Some(make_form(
            scope,
            runtime,
            &[if_symbol, is_registered, nil, ensure_generic],
        )?)
    } else {
        None
    };
    let wrapper_form = match force_wrapper {
        Some(true) => Some(wrapper),
        Some(false) => None,
        None => {
            let registered =
                scope.intern(runtime, COMMON_LISP, "%CLOS-GENERIC-FUNCTION-REGISTERED-P")?;
            let if_symbol = scope.intern(runtime, COMMON_LISP, "IF")?;
            let is_registered = make_form(scope, runtime, &[registered, quoted_name])?;
            let nil = scope.root(ncl_object::Local::from_word(Word::NIL));
            let condition = make_form(scope, runtime, &[if_symbol, is_registered, nil, wrapper])?;
            Some(condition)
        }
    };
    let mut result = scope.root_many(&[]);
    macro_push_handle(scope, &mut result, progn);
    if let Some(generic_initialization) = generic_initialization {
        macro_push_handle(scope, &mut result, generic_initialization);
    }
    if let Some(base) = initialization_base {
        // check-added-lines: allow(index)
        macro_push_handle(scope, &mut result, base);
    }
    if let Some(wrapper) = wrapper_form {
        macro_push_handle(scope, &mut result, wrapper);
    }
    macro_push_handle(scope, &mut result, method_definition);
    macro_push_handle(scope, &mut result, registration);
    scope.make_list(runtime, &result)
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
    let name_word = scope.get(name).as_word();
    let initialize_instance = scope.intern(runtime, COMMON_LISP, "INITIALIZE-INSTANCE")?;
    let shared_initialize = scope.intern(runtime, COMMON_LISP, "SHARED-INITIALIZE")?;
    if name_word != scope.get(initialize_instance).as_word()
        && name_word != scope.get(shared_initialize).as_word()
    {
        return Ok(None);
    }
    let ensure = scope.intern(runtime, COMMON_LISP, "%CLOS-ENSURE-INITIALIZATION-BASE")?;
    Ok(Some(make_form(scope, runtime, &[ensure, quoted_name])?))
}

fn method_lambda_list<'ctx>(
    scope: &mut ncl_object::Scope<'ctx>,
    runtime: &Runtime,
    next_methods: ncl_object::Handle<'ctx>,
    current_arguments: ncl_object::Handle<'ctx>,
    user_args: &ncl_object::HandleVec<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let mut values = scope.root_many(&[]);
    macro_push_handle(scope, &mut values, next_methods);
    macro_push_handle(scope, &mut values, current_arguments);
    for value in user_args.iter().copied() {
        macro_push_handle(scope, &mut values, value);
    }
    scope.make_list(runtime, &values)
}

struct MethodRewrite<'ctx> {
    next_methods: ncl_object::Handle<'ctx>,
    current_arguments: ncl_object::Handle<'ctx>,
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
    current_arguments: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let rewrite = MethodRewrite {
        next_methods,
        current_arguments,
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
        if fields.len() == 1 {
            return make_form(
                scope,
                runtime,
                &[
                    rewrite.call_next,
                    rewrite.next_methods,
                    rewrite.current_arguments,
                ],
            );
        }
        let mut supplied = scope.root_many(&[]);
        for value in fields.as_slice().iter().skip(1).copied() {
            let rewritten = rewrite_method_form(scope, runtime, value, rewrite)?;
            macro_push_handle(scope, &mut supplied, rewritten);
        }
        let mut supplied_form = scope.root_many(&[]);
        macro_push_handle(scope, &mut supplied_form, rewrite.list_name);
        for value in supplied.iter().copied() {
            macro_push_handle(scope, &mut supplied_form, value);
        }
        let supplied = scope.make_list(runtime, &supplied_form)?;
        return make_form(
            scope,
            runtime,
            &[rewrite.call_next, rewrite.next_methods, supplied],
        );
    }
    if operator_word == scope.get(rewrite.next_method_p_name).as_word()
        || operator_word == scope.get(rewrite.next_method_p_user_name).as_word()
    {
        return make_form(
            scope,
            runtime,
            &[rewrite.next_method_p, rewrite.next_methods],
        );
    }
    let mut rewritten = scope.root_many(&[]);
    for field in fields.iter().copied() {
        let value = rewrite_method_form(scope, runtime, field, rewrite)?;
        macro_push_handle(scope, &mut rewritten, value);
    }
    scope.make_list(runtime, &rewritten)
}
