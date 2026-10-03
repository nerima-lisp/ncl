#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on MOP registration")]

#[path = "../src/mop.rs"]
mod mop;

use ncl_object::{
    BuiltinArgs, BuiltinPackage, FunctionObject, MultipleValues, Runtime, ThreadContext, Word,
    make_simple_vector,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ncl_object::ObjectError> {
    let descriptor = mop::builtin_descriptors()
        .iter()
        .find(|descriptor| descriptor.name.as_str() == name)
        .unwrap();
    let mut values = MultipleValues::new();
    (descriptor.callback)(ctx, runtime, &BuiltinArgs::new(args), &mut values)
}

fn registered_call(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    args: &[Word],
) -> Result<Word, ncl_object::ObjectError> {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "NCL-MOP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, args)
}

#[test]
fn descriptors_expose_typed_class_and_slot_metadata() {
    let (runtime, mut ctx) = setup();
    let superclass = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let name = ncl_object::make_string(&mut ctx, &runtime, &['X']).unwrap();
    let slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();
    let slots = make_simple_vector(&mut ctx, &runtime, &[slot]).unwrap();
    let class =
        ncl_clos::make_class(&mut ctx, &runtime, name, superclass, slots, Word::fixnum(0)).unwrap();

    let precedence = call(&mut ctx, &runtime, "CLASS-PRECEDENCE-LIST", &[class]).unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, precedence).unwrap(),
        3
    );
    let effective_slots = call(&mut ctx, &runtime, "CLASS-SLOTS", &[class]).unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, effective_slots),
        Ok(1)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, effective_slots, 0),
        Ok(slot)
    );
    assert_eq!(
        call(&mut ctx, &runtime, "SLOT-DEFINITION-NAME", &[slot]),
        Ok(name)
    );
    assert_eq!(
        call(&mut ctx, &runtime, "SLOT-DEFINITION-LOCATION", &[slot]),
        Ok(Word::fixnum(0))
    );
}

#[test]
fn registered_mop_class_queries_return_direct_and_effective_metadata() {
    let (runtime, mut ctx) = setup();
    let parent = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let name = Word::fixnum(901);
    let slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        Word::fixnum(902),
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();
    let direct_slots = make_simple_vector(&mut ctx, &runtime, &[slot]).unwrap();
    let class = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        name,
        parent,
        direct_slots,
        Word::fixnum(0),
    )
    .unwrap();

    assert_eq!(
        registered_call(&mut ctx, &runtime, "CLASS-NAME", &[class]),
        Ok(name)
    );
    assert_eq!(
        registered_call(&mut ctx, &runtime, "CLASS-DIRECT-SLOTS", &[class]),
        Ok(direct_slots)
    );
    let effective_slots = registered_call(&mut ctx, &runtime, "CLASS-SLOTS", &[class]).unwrap();
    assert_eq!(
        ncl_object::simple_vector_length(&ctx, effective_slots),
        Ok(1)
    );
    let precedence =
        registered_call(&mut ctx, &runtime, "CLASS-PRECEDENCE-LIST", &[class]).unwrap();
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, precedence, 0),
        Ok(class)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, precedence, 1),
        Ok(parent)
    );
}

#[test]
fn registered_mop_callbacks_cover_metadata_and_slot_lifecycle() {
    let (runtime, mut ctx) = setup();
    let superclass = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let name = ncl_object::make_string(&mut ctx, &runtime, &['R']).unwrap();
    let slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();
    let slots = make_simple_vector(&mut ctx, &runtime, &[slot]).unwrap();
    let class =
        ncl_clos::make_class(&mut ctx, &runtime, name, superclass, slots, Word::fixnum(0)).unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();

    assert_eq!(
        registered_call(
            &mut ctx,
            &runtime,
            "SLOT-BOUNDP-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(Word::NIL)
    );
    ncl_object::slot_set(
        &mut ctx,
        ncl_object::Instance::from_word(instance),
        0,
        Word::TRUE,
    )
    .unwrap();
    assert_eq!(
        registered_call(
            &mut ctx,
            &runtime,
            "SLOT-VALUE-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        registered_call(
            &mut ctx,
            &runtime,
            "SLOT-MAKUNBOUND-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(instance)
    );
    let eql = mop::make_eql_specializer(&mut ctx, &runtime, Word::fixnum(7)).unwrap();
    assert_eq!(
        registered_call(&mut ctx, &runtime, "EQL-SPECIALIZER-OBJECT", &[eql]),
        Ok(Word::fixnum(7))
    );
}

#[test]
fn class_slots_and_class_direct_slots_distinguish_inherited_metadata() {
    let (runtime, mut ctx) = setup();
    let superclass = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let parent_name = Word::fixnum(10);
    let child_name = Word::fixnum(11);
    let parent_slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        parent_name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();
    let child_slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        child_name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(1)).unwrap()),
    )
    .unwrap();
    let parent_slots = make_simple_vector(&mut ctx, &runtime, &[parent_slot]).unwrap();
    let child_slots = make_simple_vector(&mut ctx, &runtime, &[child_slot]).unwrap();
    let parent = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        parent_name,
        superclass,
        parent_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        child_name,
        parent,
        child_slots,
        Word::fixnum(0),
    )
    .unwrap();

    assert_eq!(
        call(&mut ctx, &runtime, "CLASS-DIRECT-SLOTS", &[child]),
        Ok(child_slots)
    );
    let effective = call(&mut ctx, &runtime, "CLASS-SLOTS", &[child]).unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, effective), Ok(2));
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, effective, 0),
        Ok(parent_slot)
    );
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, effective, 1),
        Ok(child_slot)
    );
}

#[test]
fn typed_slot_accessors_and_eql_specializer_round_trip() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let name = ncl_object::make_string(&mut ctx, &runtime, &['S']).unwrap();
    let slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();

    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-BOUNDP-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(Word::NIL)
    );
    ncl_object::slot_set(
        &mut ctx,
        ncl_object::Instance::from_word(instance),
        0,
        Word::TRUE,
    )
    .unwrap();
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-VALUE-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(Word::TRUE)
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-MAKUNBOUND-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(instance)
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-BOUNDP-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-VALUE-USING-CLASS",
            &[class, instance, slot],
        ),
        Ok(Word::UNBOUND)
    );

    let eql = mop::make_eql_specializer(&mut ctx, &runtime, Word::fixnum(42)).unwrap();
    assert_eq!(
        call(&mut ctx, &runtime, "EQL-SPECIALIZER-OBJECT", &[eql]),
        Ok(Word::fixnum(42))
    );
}

#[test]
fn typed_slot_accessors_reject_wrong_objects_and_locations() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let other_class = runtime.class(&mut ctx, "CLASS").unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::TRUE]).unwrap();
    let name = Word::fixnum(31);
    let slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();
    let no_location = mop::make_slot_descriptor(&mut ctx, &runtime, name, None).unwrap();
    let negative_location = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        name,
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(-1)).unwrap()),
    )
    .unwrap();

    for name in [
        "SLOT-VALUE-USING-CLASS",
        "SLOT-BOUNDP-USING-CLASS",
        "SLOT-MAKUNBOUND-USING-CLASS",
    ] {
        assert_eq!(
            call(&mut ctx, &runtime, name, &[other_class, instance, slot]),
            Err(ncl_object::ObjectError::TypeError),
            "class mismatch must be rejected by {name}"
        );
        assert_eq!(
            call(&mut ctx, &runtime, name, &[class, Word::NIL, slot]),
            Err(ncl_object::ObjectError::TypeError),
            "non-instance must be rejected by {name}"
        );
    }
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-VALUE-USING-CLASS",
            &[class, instance, Word::fixnum(99)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    for malformed in [no_location, negative_location] {
        assert_eq!(
            call(
                &mut ctx,
                &runtime,
                "SLOT-BOUNDP-USING-CLASS",
                &[class, instance, malformed],
            ),
            Err(ncl_object::ObjectError::TypeError)
        );
    }
}

#[test]
fn class_slots_support_legacy_length_and_cons_superclass_descriptors() {
    let (runtime, mut ctx) = setup();
    let parent = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let direct_slots = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(71)]).unwrap();
    let legacy_class = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(70), Word::NIL, direct_slots, Word::fixnum(0)],
    )
    .unwrap();
    assert_eq!(
        call(&mut ctx, &runtime, "CLASS-SLOTS", &[legacy_class]),
        Ok(direct_slots)
    );

    let mut scope = ncl_object::Scope::new(&mut ctx);
    let parent_root = scope.root_many(&[ncl_object::Local::from_word(parent)]);
    let parent_list = scope.make_list(&runtime, &parent_root).unwrap();
    let parent_list = scope.get(parent_list).as_word();
    let class_values = scope.root_many(&[
        ncl_object::Local::from_word(Word::fixnum(72)),
        ncl_object::Local::from_word(parent_list),
        ncl_object::Local::from_word(Word::NIL),
        ncl_object::Local::from_word(Word::fixnum(0)),
    ]);
    let cons_superclass_class = scope.make_simple_vector(&runtime, &class_values).unwrap();
    let cons_superclass_class = scope.get(cons_superclass_class).as_word();
    drop(scope);
    let precedence = call(
        &mut ctx,
        &runtime,
        "CLASS-PRECEDENCE-LIST",
        &[cons_superclass_class],
    )
    .unwrap();
    assert_eq!(ncl_object::simple_vector_length(&ctx, precedence), Ok(3));
    assert_eq!(
        ncl_object::simple_vector_ref(&ctx, precedence, 1),
        Ok(parent)
    );
}

#[test]
fn class_slot_queries_reject_descriptors_without_slot_fields() {
    let (runtime, mut ctx) = setup();
    let malformed = make_simple_vector(&mut ctx, &runtime, &[]).unwrap();

    assert_eq!(
        call(&mut ctx, &runtime, "CLASS-SLOTS", &[malformed]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        call(&mut ctx, &runtime, "CLASS-DIRECT-SLOTS", &[malformed]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn metadata_callbacks_reject_non_descriptors() {
    let (runtime, mut ctx) = setup();
    let malformed = make_simple_vector(&mut ctx, &runtime, &[]).unwrap();

    for name in [
        "CLASS-PRECEDENCE-LIST",
        "SLOT-DEFINITION-NAME",
        "SLOT-DEFINITION-LOCATION",
        "EQL-SPECIALIZER-OBJECT",
    ] {
        assert_eq!(
            call(&mut ctx, &runtime, name, &[malformed]),
            Err(ncl_object::ObjectError::TypeError),
            "malformed metadata must be rejected by {name}"
        );
    }
}

#[test]
fn slot_accessors_reject_non_fixnum_locations() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::TRUE]).unwrap();
    let malformed = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::TRUE]).unwrap();

    for name in [
        "SLOT-VALUE-USING-CLASS",
        "SLOT-BOUNDP-USING-CLASS",
        "SLOT-MAKUNBOUND-USING-CLASS",
    ] {
        assert_eq!(
            call(&mut ctx, &runtime, name, &[class, instance, malformed]),
            Err(ncl_object::ObjectError::TypeError),
            "non-fixnum slot locations must be rejected by {name}"
        );
    }
}

#[test]
fn slot_location_nil_and_out_of_range_have_explicit_results() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::TRUE]).unwrap();
    let no_location =
        mop::make_slot_descriptor(&mut ctx, &runtime, Word::fixnum(91), None).unwrap();
    let out_of_range = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        Word::fixnum(92),
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(1)).unwrap()),
    )
    .unwrap();

    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-DEFINITION-LOCATION",
            &[no_location]
        ),
        Ok(Word::NIL)
    );
    for slot in [no_location, out_of_range] {
        assert_eq!(
            call(
                &mut ctx,
                &runtime,
                "SLOT-VALUE-USING-CLASS",
                &[class, instance, slot]
            ),
            Err(ncl_object::ObjectError::TypeError)
        );
        assert_eq!(
            call(
                &mut ctx,
                &runtime,
                "SLOT-BOUNDP-USING-CLASS",
                &[class, instance, slot]
            ),
            Err(ncl_object::ObjectError::TypeError)
        );
        assert_eq!(
            call(
                &mut ctx,
                &runtime,
                "SLOT-MAKUNBOUND-USING-CLASS",
                &[class, instance, slot]
            ),
            Err(ncl_object::ObjectError::TypeError)
        );
    }
}

#[test]
fn slot_accessors_require_the_instance_class_to_match_exactly() {
    let (runtime, mut ctx) = setup();
    let parent = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let child_name = Word::fixnum(81);
    let child_slots = make_simple_vector(&mut ctx, &runtime, &[]).unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        child_name,
        parent,
        child_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, child, &[Word::TRUE]).unwrap();
    let slot = mop::make_slot_descriptor(
        &mut ctx,
        &runtime,
        Word::fixnum(82),
        Some(ncl_object::Fixnum::try_from_word(Word::fixnum(0)).unwrap()),
    )
    .unwrap();

    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-VALUE-USING-CLASS",
            &[parent, instance, slot],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        call(
            &mut ctx,
            &runtime,
            "SLOT-VALUE-USING-CLASS",
            &[child, instance, slot],
        ),
        Ok(Word::TRUE)
    );
}

#[test]
fn descriptors_are_registration_ready() {
    for descriptor in mop::builtin_descriptors() {
        let implementation = mop::implementation(*descriptor);
        assert_eq!(implementation.descriptor, descriptor.builtin);
        assert_eq!(descriptor.package, BuiltinPackage::NclMop);
        assert!(!descriptor.name.as_str().is_empty());
    }
}
