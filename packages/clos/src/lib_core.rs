#[derive(Clone, Copy)]
enum BuiltinArity {
    One,
    Two,
    Three,
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
            let mut position = None;
            for (candidate_position, candidate) in effective.iter().enumerate() {
                if slot_key(scope.context(), scope.get(*candidate).as_word())? == key {
                    position = Some(candidate_position);
                    break;
                }
            }
            if let Some(position) = position {
                let slot = effective.get_mut(position).ok_or(ObjectError::Layout)?;
                *slot = direct_slot_handle;
            } else {
                effective.push(direct_slot_handle);
            }
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
            if let Ok(layout) = ncl_object::structure_layout(ctx, object)
                && runtime.structure_class(ctx, layout).is_some()
            {
                return runtime.structure_class(ctx, layout).ok_or(ObjectError::Layout);
            }
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

fn slot_index(
    ctx: &ThreadContext,
    instance: Instance,
    designator: Word,
) -> Result<usize, ObjectError> {
    if let Ok(index) = Fixnum::try_from_word(designator) {
        return usize::try_from(index.value()).map_err(|_| ObjectError::TypeError);
    }
    let class = instance_class(ctx, instance)?;
    let slots = simple_vector_ref(ctx, class, CLASS_EFFECTIVE_SLOTS)?;
    for index in 0..simple_vector_length(ctx, slots)? {
        let descriptor = simple_vector_ref(ctx, slots, index)?;
        let name = if matches!(classify_object(ctx, descriptor), ObjectRef::SimpleVector(_)) {
            simple_vector_ref(ctx, descriptor, 0)?
        } else {
            descriptor
        };
        if name == designator {
            return Ok(index);
        }
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

/// Decode a raw direct-superclass slot value into its direct parents: empty
/// at the root, one descriptor for ordinary single inheritance, or several
/// for a class with genuine multiple inheritance (for example a condition
/// class registered by `ncl-conditions` such as `simple-error`).
fn direct_superclasses(ctx: &ThreadContext, value: Word) -> Result<Vec<Word>, ObjectError> {
    if value == Word::NIL {
        return Ok(Vec::new());
    }
    if !value.is_cons() {
        return Ok(vec![value]);
    }
    let mut parents = Vec::new();
    let mut current = value;
    while current != Word::NIL {
        parents.push(car(ctx, current)?);
        current = cdr(ctx, current)?;
    }
    Ok(parents)
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
    for parent in direct_superclasses(ctx, superclass)? {
        if slot_exists_in_class(ctx, parent, slot_name)? {
            return Ok(true);
        }
    }
    Ok(false)
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
    for parent in direct_superclasses(ctx, superclass)? {
        if class_is_subclass(ctx, parent, expected)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn typep_builtin(
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

#[cfg(test)]
#[path = "../tests/support/lib_core_tests.rs"]
mod core_tests;

// Dispatch metadata is kept in a symbol plist rather than in a Rust-side
// registry. The latter would retain moving heap words without a GC root and
// would also duplicate the runtime's function registry.
