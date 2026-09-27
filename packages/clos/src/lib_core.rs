#[derive(Clone, Copy)]
enum BuiltinArity {
    One,
    Two,
    Three,
}

const fn descriptor(arity: BuiltinArity) -> Builtin {
    match arity {
        BuiltinArity::One => Builtin {
            lambda_list: LambdaList::fixed(ARGS_1),
            convention: ncl_object::BuiltinConvention::Direct(Arity::exact(1)),
        },
        BuiltinArity::Two => Builtin {
            lambda_list: LambdaList::fixed(ARGS_2),
            convention: ncl_object::BuiltinConvention::Direct(Arity::exact(2)),
        },
        BuiltinArity::Three => Builtin {
            lambda_list: LambdaList::fixed(ARGS_3),
            convention: ncl_object::BuiltinConvention::Direct(Arity::exact(3)),
        },
    }
}

/// Allocate a class descriptor backed by an object-layer vector.
///
/// # Errors
///
/// Returns an object allocation or storage error.
pub fn make_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    direct_superclasses: Word,
    slots: Word,
    kind: Word,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let rooted = scope.root_many::<Word>(
        &[name, direct_superclasses, slots, kind]
            .into_iter()
            .map(ncl_object::Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let mut effective = Vec::new();
    let mut rooted_iter = rooted.iter().copied();
    let name_handle = rooted_iter.next().ok_or(ObjectError::Layout)?;
    let superclass_handle = rooted_iter.next().ok_or(ObjectError::Layout)?;
    let slots_handle = rooted_iter.next().ok_or(ObjectError::Layout)?;
    let kind_handle = rooted_iter.next().ok_or(ObjectError::Layout)?;

    if scope.get(superclass_handle).as_word() != Word::NIL {
        let inherited = class_effective_slots(scope.context(), scope.get(superclass_handle).as_word())?;
        for slot in inherited {
            let inherited_slot_handle: ncl_object::Handle<'_, Word> =
                scope.root(ncl_object::Local::from_word(slot));
            let slot_name =
                slot_key(scope.context(), scope.get(inherited_slot_handle).as_word())?;
            let mut found = false;
            for candidate in &effective {
                if slot_key(scope.context(), scope.get(*candidate).as_word())? == slot_name {
                    found = true;
                    break;
                }
            }
            if !found {
                effective.push(inherited_slot_handle);
            }
        }
    }
    if scope.get(slots_handle).as_word() != Word::NIL {
        let slots_word = scope.get(slots_handle).as_word();
        for index in 0..simple_vector_length(scope.context(), slots_word)? {
            let slot = simple_vector_ref(scope.context(), slots_word, index)?;
            let direct_slot_handle: ncl_object::Handle<'_, Word> =
                scope.root(ncl_object::Local::from_word(slot));
            let key = slot_key(scope.context(), scope.get(direct_slot_handle).as_word())?;
            let mut retained = Vec::with_capacity(effective.len());
            for candidate in effective {
                if slot_key(scope.context(), scope.get(candidate).as_word())? != key {
                    retained.push(candidate);
                }
            }
            effective = retained;
            effective.push(direct_slot_handle);
        }
    }

    let effective_values = effective
        .iter()
        .map(|handle| ncl_object::Local::from_word(scope.get(*handle).as_word()))
        .collect::<Vec<_>>();
    let effective_handles = scope.root_many(&effective_values);
    let effective_slots = scope.make_simple_vector(runtime, &effective_handles)?;
    let class_values = [name_handle, superclass_handle, slots_handle, kind_handle]
        .into_iter()
    .map(|handle| ncl_object::Local::from_word(scope.get(handle).as_word()))
    .chain(std::iter::once(ncl_object::Local::from_word(
        scope.get(effective_slots).as_word(),
    )))
    .collect::<Vec<_>>();
    let class_handles = scope.root_many(&class_values);
    let class = scope.make_simple_vector(runtime, &class_handles)?;
    Ok(scope.get(class).as_word())
}

fn slot_key(ctx: &ThreadContext, slot: Word) -> Result<Word, ObjectError> {
    if matches!(classify_object(ctx, slot), ObjectRef::SimpleVector(_))
        && simple_vector_length(ctx, slot)? > 0
    {
        simple_vector_ref(ctx, slot, 0)
    } else {
        Ok(slot)
    }
}

fn class_effective_slots(ctx: &ThreadContext, class: Word) -> Result<Vec<Word>, ObjectError> {
    let length = simple_vector_length(ctx, class)?;
    let field = if length > CLASS_EFFECTIVE_SLOTS {
        CLASS_EFFECTIVE_SLOTS
    } else {
        CLASS_SLOTS
    };
    let slots = simple_vector_ref(ctx, class, field)?;
    if slots == Word::NIL {
        return Ok(Vec::new());
    }
    (0..simple_vector_length(ctx, slots)?)
        .map(|index| simple_vector_ref(ctx, slots, index))
        .collect()
}

/// Return the name stored in a class descriptor.
///
/// # Errors
///
/// Returns an object layout or storage error when `class` is not a vector.
pub fn class_name(ctx: &ThreadContext, class: Word) -> Result<Word, ObjectError> {
    simple_vector_ref(ctx, class, CLASS_NAME)
}

/// Return the class of an object using the existing object-layer layouts.
///
/// # Errors
///
/// Returns an object layout error when the corresponding built-in class is absent.
pub fn class_of(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    if let Ok(class) = instance_class(ctx, Instance::from_word(object)) {
        return Ok(class);
    }
    let name = if object == Word::NIL {
        "NULL"
    } else {
        let object_ref = classify_object(ctx, object);
        if matches!(object_ref, ObjectRef::Fixnum(_)) {
            "INTEGER"
        } else if matches!(object_ref, ObjectRef::Character(_)) {
            "CHARACTER"
        } else if matches!(object_ref, ObjectRef::Cons(_)) {
            "CONS"
        } else if matches!(object_ref, ObjectRef::Symbol(_)) {
            "SYMBOL"
        } else if matches!(object_ref, ObjectRef::String(_)) {
            "STRING"
        } else if matches!(object_ref, ObjectRef::SimpleVector(_)) {
            "SIMPLE-VECTOR"
        } else if matches!(
            object_ref,
            ObjectRef::Array(_) | ObjectRef::SpecializedArray(_)
        ) {
            "ARRAY"
        } else if matches!(object_ref, ObjectRef::HashTable(_)) {
            "HASH-TABLE"
        } else if matches!(object_ref, ObjectRef::Function(_) | ObjectRef::Closure(_)) {
            "FUNCTION"
        } else if matches!(object_ref, ObjectRef::Package(_)) {
            "PACKAGE"
        } else if matches!(object_ref, ObjectRef::Stream(_)) {
            "STREAM"
        } else if matches!(object_ref, ObjectRef::Structure(_)) {
            "STRUCTURE-OBJECT"
        } else if matches!(object_ref, ObjectRef::Bignum(_)) {
            "BIGNUM"
        } else if matches!(object_ref, ObjectRef::Ratio(_)) {
            "RATIO"
        } else if matches!(object_ref, ObjectRef::DoubleFloat(_)) {
            "DOUBLE-FLOAT"
        } else if matches!(object_ref, ObjectRef::Complex(_)) {
            "COMPLEX"
        } else {
            "T"
        }
    };
    runtime
        .class(ctx, name)
        .or_else(|| runtime.class(ctx, "T"))
        .ok_or(ObjectError::Layout)
}

/// Allocate an instance with an already finalized slot vector.
///
/// # Errors
///
/// Returns an object allocation or storage error.
pub fn make_instance(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    class: Word,
    slots: &[Word],
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let class: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(class));
    let rooted_slots = scope.root_many(
        &slots
            .iter()
            .copied()
            .map(ncl_object::Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let slot_words = rooted_slots
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let class_word = scope.get(class).as_word();
    let instance = allocate_instance(scope.context_mut(), runtime, class_word, &slot_words)?;
    let instance: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(instance.as_word()));
    Ok(scope.get(instance).as_word())
}

const fn typed_error(ctx: &mut ThreadContext, error: ncl_object::LispError) -> ObjectError {
    ctx.set_pending_lisp_error(error);
    ObjectError::TypeError
}

fn instance_arg(ctx: &mut ThreadContext, word: Word) -> Result<Instance, ObjectError> {
    match classify_object(ctx, word) {
        ObjectRef::Instance(_) => Ok(Instance::from_word(word)),
        _ => Err(typed_error(
            ctx,
            ncl_object::LispError::TypeError {
                datum: word,
                expected: ObjectType::Instance,
            },
        )),
    }
}

fn slot_index(ctx: &ThreadContext, instance: Instance, designator: Word) -> Result<usize, ObjectError> {
    if let Ok(index) = Fixnum::try_from_word(designator) {
        return usize::try_from(index.value()).map_err(|_| ObjectError::TypeError);
    }
    let class = instance_class(ctx, instance)?;
    let slots = simple_vector_ref(ctx, class, CLASS_EFFECTIVE_SLOTS)?;
    for index in 0..simple_vector_length(ctx, slots)? {
        let descriptor = simple_vector_ref(ctx, slots, index)?;
        let name = if matches!(classify_object(ctx, descriptor), ObjectRef::SimpleVector(_)) {
            simple_vector_ref(ctx, descriptor, 0)?
        } else { descriptor };
        if name == designator { return Ok(index); }
    }
    Err(ObjectError::TypeError)
}

fn slot_value_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = slot_index(ctx, instance, args.required(1)?)?;
    slot_ref(
        ctx,
        instance,
        index,
    )
}

fn slot_set_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = slot_index(ctx, instance, args.required(1)?)?;
    let value = args.required(2)?;
    slot_set(
        ctx,
        instance,
        index,
        value,
    )?;
    Ok(value)
}

fn slot_boundp_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = slot_value_builtin(ctx, runtime, args, values)?;
    Ok(if value == Word::UNBOUND {
        Word::NIL
    } else {
        Word::TRUE
    })
}

fn slot_makunbound_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = slot_index(ctx, instance, args.required(1)?)?;
    slot_set(
        ctx,
        instance,
        index,
        Word::UNBOUND,
    )?;
    Ok(instance.as_word())
}

fn slot_exists_in_class(
    ctx: &ThreadContext,
    class: Word,
    slot_name: Word,
) -> Result<bool, ObjectError> {
    let slots = simple_vector_ref(ctx, class, CLASS_SLOTS)?;
    if slots != Word::NIL {
        for index in 0..simple_vector_length(ctx, slots)? {
            let slot = simple_vector_ref(ctx, slots, index)?;
            let name = match simple_vector_length(ctx, slot) {
                Ok(length) if length >= 2 => simple_vector_ref(ctx, slot, 0)?,
                _ => slot,
            };
            if name == slot_name {
                return Ok(true);
            }
        }
    }
    let superclass = simple_vector_ref(ctx, class, CLASS_DIRECT_SUPERCLASS)?;
    if superclass == Word::NIL {
        return Ok(false);
    }
    slot_exists_in_class(ctx, superclass, slot_name)
}

fn slot_exists_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let class = instance_class(ctx, instance)?;
    let slot_name = args.required(1)?;
    Ok(if slot_exists_in_class(ctx, class, slot_name)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn class_of_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_of(ctx, runtime, args.required(0)?)
}

fn class_is_subclass(
    ctx: &ThreadContext,
    actual: Word,
    expected: Word,
) -> Result<bool, ObjectError> {
    if actual == expected {
        return Ok(true);
    }
    let superclass = simple_vector_ref(ctx, actual, CLASS_DIRECT_SUPERCLASS)?;
    if superclass == Word::NIL {
        return Ok(false);
    }
    class_is_subclass(ctx, superclass, expected)
}

pub(crate) fn typep_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let object = args.required(0)?;
    let expected = class_designator(ctx, runtime, args.required(1)?)?;
    let actual = class_of(ctx, runtime, object)?;
    Ok(if class_is_subclass(ctx, actual, expected)? {
        Word::TRUE
    } else {
        Word::NIL
    })
}

fn class_name_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_name(ctx, args.required(0)?)
}

// Dispatch metadata is kept in a symbol plist rather than in a Rust-side
// registry.  The latter would retain moving heap words without a GC root and
// would also duplicate the runtime's function registry.
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
    match classify_object(ctx, generic) {
        ObjectRef::Symbol(_) => Ok(generic),
        ObjectRef::Function(_) | ObjectRef::Closure(_) => {
            let function = ncl_object::Function::from_word(generic);
            ncl_object::function_name(ctx, function)
        }
        _ => Err(ObjectError::TypeError),
    }
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
        let mut property = ncl_object::make_cons(ctx, runtime, *roots[1], *roots[2])?;
        ncl_object::with_root(ctx, &mut property, |ctx, property| {
            let plist = ncl_object::make_cons(ctx, runtime, *property, *roots[3])?;
            ncl_object::set_symbol_plist(ctx, *roots[0], plist)
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
