#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin registration")]

use ncl_object::{BuiltinConvention, FunctionObject, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

#[test]
fn class_of_and_class_name_are_bound_builtins() {
    let (runtime, mut ctx) = setup();
    let class_of = function(&runtime, &mut ctx, "CLASS-OF");
    let integer = runtime.class(&mut ctx, "INTEGER").unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, class_of, &[Word::fixnum(7)]),
        Ok(integer)
    );

    let class_name = function(&runtime, &mut ctx, "CLASS-NAME");
    let name = runtime
        .call_builtin(&mut ctx, class_name, &[integer])
        .unwrap();
    assert_ne!(name, Word::UNBOUND);
}

#[test]
fn slot_builtins_round_trip_and_reject_non_instances() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, class, &[Word::UNBOUND]).unwrap();
    let slot_value = function(&runtime, &mut ctx, "SLOT-VALUE");
    let slot_set = function(&runtime, &mut ctx, "SLOT-VALUE-SET");
    assert_eq!(
        runtime.builtin_descriptor(slot_set).unwrap().convention,
        BuiltinConvention::Direct(ncl_object::Arity::exact(3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_set, &[instance, Word::fixnum(0), Word::TRUE]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_value, &[Word::NIL, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn slot_exists_p_recognizes_direct_and_inherited_slots() {
    let (runtime, mut ctx) = setup();
    let inherited_name = Word::fixnum(17);
    let direct_name = Word::fixnum(23);
    let inherited_slot =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[inherited_name, Word::fixnum(0)])
            .unwrap();
    let parent_slots =
        ncl_object::make_simple_vector(&mut ctx, &runtime, &[inherited_slot]).unwrap();
    let parent = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        parent_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let child_slots = ncl_object::make_simple_vector(&mut ctx, &runtime, &[direct_name]).unwrap();
    let child = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::NIL,
        parent,
        child_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let instance = ncl_clos::make_instance(&mut ctx, &runtime, child, &[]).unwrap();
    let slot_exists = function(&runtime, &mut ctx, "SLOT-EXISTS-P");

    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, inherited_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, direct_name]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[instance, Word::fixnum(99)]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, slot_exists, &[Word::NIL, inherited_name]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn production_table_contains_only_bound_builtins() {
    let (runtime, mut ctx) = setup();
    let production = ncl_clos::production_function_names();
    assert!(!production.is_empty());
    for identifier in production {
        let word = runtime
            .function(
                &mut ctx,
                identifier.package.as_str(),
                identifier.name.as_str(),
            )
            .unwrap_or_else(|| panic!("production builtin is not registered: {identifier:?}"));
        let function = FunctionObject::try_from(word).unwrap_or_else(|_| {
            panic!("production builtin is not a FunctionObject: {identifier:?}")
        });
        assert!(
            runtime.builtin_descriptor(function).is_some(),
            "production builtin has no descriptor: {identifier:?}"
        );
    }
}
