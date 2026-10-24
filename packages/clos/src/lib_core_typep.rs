use ncl_object::HeapRootedWord;

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
    let object = HeapRootedWord::new(ctx, args.required(0)?)?;
    let type_specifier = HeapRootedWord::new(ctx, args.required(1)?)?;
    let expanded = ncl_types::expand_deftype_specifier(ctx, runtime, type_specifier.get())
        .map_err(|_| ObjectError::TypeError)?;
    let expanded = HeapRootedWord::new(ctx, expanded)?;
    Ok(
        if typep_with_class_designators(ctx, runtime, object.get(), expanded.get())? {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

fn typep_with_class_designators(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    type_specifier: Word,
) -> Result<bool, ObjectError> {
    if type_specifier == Word::NIL {
        return Ok(false);
    }
    if let Ok(expected) = class_designator(ctx, runtime, type_specifier) {
        let actual = class_of(ctx, runtime, object)?;
        if class_is_subclass(ctx, actual, expected)? {
            return Ok(true);
        }
    }
    if type_specifier.is_cons() {
        let operator = car(ctx, type_specifier)?;
        if matches!(classify_object(ctx, operator), ObjectRef::Symbol(_)) {
            let name = symbol_name_string(ctx, operator)?;
            if matches!(name.as_str(), "AND" | "OR" | "NOT") {
                let mut forms = cdr(ctx, type_specifier)?;
                if name == "NOT" {
                    if !forms.is_cons() {
                        return Ok(false);
                    }
                    let form = car(ctx, forms)?;
                    if cdr(ctx, forms)? != Word::NIL {
                        return Ok(false);
                    }
                    return Ok(!typep_with_class_designators(ctx, runtime, object, form)?);
                }
                let conjunction = name == "AND";
                while forms != Word::NIL {
                    if !forms.is_cons() {
                        return Ok(false);
                    }
                    let form = car(ctx, forms)?;
                    let matches = typep_with_class_designators(ctx, runtime, object, form)?;
                    if (conjunction && !matches) || (!conjunction && matches) {
                        return Ok(!conjunction);
                    }
                    forms = cdr(ctx, forms)?;
                }
                return Ok(conjunction);
            }
        }
    }
    Ok(ncl_types::parse_type_specifier(ctx, type_specifier)
        .and_then(|specifier| ncl_types::typep(ctx, object, &specifier))
        .unwrap_or(false))
}

fn class_name_builtin(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    class_name(ctx, args.required(0)?)
}
