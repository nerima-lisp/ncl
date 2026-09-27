fn clos_define_generic_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    set_method_registry(ctx, runtime, name, Word::NIL)?;
    Ok(name)
}

fn clos_ensure_initialization_base_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let name_text = symbol_name_string(ctx, name)?;
    if name_text != "INITIALIZE-INSTANCE" && name_text != "SHARED-INITIALIZE" {
        return Ok(name);
    }
    if has_method_registry(ctx, runtime, name)? {
        return Ok(name);
    }
    let function = symbol_function(ctx, name)?;
    // check-added-lines: allow(unbound) function cell absence is reported as an error
    if function == Word::UNBOUND {
        return Err(ObjectError::UndefinedFunction);
    }
    let variable = ncl_symbol(ctx, runtime, "INSTANCE")?;
    let class = common_lisp_symbol(ctx, runtime, "T")?;
    let specializer = lisp_list(ctx, runtime, &[variable, class])?;
    let specializers = lisp_list(ctx, runtime, &[specializer])?;
    let entry = method_registry_entry(
        ctx,
        runtime,
        specializers,
        Word::fixnum(METHOD_QUALIFIER_PRIMARY),
        function,
    )?;
    let registry = lisp_list(ctx, runtime, &[entry])?;
    set_method_registry(ctx, runtime, name, registry)?;
    Ok(name)
}

fn clos_add_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let encoded = args.required(1)?;
    let (specializers, qualifier) = method_definition_parts(ctx, runtime, encoded)?;
    let function = FunctionObject::try_from(args.required(2)?)?.as_word();
    let old = method_registry(ctx, runtime, name)?;
    let mut entries = Vec::new();
    let mut cursor = old;
    while cursor != Word::NIL {
        entries.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    entries.retain(|entry| {
        let Ok(entry_specializers) = car(ctx, *entry) else {
            return true;
        };
        let Ok(qualifier_pair) = cdr(ctx, *entry) else {
            return true;
        };
        let Ok(entry_qualifier) = car(ctx, qualifier_pair) else {
            return true;
        };
        entry_specializers != specializers || entry_qualifier != qualifier
    });
    let entry = method_registry_entry(ctx, runtime, specializers, qualifier, function)?;
    entries.insert(0, entry);
    let registry = lisp_list(ctx, runtime, &entries)?;
    set_method_registry(ctx, runtime, name, registry)?;
    Ok(name)
}

fn continuation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    kind: i64,
    payload: Word,
) -> Result<Word, ObjectError> {
    let tag = ncl_symbol(ctx, runtime, "*CLOS-CONTINUATION*")?;
    lisp_list(ctx, runtime, &[tag, Word::fixnum(kind), payload])
}

fn call_method(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    arguments: &[Word],
    argument_list: Word,
    next: Word,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let next_symbol = ncl_symbol(ctx, runtime, "*CLOS-NEXT-METHOD*")?;
    let args_symbol = ncl_symbol(ctx, runtime, "*CLOS-CURRENT-ARGS*")?;
    let previous_next = symbol_value(ctx, next_symbol)?;
    let previous_args = symbol_value(ctx, args_symbol)?;
    set_symbol_value(ctx, next_symbol, next)?;
    set_symbol_value(ctx, args_symbol, argument_list)?;
    let result = BuiltinFunctionCaller.call_function(
        ctx,
        runtime,
        FunctionDesignator::Function(FunctionObject::try_from(function)?),
        FunctionArguments::new(arguments),
        values,
    );
    set_symbol_value(ctx, next_symbol, previous_next)?;
    set_symbol_value(ctx, args_symbol, previous_args)?;
    result
}

fn invoke_core(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    payload: Word,
    arguments: &[Word],
    argument_list: Word,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let fields = form_elements(ctx, payload)?;
    let before = form_elements(ctx, fields.first().copied().ok_or(ObjectError::TypeError)?)?;
    let primary = form_elements(ctx, fields.get(1).copied().ok_or(ObjectError::TypeError)?)?;
    let after = form_elements(ctx, fields.get(2).copied().ok_or(ObjectError::TypeError)?)?;
    for function in before {
        call_method(
            ctx,
            runtime,
            function,
            arguments,
            argument_list,
            Word::NIL,
            values,
        )?;
    }
    let next = if primary.len() > 1 {
        let rest = lisp_list(
            ctx,
            runtime,
            primary.get(1..).ok_or(ObjectError::Layout)?,
        )?;
        continuation(ctx, runtime, CONTINUATION_PRIMARY, rest)?
    } else {
        Word::NIL
    };
    let result = call_method(
        ctx,
        runtime,
        *primary.first().ok_or(ObjectError::UndefinedFunction)?,
        arguments,
        argument_list,
        next,
        values,
    )?;
    let returned_values = values.as_slice().to_vec();
    for function in after {
        call_method(
            ctx,
            runtime,
            function,
            arguments,
            argument_list,
            Word::NIL,
            values,
        )?;
    }
    values.set(&returned_values);
    Ok(result)
}

fn invoke_continuation(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    value: Word,
    arguments: &[Word],
    argument_list: Word,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let fields = form_elements(ctx, value)?;
    let kind = fields
        .get(1)
        .copied()
        .ok_or(ObjectError::TypeError)?
        .as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let payload = fields.get(2).copied().ok_or(ObjectError::TypeError)?;
    if kind == CONTINUATION_CORE {
        return invoke_core(ctx, runtime, payload, arguments, argument_list, values);
    }
    if kind == CONTINUATION_AROUND || kind == CONTINUATION_PRIMARY {
            let (methods, tail) = if kind == CONTINUATION_AROUND {
                let fields = form_elements(ctx, payload)?;
                (
                    form_elements(ctx, fields.first().copied().ok_or(ObjectError::TypeError)?)?,
                    fields.get(1).copied().ok_or(ObjectError::TypeError)?,
                )
            } else {
                (form_elements(ctx, payload)?, Word::NIL)
            };
            let function = *methods.first().ok_or(ObjectError::UndefinedFunction)?;
            let rest = lisp_list(
                ctx,
                runtime,
                methods.get(1..).ok_or(ObjectError::Layout)?,
            )?;
            let next = if rest == Word::NIL {
                if kind == CONTINUATION_AROUND {
                    tail
                } else {
                    Word::NIL
                }
            } else if kind == CONTINUATION_AROUND {
                let next_payload = lisp_list(ctx, runtime, &[rest, tail])?;
                continuation(ctx, runtime, kind, next_payload)?
            } else {
                continuation(ctx, runtime, kind, rest)?
            };
            return call_method(
                ctx,
                runtime,
                function,
                arguments,
                argument_list,
                next,
                values,
            );
    }
    Err(ObjectError::TypeError)
}

fn clos_dispatch_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let name = args.required(0)?;
    let argument_list = args.required(1)?;
    let arguments = form_elements(ctx, argument_list)?;
    if !has_method_registry(ctx, runtime, name)? {
        ctx.set_pending_lisp_error(LispError::CellError(CellError::UndefinedFunction { name }));
        return Err(ObjectError::UndefinedFunction);
    }
    let mut matches = Vec::new();
    let mut cursor = method_registry(ctx, runtime, name)?;
    while cursor != Word::NIL {
        let entry = car(ctx, cursor)?;
        let fields = form_elements(ctx, entry)?;
        let specializers = *fields.first().ok_or(ObjectError::TypeError)?;
        let qualifier = *fields.get(1).ok_or(ObjectError::TypeError)?;
        let function = *fields.get(2).ok_or(ObjectError::TypeError)?;
        if let Some(score) = method_match(ctx, runtime, specializers, &arguments)? {
            matches.push((score, qualifier, function));
        }
        cursor = cdr(ctx, cursor)?;
    }
    matches.sort_by_key(|left| std::cmp::Reverse(left.0));
    let mut around = Vec::new();
    let mut before = Vec::new();
    let mut primary = Vec::new();
    let mut after = Vec::new();
    for (_, qualifier, function) in matches {
        if qualifier.as_fixnum() == Some(METHOD_QUALIFIER_AROUND) {
            around.push(function);
        } else if qualifier.as_fixnum() == Some(METHOD_QUALIFIER_BEFORE) {
            before.push(function);
        } else if qualifier.as_fixnum() == Some(METHOD_QUALIFIER_AFTER) {
            after.push(function);
        } else {
            primary.push(function);
        }
    }
    after.reverse();
    if primary.is_empty() {
        ctx.set_pending_lisp_error(LispError::Object(ObjectError::Unbound));
        return Err(ObjectError::Unbound);
    }
    let before_list = lisp_list(ctx, runtime, &before)?;
    let primary_list = lisp_list(ctx, runtime, &primary)?;
    let after_list = lisp_list(ctx, runtime, &after)?;
    let payload = lisp_list(ctx, runtime, &[before_list, primary_list, after_list])?;
    let core = continuation(ctx, runtime, CONTINUATION_CORE, payload)?;
    let around = lisp_list(ctx, runtime, &around)?;
    if around == Word::NIL {
        invoke_continuation(ctx, runtime, core, &arguments, argument_list, values)
    } else {
        let around_payload = lisp_list(ctx, runtime, &[around, core])?;
        let wrapper = continuation(ctx, runtime, CONTINUATION_AROUND, around_payload)?;
        invoke_continuation(ctx, runtime, wrapper, &arguments, argument_list, values)
    }
}

fn clos_call_next_method_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let next_symbol = ncl_symbol(ctx, runtime, "*CLOS-NEXT-METHOD*")?;
    let args_symbol = ncl_symbol(ctx, runtime, "*CLOS-CURRENT-ARGS*")?;
    let next = symbol_value(ctx, next_symbol)?;
    // An unbound dynamic variable means CALL-NEXT-METHOD has no continuation.
    // check-added-lines: allow(unbound) dynamic sentinel for missing continuation
    if next == Word::NIL || next == Word::UNBOUND {
        let call_next_name = common_lisp_symbol(ctx, runtime, "CALL-NEXT-METHOD")?;
        ctx.set_pending_lisp_error(LispError::CellError(CellError::UndefinedFunction {
            name: call_next_name,
        }));
        return Err(ObjectError::UndefinedFunction);
    }
    let argument_list = symbol_value(ctx, args_symbol)?;
    let arguments = form_elements(ctx, argument_list)?;
    invoke_continuation(ctx, runtime, next, &arguments, argument_list, values)
}

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    // check-added-lines: allow(index)
    let name = symbol_name(ctx, symbol)?;
    let length = string_length(ctx, name)?;
    (0..length)
        .map(|index| string_ref(ctx, name, index))
        .collect()
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
            // check-added-lines: allow(unbound) class placeholder
            .filter(|class| *class != Word::UNBOUND)
            .ok_or(ObjectError::TypeError);
    }
    Err(ObjectError::TypeError)
}
