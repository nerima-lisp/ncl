
fn form_elements(ctx: &ThreadContext, mut form: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while form != Word::NIL {
        if !form.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(car(ctx, form)?);
        form = cdr(ctx, form)?;
    }
    Ok(result)
}

fn lisp_list(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result<Word, ObjectError> {
    ncl_object::with_roots(ctx, values, |ctx, roots| {
        let mut result = Word::NIL;
        for value in roots.iter().rev() {
            result = ncl_object::with_root(ctx, &mut result, |ctx, result| {
                make_cons(ctx, runtime, **value, *result)
            })?;
        }
        Ok(result)
    })
}

fn quoted(ctx: &mut ThreadContext, runtime: &Runtime, value: Word) -> Result<Word, ObjectError> {
    let quote = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "QUOTE")?
        .0;
    lisp_list(ctx, runtime, &[quote, value]) // check-added-lines: allow(index)
}

fn make_accessor_definition(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    slot: Word,
) -> Result<Word, ObjectError> {
    let defun = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "DEFUN")?
        .0;
    let instance = Package::from_word(runtime.ensure_package(ctx, "NCL")?)
        .intern(ctx, runtime, "INSTANCE")?
        .0;
    let lambda = lisp_list(ctx, runtime, &[instance])?;
    let slot_value = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "SLOT-VALUE")?
        .0;
    let quoted_slot = quoted(ctx, runtime, slot)?;
    let body = lisp_list(ctx, runtime, &[slot_value, instance, quoted_slot])?;
    lisp_list(ctx, runtime, &[defun, name, lambda, body])
}

fn make_progn(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    forms: &[Word], // check-added-lines: allow(index)
) -> Result<Word, ObjectError> {
    let progn = Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, "PROGN")?
        .0;
    let mut values = Vec::with_capacity(forms.len() + 1);
    values.push(progn);
    values.extend_from_slice(forms);
    lisp_list(ctx, runtime, &values)
}

fn ncl_symbol(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Result<Word, ObjectError> {
    Package::from_word(runtime.ensure_package(ctx, "NCL")?)
        .intern(ctx, runtime, name)
        .map(|(symbol, _)| symbol)
}

fn common_lisp_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ObjectError> {
    Package::from_word(runtime.ensure_package(ctx, COMMON_LISP)?)
        .intern(ctx, runtime, name)
        .map(|(symbol, _)| symbol)
}

fn method_registry_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializers: Word,
    qualifier: Word,
    function: Word,
) -> Result<Word, ObjectError> {
    lisp_list(ctx, runtime, &[specializers, qualifier, function])
}

fn method_qualifier(ctx: &ThreadContext, value: Word) -> Result<Option<Word>, ObjectError> {
    if !matches!(classify_object(ctx, value), ObjectRef::Symbol(_)) {
        return Ok(None);
    }
    let name = symbol_name_string(ctx, value)?;
    let qualifier = if matches!(name.as_str(), ":BEFORE" | "BEFORE") {
        METHOD_QUALIFIER_BEFORE
    } else if matches!(name.as_str(), ":AFTER" | "AFTER") {
        METHOD_QUALIFIER_AFTER
    } else if matches!(name.as_str(), ":AROUND" | "AROUND") {
        METHOD_QUALIFIER_AROUND
    } else {
        return Ok(None);
    };
    Ok(Some(Word::fixnum(qualifier)))
}

fn method_definition_parts(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut encoded: Word,
) -> Result<(Word, Word), ObjectError> {
    ncl_object::with_root(ctx, &mut encoded, |ctx, encoded| {
        let fields = form_elements(ctx, *encoded)?;
        let tag = ncl_symbol(ctx, runtime, "*CLOS-METHOD-DEFINITION*")?;
        if fields.first().copied() == Some(tag) {
            let qualifier = *fields.get(1).ok_or(ObjectError::TypeError)?;
            let specializers = *fields.get(2).ok_or(ObjectError::TypeError)?;
            return Ok((specializers, qualifier));
        }
        Ok((*encoded, Word::fixnum(METHOD_QUALIFIER_PRIMARY)))
    })
}

fn method_registry_key(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ObjectError> {
    ncl_symbol(ctx, runtime, METHOD_REGISTRY_KEY_NAME)
}

fn method_registry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut name: Word,
) -> Result<Word, ObjectError> {
    ncl_object::with_root(ctx, &mut name, |ctx, name| {
        let key = method_registry_key(ctx, runtime)?;
        let mut plist = symbol_plist(ctx, *name)?;
        while plist != Word::NIL {
            let property = car(ctx, plist)?;
            if car(ctx, property)? == key {
                return cdr(ctx, property);
            }
            plist = cdr(ctx, plist)?;
        }
        Ok(Word::NIL)
    })
}

fn has_method_registry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut name: Word,
) -> Result<bool, ObjectError> {
    ncl_object::with_root(ctx, &mut name, |ctx, name| {
        let key = method_registry_key(ctx, runtime)?;
        let mut plist = symbol_plist(ctx, *name)?;
        while plist != Word::NIL {
            let property = car(ctx, plist)?;
            if car(ctx, property)? == key {
                return Ok(true);
            }
            plist = cdr(ctx, plist)?;
        }
        Ok(false)
    })
}

fn set_method_registry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    mut name: Word,
    registry: Word,
) -> Result<(), ObjectError> {
    ncl_object::with_root(ctx, &mut name, |ctx, name| {
        let key = method_registry_key(ctx, runtime)?;
        let old_plist = symbol_plist(ctx, *name)?;
        ncl_object::with_roots(ctx, &[*name, key, registry, old_plist], |ctx, roots| {
            let mut property = make_cons(
                ctx,
                runtime,
                **roots.get(1).ok_or(ObjectError::Layout)?,
                **roots.get(2).ok_or(ObjectError::Layout)?,
            )?;
            ncl_object::with_root(ctx, &mut property, |ctx, property| {
                let plist = make_cons(
                    ctx,
                    runtime,
                    *property,
                    **roots.get(3).ok_or(ObjectError::Layout)?,
                )?;
                set_symbol_plist(ctx, **roots.first().ok_or(ObjectError::Layout)?, plist)
            })
        })
    })
}

fn eql_word(ctx: &ThreadContext, left: Word, right: Word) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (classify_object(ctx, left), classify_object(ctx, right)),
        (ObjectRef::DoubleFloat(left), ObjectRef::DoubleFloat(right)) if left == right
    )
}

fn class_depth(ctx: &ThreadContext, class: Word) -> Result<usize, ObjectError> {
    let superclass = simple_vector_ref(ctx, class, CLASS_DIRECT_SUPERCLASS)?;
    if superclass == Word::NIL {
        Ok(0)
    } else {
        Ok(class_depth(ctx, superclass)? + 1)
    }
}

fn method_match(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    specializers: Word,
    arguments: &[Word],
) -> Result<Option<usize>, ObjectError> {
    let specializers = form_elements(ctx, specializers)?;
    if specializers.len() != arguments.len() {
        return Ok(None);
    }
    let mut score = 0;
    for (specializer, argument) in specializers.iter().zip(arguments) {
        let fields = form_elements(ctx, *specializer)?;
        let designator = *fields.get(1).ok_or(ObjectError::TypeError)?;
        if designator.is_cons() {
            let eql_fields = form_elements(ctx, designator)?;
            let head = eql_fields.first().copied().ok_or(ObjectError::TypeError)?;
            let head_name = symbol_name_string(ctx, head)?;
            if head_name != "EQL"
                || !eql_word(
                    ctx,
                    *argument,
                    *eql_fields.get(1).ok_or(ObjectError::TypeError)?,
                )
            {
                return Ok(None);
            }
            score += 10_000;
            continue;
        }
        if symbol_name_string(ctx, designator)? == "T" {
            continue;
        }
        let expected = class_designator(ctx, runtime, designator)?;
        let actual = class_of(ctx, runtime, *argument)?;
        if !class_is_subclass(ctx, actual, expected)? {
            return Ok(None);
        }
        score += class_depth(ctx, expected)?;
    }
    Ok(Some(score))
}
