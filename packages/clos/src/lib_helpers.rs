fn form_elements(ctx: &ThreadContext, mut form: Word) -> Result<Vec<Word>, ObjectError> {
    let mut result = Vec::new();
    while form != Word::NIL {
        if !form.is_cons() {
            return Err(ObjectError::TypeError);
        }
        result.push(ncl_object::car(ctx, form)?);
        form = ncl_object::cdr(ctx, form)?;
    }
    Ok(result)
}

fn symbol_name_string(ctx: &ThreadContext, symbol: Word) -> Result<String, ObjectError> {
    let name = ncl_object::symbol_name(ctx, symbol)?;
    let length = ncl_object::string_length(ctx, name)?;
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index))
        .collect()
}

fn push_helper_handle<'ctx>(
    scope: &mut Scope<'ctx>,
    values: &mut ncl_object::HandleVec<'ctx>,
    handle: ncl_object::Handle<'ctx>,
) {
    let local = Local::from_word(scope.get(handle).as_word());
    values.push(scope, local);
}

fn quoted<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    value: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let quote = scope.intern(runtime, COMMON_LISP, "QUOTE")?;
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, quote);
    push_helper_handle(scope, &mut values, value);
    scope.make_list(runtime, &values)
}

fn make_accessor_definition<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    slot: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let instance = scope.intern(runtime, "NCL", "INSTANCE")?;
    let mut lambda_values = scope.root_many(&[]);
    push_helper_handle(scope, &mut lambda_values, instance);
    let lambda = scope.make_list(runtime, &lambda_values)?;
    let slot_value = scope.intern(runtime, COMMON_LISP, "SLOT-VALUE")?;
    let quoted_slot = quoted(scope, runtime, slot)?;
    let mut body_values = scope.root_many(&[]);
    push_helper_handle(scope, &mut body_values, slot_value);
    push_helper_handle(scope, &mut body_values, instance);
    push_helper_handle(scope, &mut body_values, quoted_slot);
    let body = scope.make_list(runtime, &body_values)?;
    let mut definition = scope.root_many(&[]);
    push_helper_handle(scope, &mut definition, defun);
    push_helper_handle(scope, &mut definition, name);
    push_helper_handle(scope, &mut definition, lambda);
    push_helper_handle(scope, &mut definition, body);
    scope.make_list(runtime, &definition)
}

fn make_progn<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    forms: &ncl_object::HandleVec<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let progn = scope.intern(runtime, COMMON_LISP, "PROGN")?;
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, progn);
    for form in forms.iter().copied() {
        push_helper_handle(scope, &mut values, form);
    }
    scope.make_list(runtime, &values)
}

fn method_registry_entry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    specializers: ncl_object::Handle<'ctx>,
    qualifier: ncl_object::Handle<'ctx>,
    function: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let mut values = scope.root_many(&[]);
    push_helper_handle(scope, &mut values, specializers);
    push_helper_handle(scope, &mut values, qualifier);
    push_helper_handle(scope, &mut values, function);
    scope.make_list(runtime, &values)
}

fn method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    let mut plist = ncl_object::symbol_plist(scope.context(), scope.get(name).as_word())?;
    while plist != Word::NIL {
        let property = ncl_object::car(scope.context(), plist)?;
        if ncl_object::car(scope.context(), property)? == scope.get(key).as_word() {
            return Ok(scope.root(Local::from_word(ncl_object::cdr(
                scope.context(),
                property,
            )?)));
        }
        plist = ncl_object::cdr(scope.context(), plist)?;
    }
    Ok(scope.root(Local::from_word(Word::NIL)))
}

fn has_method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
) -> Result<bool, ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    let mut plist = ncl_object::symbol_plist(scope.context(), scope.get(name).as_word())?;
    while plist != Word::NIL {
        let property = ncl_object::car(scope.context(), plist)?;
        if ncl_object::car(scope.context(), property)? == scope.get(key).as_word() {
            return Ok(true);
        }
        plist = ncl_object::cdr(scope.context(), plist)?;
    }
    Ok(false)
}

fn set_method_registry<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    name: ncl_object::Handle<'ctx>,
    registry: ncl_object::Handle<'ctx>,
) -> Result<(), ObjectError> {
    let key = method_registry_key(scope, runtime)?;
    let old_plist = scope.root(Local::from_word(ncl_object::symbol_plist(
        scope.context(),
        scope.get(name).as_word(),
    )?));
    let property = scope.make_cons(runtime, key, registry)?;
    let plist = scope.make_cons(runtime, property, old_plist)?;
    let name_word = scope.get(name).as_word();
    let plist_word = scope.get(plist).as_word();
    ncl_object::set_symbol_plist(scope.context_mut(), name_word, plist_word)
}

fn method_registry_key<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
) -> Result<ncl_object::Handle<'ctx>, ObjectError> {
    scope.intern(runtime, "NCL", METHOD_REGISTRY_KEY_NAME)
}

fn method_definition_parts<'ctx>(
    scope: &mut Scope<'ctx>,
    runtime: &Runtime,
    encoded: ncl_object::Handle<'ctx>,
) -> Result<(ncl_object::Handle<'ctx>, ncl_object::Handle<'ctx>), ObjectError> {
    let encoded_local = Local::from_word(scope.get(encoded).as_word());
    let fields = scope.list_to_handle_vec(encoded_local)?;
    let tag = scope.intern(runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
    if fields
        .as_slice()
        .first()
        .is_some_and(|field| scope.get(*field).as_word() == scope.get(tag).as_word())
    {
        let qualifier = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
        let specializers = *fields.as_slice().get(2).ok_or(ObjectError::TypeError)?;
        return Ok((specializers, qualifier));
    }
    let qualifier = scope.root(Local::from_word(Word::fixnum(METHOD_QUALIFIER_PRIMARY)));
    Ok((encoded, qualifier))
}

fn method_qualifier(ctx: &ThreadContext, value: Word) -> Result<Option<Word>, ObjectError> {
    if !matches!(ncl_object::classify_object(ctx, value), ObjectRef::Symbol(_)) {
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

fn eql_word(ctx: &ThreadContext, left: Word, right: Word) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (ncl_object::classify_object(ctx, left), ncl_object::classify_object(ctx, right)),
        (ObjectRef::DoubleFloat(left), ObjectRef::DoubleFloat(right)) if left == right
    )
}

fn class_depth(ctx: &ThreadContext, class: Word) -> Result<usize, ObjectError> {
    let superclass = ncl_object::simple_vector_ref(ctx, class, CLASS_DIRECT_SUPERCLASS)?;
    // A class with more than one direct superclass (for example a
    // multiple-inheritance condition class from `ncl-conditions`) stores a
    // list of parent descriptors here rather than a bare descriptor word.
    // Method-dispatch specificity depth is not otherwise a concern for those
    // classes in Phase 1, so the first (most-specific) parent stands in for
    // the whole precedence list.
    let superclass = if superclass.is_cons() {
        car(ctx, superclass)?
    } else {
        superclass
    };
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

#[cfg(test)]
mod helper_tests {
    #![allow(
        clippy::expect_used,
        clippy::similar_names,
        reason = "coverage tests assert on helper results"
    )]

    use super::*;

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut context = ThreadContext::new();
        context.register(&runtime).expect("context");
        (runtime, context)
    }

    fn list<'ctx>(
        scope: &mut Scope<'ctx>,
        runtime: &Runtime,
        words: &[Word],
    ) -> ncl_object::Handle<'ctx> {
        let locals = words
            .iter()
            .copied()
            .map(Local::from_word)
            .collect::<Vec<_>>();
        let values = scope.root_many(&locals);
        scope.make_list(runtime, &values).expect("list")
    }

    fn word<'ctx>(scope: &Scope<'ctx>, handle: ncl_object::Handle<'ctx>) -> Word {
        scope.get(handle).as_word()
    }

    #[test]
    fn list_and_symbol_helpers_validate_object_shapes() -> Result<(), ObjectError> {
        let (runtime, mut context) = setup();
        let mut scope = Scope::new(&mut context);
        let symbol = scope.intern(&runtime, COMMON_LISP, "HELPER")?;
        let symbol_word = word(&scope, symbol);
        let proper = list(&mut scope, &runtime, &[symbol_word, Word::fixnum(7)]);

        assert_eq!(form_elements(scope.context(), word(&scope, proper))?.len(), 2);
        assert_eq!(symbol_name_string(scope.context(), symbol_word)?, "HELPER");
        assert_eq!(form_elements(scope.context(), Word::fixnum(0)), Err(ObjectError::TypeError));
        assert_eq!(symbol_name_string(scope.context(), Word::fixnum(0)), Err(ObjectError::TypeError));
        Ok(())
    }

    #[test]
    fn list_construction_and_method_registry_helpers_round_trip() -> Result<(), ObjectError> {
        let (runtime, mut context) = setup();
        let mut scope = Scope::new(&mut context);
        let name = scope.intern(&runtime, COMMON_LISP, "HELPER-GENERIC")?;
        let slot = scope.intern(&runtime, COMMON_LISP, "VALUE")?;
        let accessor = scope.intern(&runtime, COMMON_LISP, "VALUE-OF")?;
        let quoted_slot = quoted(&mut scope, &runtime, slot)?;
        let quote = scope.intern(&runtime, COMMON_LISP, "QUOTE")?;
        assert_eq!(car(scope.context(), word(&scope, quoted_slot))?, word(&scope, quote));

        let accessor_form = make_accessor_definition(&mut scope, &runtime, accessor, slot)?;
        assert_eq!(form_elements(scope.context(), word(&scope, accessor_form))?.len(), 4);
        let accessor_local = Local::from_word(word(&scope, accessor_form));
        let accessor_values = scope.root_many(&[accessor_local]);
        let progn = make_progn(&mut scope, &runtime, &accessor_values)?;
        assert_eq!(form_elements(scope.context(), word(&scope, progn))?.len(), 2);

        let specializers = scope.root(Local::from_word(Word::NIL));
        let qualifier = scope.root(Local::from_word(Word::fixnum(METHOD_QUALIFIER_PRIMARY)));
        let function = scope.root(Local::from_word(Word::fixnum(42)));
        let entry = method_registry_entry(&mut scope, &runtime, specializers, qualifier, function)?;
        set_method_registry(&mut scope, &runtime, name, entry)?;
        assert!(has_method_registry(&mut scope, &runtime, name)?);
        let registry = method_registry(&mut scope, &runtime, name)?;
        assert_ne!(word(&scope, registry), Word::NIL);
        Ok(())
    }

    #[test]
    fn method_definition_and_dispatch_helpers_cover_tagged_and_eql_forms() -> Result<(), ObjectError> {
        let (runtime, mut context) = setup();
        let mut scope = Scope::new(&mut context);
        let tag = scope.intern(&runtime, "NCL", "*CLOS-METHOD-DEFINITION*")?;
        let eql = scope.intern(&runtime, COMMON_LISP, "EQL")?;
        let parameter = scope.intern(&runtime, COMMON_LISP, "X")?;
        let tag_word = word(&scope, tag);
        let parameter_word = word(&scope, parameter);
        let tagged = list(
            &mut scope,
            &runtime,
            &[tag_word, Word::fixnum(METHOD_QUALIFIER_AFTER), parameter_word],
        );
        let (specializers, qualifier) = method_definition_parts(&mut scope, &runtime, tagged)?;
        assert_eq!(word(&scope, specializers), word(&scope, parameter));
        assert_eq!(word(&scope, qualifier), Word::fixnum(METHOD_QUALIFIER_AFTER));

        let untagged = list(&mut scope, &runtime, &[parameter_word]);
        let (_, primary) = method_definition_parts(&mut scope, &runtime, untagged)?;
        assert_eq!(word(&scope, primary), Word::fixnum(METHOD_QUALIFIER_PRIMARY));

        let eql_symbol_word = word(&scope, eql);
        let eql_designator = list(&mut scope, &runtime, &[eql_symbol_word, Word::fixnum(9)]);
        let eql_designator_word = word(&scope, eql_designator);
        let specializer = list(&mut scope, &runtime, &[parameter_word, eql_designator_word]);
        let specializer_word = word(&scope, specializer);
        let specializers = list(&mut scope, &runtime, &[specializer_word]);
        let specializers_word = word(&scope, specializers);
        assert_eq!(method_match(scope.context_mut(), &runtime, specializers_word, &[Word::fixnum(9)])?, Some(10_000));
        assert_eq!(method_match(scope.context_mut(), &runtime, specializers_word, &[Word::fixnum(8)])?, None);
        assert!(eql_word(scope.context(), Word::fixnum(1), Word::fixnum(1)));
        assert!(!eql_word(scope.context(), Word::fixnum(1), Word::fixnum(2)));
        Ok(())
    }

    #[test]
    fn class_depth_counts_parent_descriptors() -> Result<(), ObjectError> {
        let (runtime, mut context) = setup();
        let root = make_class(&mut context, &runtime, Word::fixnum(1), Word::NIL, Word::NIL, Word::NIL)?;
        let child = make_class(&mut context, &runtime, Word::fixnum(2), root, Word::NIL, Word::NIL)?;
        assert_eq!(class_depth(&context, root)?, 0);
        assert_eq!(class_depth(&context, child)?, 1);
        assert_eq!(class_depth(&context, Word::fixnum(0)), Err(ObjectError::TypeError));
        Ok(())
    }
}
