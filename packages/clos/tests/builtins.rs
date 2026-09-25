#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin registration")]

use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

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
