fn find_class_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_designator(ctx, runtime, args.required(0)?)
}

#[allow(clippy::too_many_lines)]
fn defclass_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let form = args.required(0)?;
    let parts = form_elements(ctx, form)?;
    let name = *parts.get(1).ok_or(ObjectError::TypeError)?;
    let supers = parts.get(2).copied().unwrap_or(Word::NIL);
    let superclass = form_elements(ctx, supers)?
        .first()
        .copied()
        .unwrap_or(Word::NIL);
    let superclass = if superclass == Word::NIL {
        Word::NIL
    } else {
        class_designator(ctx, runtime, superclass)?
    };
    let class_name = symbol_name_string(ctx, name)?;
    let slot_forms = parts.get(3).copied().unwrap_or(Word::NIL);
    let mut slot_specs = Vec::new();
    let mut accessors = Vec::new();
    for slot_form in form_elements(ctx, slot_forms)? {
        let fields = form_elements(ctx, slot_form)?;
        let slot_name = *fields.first().ok_or(ObjectError::TypeError)?;
        let mut initarg = Word::NIL;
        // check-added-lines: allow(unbound) sentinel initialization
        let mut initform = Word::UNBOUND;
        let mut index = 1;
        while index + 1 < fields.len() {
            // check-added-lines: allow(index) slot specification
            let key = fields[index];
            let key_name = symbol_name_string(ctx, key)?;
            if key_name == ":INITARG" || key_name == "INITARG" {
                // check-added-lines: allow(index) slot specification
                initarg = fields[index + 1];
            }
            if key_name == ":INITFORM" || key_name == "INITFORM" {
                // check-added-lines: allow(index) slot specification
                initform = fields[index + 1];
            }
            if (key_name == ":ACCESSOR"
                || key_name == "ACCESSOR"
                || key_name == ":READER"
                || key_name == "READER") // check-added-lines: allow(index) slot specification
                && fields[index + 1] != Word::NIL
            {
                // check-added-lines: allow(index) slot specification
                accessors.push((fields[index + 1], slot_name));
            }
            index += 2;
        }
        slot_specs.push((slot_name, initarg, initform));
    }
    let mut scope = Scope::new(ctx);
    let mut input_words = vec![name, superclass];
    for (slot_name, initarg, initform) in &slot_specs {
        input_words.extend_from_slice(&[*slot_name, *initarg, *initform]);
    }
    for (accessor, slot_name) in &accessors {
        input_words.extend_from_slice(&[*accessor, *slot_name]);
    }
    let input_roots = scope.root_many(
        &input_words
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let mut descriptors = Vec::new();
    let mut input_index = 2;
    for _ in &slot_specs {
        let slot_name = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        let initarg = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index + 1)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        let initform = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index + 2)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        input_index += 3;
        let values: [Word; 3] = (slot_name, initarg, initform).into();
        let roots = scope.root_many(&values.map(Local::from_word));
        descriptors.push(scope.make_simple_vector(runtime, &roots)?);
    }
    let slot_roots = scope.root_many(
        &descriptors
            .iter()
            .map(|handle| Local::from_word(scope.get(*handle).as_word()))
            .collect::<Vec<_>>(),
    );
    let slots = scope.make_simple_vector(runtime, &slot_roots)?;
    let name_root = *input_roots.as_slice().first().ok_or(ObjectError::Layout)?;
    let super_root = *input_roots.as_slice().get(1).ok_or(ObjectError::Layout)?;
    let slots_word = scope.get(slots).as_word();
    let name_word = scope.get(name_root).as_word();
    let super_word = scope.get(super_root).as_word();
    let class = make_class(
        scope.context_mut(),
        runtime,
        name_word,
        super_word,
        slots_word,
        Word::NIL,
    )?;
    let class_root = scope.root(Local::<Word>::from_word(class));
    let class_word = scope.get(class_root).as_word();
    runtime.define_class(scope.context_mut(), class_name, class_word)?;
    let mut definitions = Vec::new();
    for _ in &accessors {
        let accessor = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        let slot = scope
            .get(
                *input_roots
                    .as_slice()
                    .get(input_index + 1)
                    .ok_or(ObjectError::Layout)?,
            )
            .as_word();
        input_index += 2;
        definitions.push(make_accessor_definition(
            scope.context_mut(),
            runtime,
            accessor,
            slot,
        )?);
    }
    let expansion = if definitions.is_empty() {
        Word::NIL
    } else {
        make_progn(scope.context_mut(), runtime, &definitions)?
    };
    drop(scope);
    Ok(expansion)
}

#[allow(clippy::missing_const_for_fn, clippy::unnecessary_wraps)]
fn defgeneric_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, args.required(0)?)?;
    let name = *parts.get(1).ok_or(ObjectError::TypeError)?;
    let _lambda = *parts.get(2).ok_or(ObjectError::TypeError)?;
    let defun = common_lisp_symbol(ctx, runtime, "DEFUN")?;
    let progn = common_lisp_symbol(ctx, runtime, "PROGN")?;
    let define = common_lisp_symbol(ctx, runtime, "%CLOS-DEFINE-GENERIC")?;
    let dispatch = common_lisp_symbol(ctx, runtime, "%CLOS-DISPATCH")?;
    let args = common_lisp_symbol(ctx, runtime, "ARGS")?;
    let rest = common_lisp_symbol(ctx, runtime, "&REST")?;
    let quoted_name = quoted(ctx, runtime, name)?;
    let clear = lisp_list(ctx, runtime, &[define, quoted_name])?;
    let dispatch_call = lisp_list(ctx, runtime, &[dispatch, quoted_name, args])?;
    let lambda_list = lisp_list(ctx, runtime, &[rest, args])?;
    let function = lisp_list(ctx, runtime, &[defun, name, lambda_list, dispatch_call])?;
    lisp_list(ctx, runtime, &[progn, clear, function, name])
}

static METHOD_FUNCTION_COUNTER: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn defmethod_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let parts = form_elements(ctx, args.required(0)?)?;
    let name = *parts.get(1).ok_or(ObjectError::TypeError)?;
    let (qualifier, specializer_index) = match parts.get(2).copied() {
        Some(value) => method_qualifier(ctx, value)?.map_or_else(
            || (Word::fixnum(METHOD_QUALIFIER_PRIMARY), 2),
            |qualifier| (qualifier, 3),
        ),
        None => return Err(ObjectError::TypeError),
    };
    let specializer_form = *parts.get(specializer_index).ok_or(ObjectError::TypeError)?;
    let specializer_fields = form_elements(ctx, specializer_form)?;
    let mut lambda = Vec::with_capacity(specializer_fields.len());
    let mut dispatch_specializers = Vec::new();
    let mut index = 0;
    while index < specializer_fields.len() {
        let field = specializer_fields[index];
        if field.is_cons() {
            let fields = form_elements(ctx, field)?;
            lambda.push(*fields.first().ok_or(ObjectError::TypeError)?);
            dispatch_specializers.push(field);
            index += 1;
        } else if symbol_name_string(ctx, field)? == "&REST" {
            lambda.push(field);
            lambda.push(
                *specializer_fields
                    .get(index + 1)
                    .ok_or(ObjectError::TypeError)?,
            );
            index += 2;
        } else {
            return Err(ObjectError::TypeError);
        }
    }
    let dispatch_specializers = lisp_list(ctx, runtime, &dispatch_specializers)?;
    let defun = common_lisp_symbol(ctx, runtime, "DEFUN")?;
    let dispatch = common_lisp_symbol(ctx, runtime, "%CLOS-DISPATCH")?;
    let args_symbol = common_lisp_symbol(ctx, runtime, "ARGS")?;
    let rest = common_lisp_symbol(ctx, runtime, "&REST")?;
    let lambda_words = lambda;
    let lambda = lisp_list(ctx, runtime, &lambda_words)?;
    let body = parts
        .get(specializer_index + 1..)
        .ok_or(ObjectError::TypeError)?;
    let body = make_progn(ctx, runtime, body)?;
    let method_name = {
        let id = METHOD_FUNCTION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let symbol_name = format!("%CLOS-METHOD-{id}");
        ncl_symbol(ctx, runtime, &symbol_name)?
    };
    let function_operator = common_lisp_symbol(ctx, runtime, "FUNCTION")?;
    let method_function = lisp_list(ctx, runtime, &[function_operator, method_name])?;
    let tag = ncl_symbol(ctx, runtime, "*CLOS-METHOD-DEFINITION*")?;
    let definition = lisp_list(ctx, runtime, &[tag, qualifier, dispatch_specializers])?;
    let quoted_specializers = quoted(ctx, runtime, definition)?;
    let add_method = common_lisp_symbol(ctx, runtime, "%CLOS-ADD-METHOD")?;
    let quoted_name = quoted(ctx, runtime, name)?;
    let registration = lisp_list(
        ctx,
        runtime,
        &[
            add_method,
            quoted_name,
            quoted_specializers,
            method_function,
        ],
    )?;
    let method_definition = lisp_list(ctx, runtime, &[defun, method_name, lambda, body])?;
    let dispatch_call = lisp_list(ctx, runtime, &[dispatch, quoted_name, args_symbol])?;
    let wrapper_lambda = lisp_list(ctx, runtime, &[rest, args_symbol])?;
    let wrapper = lisp_list(ctx, runtime, &[defun, name, wrapper_lambda, dispatch_call])?;
    let name_text = symbol_name_string(ctx, name)?;
    let initialization_base = if name_text == "INITIALIZE-INSTANCE"
        || name_text == "SHARED-INITIALIZE"
    {
        let ensure = common_lisp_symbol(ctx, runtime, "%CLOS-ENSURE-INITIALIZATION-BASE")?;
        Some(lisp_list(ctx, runtime, &[ensure, quoted_name])?)
    } else {
        None
    };
    let progn = common_lisp_symbol(ctx, runtime, "PROGN")?;
    let mut forms = Vec::with_capacity(6);
    forms.push(progn);
    if let Some(base) = initialization_base {
        forms.push(base);
    }
    forms.push(wrapper);
    forms.push(method_definition);
    forms.push(registration);
    forms.push(name);
    lisp_list(
        ctx,
        runtime,
        &forms,
    )
}
