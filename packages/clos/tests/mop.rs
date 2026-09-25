#![allow(missing_docs)]

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
    assert_eq!(call(&mut ctx, &runtime, "CLASS-SLOTS", &[class]), Ok(slots));
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
