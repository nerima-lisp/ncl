const fn descriptor(arity: u8) -> Builtin {
    let required = match arity {
        0 => ARGS_0,
        1 => ARGS_1,
        2 => ARGS_2,
        3 => ARGS_3,
        4 => ARGS_4,
        _ => &[],
    };
    Builtin {
        lambda_list: LambdaList::fixed(required),
        convention: ncl_object::BuiltinConvention::Direct(Arity::exact(arity)),
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
    let mut effective = Vec::new();
    if direct_superclasses != Word::NIL {
        let inherited = class_effective_slots(ctx, direct_superclasses)?;
        for slot in inherited {
            let slot_name = slot_key(ctx, slot)?;
            let mut found = false;
            for candidate in &effective {
                if slot_key(ctx, *candidate)? == slot_name {
                    found = true;
                    break;
                }
            }
            if !found {
                effective.push(slot);
            }
        }
    }
    if slots != Word::NIL {
        for index in 0..simple_vector_length(ctx, slots)? {
            let slot = simple_vector_ref(ctx, slots, index)?;
            let key = slot_key(ctx, slot)?;
            let mut retained = Vec::with_capacity(effective.len());
            for candidate in effective {
                if slot_key(ctx, candidate)? != key {
                    retained.push(candidate);
                }
            }
            effective = retained;
            effective.push(slot);
        }
    }
    let effective_slots = make_simple_vector(ctx, runtime, &effective)?;
    make_simple_vector(
        ctx,
        runtime,
        &[name, direct_superclasses, slots, kind, effective_slots],
    )
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
        match classify_object(ctx, object) {
            ObjectRef::Fixnum(_) => "INTEGER",
            ObjectRef::Character(_) => "CHARACTER",
            ObjectRef::Cons(_) => "CONS",
            ObjectRef::Symbol(_) => "SYMBOL",
            ObjectRef::String(_) => "STRING",
            ObjectRef::SimpleVector(_) => "SIMPLE-VECTOR",
            ObjectRef::Array(_) | ObjectRef::SpecializedArray(_) => "ARRAY",
            ObjectRef::HashTable(_) => "HASH-TABLE",
            ObjectRef::Function(_) | ObjectRef::Closure(_) => "FUNCTION",
            ObjectRef::Package(_) => "PACKAGE",
            ObjectRef::Stream(_) => "STREAM",
            ObjectRef::Structure(_) => "STRUCTURE-OBJECT",
            ObjectRef::Bignum(_) => "BIGNUM",
            ObjectRef::Ratio(_) => "RATIO",
            ObjectRef::DoubleFloat(_) => "DOUBLE-FLOAT",
            ObjectRef::Complex(_) => "COMPLEX",
            _ => "T",
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
    Ok(allocate_instance(ctx, runtime, class, slots)?.as_word())
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

fn fixnum_arg(ctx: &mut ThreadContext, word: Word) -> Result<Fixnum, ObjectError> {
    Fixnum::try_from_word(word).map_err(|error| typed_error(ctx, error.into()))
}

fn slot_value_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = fixnum_arg(ctx, args.required(1)?)?;
    slot_ref(
        ctx,
        instance,
        usize::try_from(index.value()).map_err(|_| ObjectError::TypeError)?,
    )
}

fn slot_set_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let instance = instance_arg(ctx, args.required(0)?)?;
    let index = fixnum_arg(ctx, args.required(1)?)?;
    let value = args.required(2)?;
    slot_set(
        ctx,
        instance,
        usize::try_from(index.value()).map_err(|_| ObjectError::TypeError)?,
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
    let index = fixnum_arg(ctx, args.required(1)?)?;
    slot_set(
        ctx,
        instance,
        usize::try_from(index.value()).map_err(|_| ObjectError::TypeError)?,
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

fn class_name_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_name(ctx, args.required(0)?)
}

fn install_class(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    superclass: Option<&str>,
) -> Result<(), ObjectError> {
    let name_word = ncl_object::make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    let supers = superclass
        .and_then(|parent| runtime.class(ctx, parent))
        .unwrap_or(Word::NIL);
    let class = make_class(ctx, runtime, name_word, supers, Word::NIL, Word::fixnum(0))?;
    runtime.define_class(ctx, name, class)
}

