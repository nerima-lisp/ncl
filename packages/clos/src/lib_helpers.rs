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
#[allow(clippy::expect_used)]
mod helper_tests {
    use super::*;
    use ncl_object::{make_cons, make_double, Package};

    fn setup() -> (Runtime, ThreadContext) {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("context");
        crate::register(&runtime).expect("clos registration");
        (runtime, ctx)
    }

    fn list(ctx: &mut ThreadContext, runtime: &Runtime, values: &[Word]) -> Word {
        let mut scope = Scope::new(ctx);
        let roots = scope.root_many(
            &values
                .iter()
                .copied()
                .map(Local::from_word)
                .collect::<Vec<_>>(),
        );
        let result = scope.make_list(runtime, &roots).expect("list");
        scope.get(result).as_word()
    }

    fn intern(ctx: &mut ThreadContext, runtime: &Runtime, package: &str, name: &str) -> Word {
        Package::from_word(runtime.find_package(ctx, package).expect("package"))
            .intern(ctx, runtime, name)
            .expect("symbol")
            .0
    }

    #[test]
    fn form_elements_rejects_dotted_lists_and_method_qualifiers_map_to_values() {
        let (runtime, mut ctx) = setup();
        let dotted = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).expect("dotted");
        assert_eq!(form_elements(&ctx, dotted), Err(ObjectError::TypeError));

        for (name, expected) in [
            ("BEFORE", METHOD_QUALIFIER_BEFORE),
            (":BEFORE", METHOD_QUALIFIER_BEFORE),
            ("AFTER", METHOD_QUALIFIER_AFTER),
            (":AFTER", METHOD_QUALIFIER_AFTER),
            ("AROUND", METHOD_QUALIFIER_AROUND),
            (":AROUND", METHOD_QUALIFIER_AROUND),
        ] {
            let symbol = intern(&mut ctx, &runtime, "NCL", name);
            assert_eq!(method_qualifier(&ctx, symbol), Ok(Some(Word::fixnum(expected))));
        }
        assert_eq!(method_qualifier(&ctx, Word::fixnum(0)), Ok(None));
        let primary = intern(&mut ctx, &runtime, "NCL", "PRIMARY");
        assert_eq!(method_qualifier(&ctx, primary), Ok(None));
    }

    #[test]
    fn eql_word_distinguishes_double_float_objects_with_equal_payloads() {
        let (runtime, mut ctx) = setup();
        let left = make_double(&mut ctx, &runtime, 2.5).expect("double").as_word();
        let same = make_double(&mut ctx, &runtime, 2.5).expect("double").as_word();
        let different = make_double(&mut ctx, &runtime, 3.5).expect("double").as_word();

        assert!(eql_word(&ctx, left, left));
        assert!(!eql_word(&ctx, left, same));
        assert!(!eql_word(&ctx, left, different));
        assert!(!eql_word(&ctx, left, Word::fixnum(2)));
    }

    #[test]
    fn method_definition_parts_support_tagged_and_plain_encodings() {
        let (runtime, mut ctx) = setup();
        let tag = intern(&mut ctx, &runtime, "NCL", "*CLOS-METHOD-DEFINITION*");
        let qualifier = Word::fixnum(METHOD_QUALIFIER_AFTER);
        let specializers = list(&mut ctx, &runtime, &[Word::fixnum(10)]);
        let tagged = list(&mut ctx, &runtime, &[tag, qualifier, specializers]);
        let mut scope = Scope::new(&mut ctx);
        let encoded = scope.root(Local::from_word(tagged));
        let (actual_specializers, actual_qualifier) =
            method_definition_parts(&mut scope, &runtime, encoded).expect("tagged definition");
        assert_eq!(scope.get(actual_specializers).as_word(), specializers);
        assert_eq!(scope.get(actual_qualifier).as_word(), qualifier);

        let plain = scope.root(Local::from_word(specializers));
        let (actual_plain, primary) =
            method_definition_parts(&mut scope, &runtime, plain).expect("plain definition");
        assert_eq!(scope.get(actual_plain).as_word(), specializers);
        assert_eq!(scope.get(primary).as_word(), Word::fixnum(METHOD_QUALIFIER_PRIMARY));
    }

    #[test]
    fn method_match_returns_value_scores_for_length_eql_and_class_cases() {
        let (runtime, mut ctx) = setup();
        let variable = intern(&mut ctx, &runtime, "NCL", "VALUE");
        let t = intern(&mut ctx, &runtime, "COMMON-LISP", "T");
        let integer = intern(&mut ctx, &runtime, "COMMON-LISP", "INTEGER");
        let eql = intern(&mut ctx, &runtime, "COMMON-LISP", "EQL");
        let class_specializer = list(&mut ctx, &runtime, &[variable, integer]);
        let t_specializer = list(&mut ctx, &runtime, &[variable, t]);
        let eql_form = list(&mut ctx, &runtime, &[eql, Word::fixnum(7)]);
        let eql_specializer = list(&mut ctx, &runtime, &[variable, eql_form]);
        let class_method = list(&mut ctx, &runtime, &[class_specializer]);
        let t_method = list(&mut ctx, &runtime, &[t_specializer]);
        let eql_method = list(&mut ctx, &runtime, &[eql_specializer]);

        assert_eq!(method_match(&mut ctx, &runtime, class_method, &[Word::fixnum(7)]), Ok(Some(1)));
        assert_eq!(method_match(&mut ctx, &runtime, t_method, &[Word::fixnum(7)]), Ok(Some(0)));
        assert_eq!(method_match(&mut ctx, &runtime, eql_method, &[Word::fixnum(7)]), Ok(Some(10_000)));
        assert_eq!(method_match(&mut ctx, &runtime, eql_method, &[Word::fixnum(8)]), Ok(None));
        assert_eq!(method_match(&mut ctx, &runtime, t_method, &[]), Ok(None));
    }

    #[test]
    fn method_registry_reports_absence_then_returns_the_registered_value() {
        let (runtime, mut ctx) = setup();
        let name = intern(&mut ctx, &runtime, "NCL", "REGISTRY-TEST");
        let registry = list(&mut ctx, &runtime, &[Word::fixnum(42)]);
        let mut scope = Scope::new(&mut ctx);
        let name = scope.root(Local::from_word(name));
        assert_eq!(has_method_registry(&mut scope, &runtime, name), Ok(false));
        assert_eq!(method_registry(&mut scope, &runtime, name).map(|h| scope.get(h).as_word()), Ok(Word::NIL));
        let registry = scope.root(Local::from_word(registry));
        set_method_registry(&mut scope, &runtime, name, registry).expect("set registry");
        assert_eq!(has_method_registry(&mut scope, &runtime, name), Ok(true));
        assert_eq!(method_registry(&mut scope, &runtime, name).map(|h| scope.get(h).as_word()), Ok(scope.get(registry).as_word()));
    }

    #[test]
    fn helper_form_builders_preserve_their_exact_shapes() {
        let (runtime, mut ctx) = setup();
        let name = intern(&mut ctx, &runtime, "NCL", "ACCESSOR");
        let slot = intern(&mut ctx, &runtime, "NCL", "SLOT");
        let value = intern(&mut ctx, &runtime, "NCL", "VALUE");
        let mut scope = Scope::new(&mut ctx);
        let name = scope.root(Local::from_word(name));
        let slot = scope.root(Local::from_word(slot));
        let value = scope.root(Local::from_word(value));

        let quoted_form = quoted(&mut scope, &runtime, value).expect("quoted form");
        let quoted_word = scope.get(quoted_form).as_word();
        assert_eq!(form_elements(scope.context(), quoted_word).unwrap().len(), 2);

        let definition = make_accessor_definition(&mut scope, &runtime, name, slot)
            .expect("accessor definition");
        assert_eq!(form_elements(scope.context(), scope.get(definition).as_word()).unwrap().len(), 4);

        let value_local = Local::from_word(scope.get(value).as_word());
        let forms = scope.root_many(&[value_local]);
        let progn = make_progn(&mut scope, &runtime, &forms).expect("progn");
        assert_eq!(form_elements(scope.context(), scope.get(progn).as_word()).unwrap().len(), 2);

        let entry = method_registry_entry(&mut scope, &runtime, value, slot, name)
            .expect("method registry entry");
        assert_eq!(form_elements(scope.context(), scope.get(entry).as_word()).unwrap().len(), 3);
    }

    #[test]
    fn class_depth_reads_the_registered_superclass_chain() {
        let (runtime, mut ctx) = setup();
        let integer = runtime.class(&mut ctx, "INTEGER").expect("integer class");
        let t = runtime.class(&mut ctx, "T").expect("root class");
        assert_eq!(class_depth(&ctx, integer), Ok(1));
        assert_eq!(class_depth(&ctx, t), Ok(0));
    }
}
