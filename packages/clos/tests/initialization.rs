#![allow(missing_docs)]

use ncl_object::{make_simple_vector, FunctionObject, Runtime, ThreadContext, Word};

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
