#![allow(missing_docs)]

use ncl_clos::{class_of, make_class, make_instance, register};
use ncl_object::{FunctionObject, Runtime, ThreadContext, Word};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    register(&runtime).expect("clos registration");
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .expect("registered function");
    runtime
        .call_builtin(ctx, FunctionObject::from(function), args)
        .expect("builtin call")
}

#[test]
fn slot_accessors_read_write_bound_and_unbound_values() {
    let (runtime, mut ctx) = setup();
    let class = runtime.class(&mut ctx, "STANDARD-OBJECT").expect("class");
    let instance = make_instance(
        &mut ctx,
        &runtime,
        class,
        &[Word::fixnum(10), Word::UNBOUND],
    )
    .expect("instance");

    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SLOT-VALUE",
            &[instance, Word::fixnum(0)]
        ),
        Word::fixnum(10)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SLOT-VALUE-SET",
            &[instance, Word::fixnum(1), Word::fixnum(22)],
        ),
        Word::fixnum(22)
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SLOT-BOUNDP",
            &[instance, Word::fixnum(1)]
        ),
        Word::TRUE
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SLOT-MAKUNBOUND",
            &[instance, Word::fixnum(1)]
        ),
        instance
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SLOT-BOUNDP",
            &[instance, Word::fixnum(1)]
        ),
        Word::NIL
    );
    assert_eq!(
        call(
            &runtime,
            &mut ctx,
            "SLOT-EXISTS-P",
            &[instance, Word::fixnum(1)]
        ),
        Word::TRUE
    );
}

#[test]
fn class_of_uses_registered_builtin_classes_and_instance_class() {
    let (runtime, mut ctx) = setup();
    let integer = runtime.class(&mut ctx, "INTEGER").expect("integer class");
    assert_eq!(class_of(&mut ctx, &runtime, Word::fixnum(7)), Ok(integer));

    let class = make_class(
        &mut ctx,
        &runtime,
        Word::fixnum(99),
        Word::NIL,
        Word::NIL,
        Word::fixnum(0),
    )
    .expect("class descriptor");
    let instance = make_instance(&mut ctx, &runtime, class, &[]).expect("instance");
    assert_eq!(call(&runtime, &mut ctx, "CLASS-OF", &[instance]), class);
}
