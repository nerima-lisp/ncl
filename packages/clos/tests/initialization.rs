#![allow(missing_docs)]
#![allow(
    clippy::unwrap_used,
    reason = "tests assert on initialization protocol"
)]

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word, make_simple_vector};

#[path = "../src/initialization.rs"]
mod initialization;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_clos::register(&runtime).unwrap();
    initialization::register(&runtime).unwrap();
    (runtime, ctx)
}

fn function(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

fn class_with_slots(ctx: &mut ThreadContext, runtime: &Runtime, keys: &[Word]) -> Word {
    let slots = make_simple_vector(ctx, runtime, keys).unwrap();
    ncl_clos::make_class(ctx, runtime, Word::NIL, Word::NIL, slots, Word::fixnum(0)).unwrap()
}

#[test]
fn make_instance_applies_initargs_and_initialize_instance_returns_instance() {
    let (runtime, mut ctx) = setup();
    let key = Word::fixnum(17);
    let class = class_with_slots(&mut ctx, &runtime, &[key]);
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let instance = runtime
        .call_builtin(&mut ctx, make, &[class, key, Word::TRUE])
        .unwrap();
    let slot_value = function(&runtime, &mut ctx, "COMMON-LISP", "SLOT-VALUE");
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)])
            .unwrap(),
        Word::TRUE
    );

    let initialize = function(&runtime, &mut ctx, "COMMON-LISP", "INITIALIZE-INSTANCE");
    let result = runtime
        .call_builtin(&mut ctx, initialize, &[instance, key, Word::NIL])
        .unwrap();
    assert_eq!(result, instance);
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)])
            .unwrap(),
        Word::NIL
    );
}

#[test]
fn shared_initialize_rejects_odd_initargs_and_non_instance() {
    let (runtime, mut ctx) = setup();
    let shared = function(&runtime, &mut ctx, "COMMON-LISP", "SHARED-INITIALIZE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, shared, &[Word::NIL, Word::fixnum(1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, shared, &[Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn make_instance_initializes_slots_inherited_through_three_generations() {
    let (runtime, mut ctx) = setup();
    let root_slot = Word::fixnum(101);
    let middle_slot = Word::fixnum(102);
    let leaf_slot = Word::fixnum(103);
    let root = class_with_slots(&mut ctx, &runtime, &[root_slot]);
    let middle_slots = make_simple_vector(&mut ctx, &runtime, &[middle_slot]).unwrap();
    let middle = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(201),
        root,
        middle_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let leaf_slots = make_simple_vector(&mut ctx, &runtime, &[leaf_slot]).unwrap();
    let leaf = ncl_clos::make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(202),
        middle,
        leaf_slots,
        Word::fixnum(0),
    )
    .unwrap();
    let make = function(&runtime, &mut ctx, "COMMON-LISP", "MAKE-INSTANCE");
    let instance = runtime
        .call_builtin(
            &mut ctx,
            make,
            &[
                leaf,
                root_slot,
                Word::fixnum(11),
                middle_slot,
                Word::fixnum(22),
                leaf_slot,
                Word::fixnum(33),
            ],
        )
        .unwrap();
    let slot_value = function(&runtime, &mut ctx, "COMMON-LISP", "SLOT-VALUE");
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(0)])
            .unwrap(),
        Word::fixnum(11)
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(1)])
            .unwrap(),
        Word::fixnum(22)
    );
    assert_eq!(
        runtime
            .call_builtin(&mut ctx, slot_value, &[instance, Word::fixnum(2)])
            .unwrap(),
        Word::fixnum(33)
    );
}
