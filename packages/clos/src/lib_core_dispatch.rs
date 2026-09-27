const DISPATCH_KEY_PACKAGE: &str = "NCL";
const DISPATCH_KEY_NAME: &str = "%CLOS-DISPATCH-METHODS";
const DISPATCH_RECORD_METHODS: usize = 1;
const METHOD_SPECIALIZERS: usize = 0;
const METHOD_QUALIFIER: usize = 1;
const METHOD_FUNCTION: usize = 2;

fn dispatch_key(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    Package::from_word(runtime.ensure_package(ctx, DISPATCH_KEY_PACKAGE)?)
        .intern(ctx, runtime, DISPATCH_KEY_NAME)
        .map(|(symbol, _)| symbol)
}

fn generic_symbol(ctx: &ThreadContext, generic: Word) -> Result<Word, ObjectError> {
    if matches!(classify_object(ctx, generic), ObjectRef::Symbol(_)) {
        return Ok(generic);
    }
    if matches!(
        classify_object(ctx, generic),
        ObjectRef::Function(_) | ObjectRef::Closure(_)
    ) {
        let function = ncl_object::Function::from_word(generic);
        return ncl_object::function_name(ctx, function);
    }
    Err(ObjectError::TypeError)
}

fn dispatch_record(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    generic: Word,
) -> Result<Option<Word>, ObjectError> {
    let symbol = generic_symbol(ctx, generic)?;
    let key = dispatch_key(ctx, runtime)?;
    let mut plist = ncl_object::symbol_plist(ctx, symbol)?;
    while plist != Word::NIL {
        let property = ncl_object::car(ctx, plist)?;
        if ncl_object::car(ctx, property)? == key {
            return Ok(Some(ncl_object::cdr(ctx, property)?));
        }
        plist = ncl_object::cdr(ctx, plist)?;
    }
    Ok(None)
}

fn vector(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        ncl_object::make_simple_vector(
            ctx,
            runtime,
            &roots.iter().map(|value| **value).collect::<Vec<_>>(),
        )
    })
}

fn install_dispatch_record(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    generic: Word,
    record: Word,
) -> Result<(), ObjectError> {
    let symbol = generic_symbol(ctx, generic)?;
    let key = dispatch_key(ctx, runtime)?;
    let old_plist = ncl_object::symbol_plist(ctx, symbol)?;
    ncl_object::with_roots(ctx, &[symbol, key, record, old_plist], |ctx, roots| {
        let key = **roots.get(1).ok_or(ObjectError::Layout)?;
        let record = **roots.get(2).ok_or(ObjectError::Layout)?;
        let old_plist = **roots.get(3).ok_or(ObjectError::Layout)?;
        let symbol = **roots.first().ok_or(ObjectError::Layout)?;
        let mut property = ncl_object::make_cons(ctx, runtime, key, record)?;
        ncl_object::with_root(ctx, &mut property, |ctx, property| {
            let plist = ncl_object::make_cons(ctx, runtime, *property, old_plist)?;
            ncl_object::set_symbol_plist(ctx, symbol, plist)
        })
    })
}

fn eql_specializer(ctx: &ThreadContext, specializer: Word) -> Result<Option<Word>, ObjectError> {
    if !matches!(classify_object(ctx, specializer), ObjectRef::SimpleVector(_))
        || simple_vector_length(ctx, specializer)? != 2
    {
        return Ok(None);
    }
    let marker = simple_vector_ref(ctx, specializer, 0)?;
    if !matches!(classify_object(ctx, marker), ObjectRef::Symbol(_)) {
        return Ok(None);
    }
    (symbol_name_string(ctx, marker)? == "EQL")
        .then(|| simple_vector_ref(ctx, specializer, 1))
        .transpose()
}

fn dispatch_arguments(ctx: &ThreadContext, arguments: Word) -> Result<Vec<Word>, ObjectError> {
    if matches!(classify_object(ctx, arguments), ObjectRef::SimpleVector(_)) {
        return (0..simple_vector_length(ctx, arguments)?)
            .map(|index| simple_vector_ref(ctx, arguments, index))
            .collect();
    }
    form_elements(ctx, arguments)
}

fn method_matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializer: Word,
    argument: Word,
) -> Result<bool, ObjectError> {
    if let Some(value) = eql_specializer(ctx, specializer)? {
        return Ok(value == argument);
    }
    let expected = class_designator(ctx, runtime, specializer)?;
    let actual = class_of(ctx, runtime, argument)?;
    class_is_subclass(ctx, actual, expected)
}

/// Add a heap-owned method record to a generic function's dispatch metadata.
///
/// `specializers` is a vector of class designators or two-element `EQL`
/// vectors. `qualifier` is zero for a primary method. The method body is kept
/// as a function word in the same heap record, so it remains live after GC.
///
/// # Errors
/// Returns an object error when the generic function metadata or method body
/// is malformed, or when heap allocation fails.
pub fn add_method(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    generic: Word,
    specializers: Word,
    qualifier: Word,
    method_function: Word,
) -> Result<Word, ObjectError> {
    let _ = ncl_object::FunctionObject::try_from(method_function)
        .map_err(|_| ObjectError::TypeError)?;
    if !matches!(classify_object(ctx, specializers), ObjectRef::SimpleVector(_)) {
        return Err(ObjectError::TypeError);
    }
    let record = dispatch_record(ctx, runtime, generic)?;
    let methods = record
        .map(|record| simple_vector_ref(ctx, record, DISPATCH_RECORD_METHODS))
        .transpose()?
        .unwrap_or(Word::NIL);
    let method = vector(ctx, runtime, &[specializers, qualifier, method_function])?;
    let old_methods = if methods == Word::NIL {
        Vec::new()
    } else {
        (0..simple_vector_length(ctx, methods)?)
            .map(|index| simple_vector_ref(ctx, methods, index))
            .collect::<Result<Vec<_>, _>>()?
    };
    for existing in &old_methods {
        if simple_vector_ref(ctx, *existing, METHOD_SPECIALIZERS)? == specializers
            && simple_vector_ref(ctx, *existing, METHOD_QUALIFIER)? == qualifier
        {
            return Err(ObjectError::TypeError);
        }
    }
    let mut next_methods = old_methods;
    next_methods.push(method);
    let methods = vector(ctx, runtime, &next_methods)?;
    let record = vector(ctx, runtime, &[Word::fixnum(1), methods])?;
    install_dispatch_record(ctx, runtime, generic, record)?;
    Ok(method_function)
}

/// Select the first applicable primary method for a generic function.
///
/// This adapter intentionally returns the selected function. Calling it is a
/// runtime-layer operation because `RuntimeFunctionCaller` belongs to
/// `ncl-runtime`, not to this crate.
///
/// # Errors
/// Returns an object error when the generic function has no registered method
/// or its dispatch metadata is malformed.
pub fn dispatch(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    generic: Word,
    arguments: Word,
) -> Result<Word, ObjectError> {
    let record = dispatch_record(ctx, runtime, generic)?.ok_or(ObjectError::UndefinedFunction)?;
    let methods = simple_vector_ref(ctx, record, DISPATCH_RECORD_METHODS)?;
    let arguments = dispatch_arguments(ctx, arguments)?;
    for index in 0..simple_vector_length(ctx, methods)? {
        let method = simple_vector_ref(ctx, methods, index)?;
        if simple_vector_ref(ctx, method, METHOD_QUALIFIER)? != Word::fixnum(0) {
            continue;
        }
        let specializers = simple_vector_ref(ctx, method, METHOD_SPECIALIZERS)?;
        if simple_vector_length(ctx, specializers)? != arguments.len() {
            continue;
        }
        let mut applicable = true;
        for (index, argument) in arguments.iter().copied().enumerate() {
            if !method_matches(
                ctx,
                runtime,
                simple_vector_ref(ctx, specializers, index)?,
                argument,
            )? {
                applicable = false;
                break;
            }
        }
        if applicable {
            return simple_vector_ref(ctx, method, METHOD_FUNCTION);
        }
    }
    Err(ObjectError::UndefinedFunction)
}
