#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on MOP registration")]

#[path = "../src/mop.rs"]
mod mop;

use ncl_object::{
    BuiltinArgs, BuiltinPackage, MultipleValues, Runtime, ThreadContext, Word, make_simple_vector,
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

    let eql = mop::make_eql_specializer(&mut ctx, &runtime, Word::fixnum(42)).unwrap();
    assert_eq!(
        call(&mut ctx, &runtime, "EQL-SPECIALIZER-OBJECT", &[eql]),
        Ok(Word::fixnum(42))
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
