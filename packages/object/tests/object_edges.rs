#![allow(missing_docs)]

use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, car, cdr, classify, classify_object,
    make_simple_vector, make_string, simple_vector_ref, string_ref,
};
use ncl_sys::{LowTag, StorageCondition, Word};

#[test]
fn classify_covers_immediate_and_pointer_lowtags() {
    assert_eq!(
        classify(Word::TRUE),
        ObjectRef::Other {
            word: Word::TRUE,
            widetag: 0,
        }
    );
    let function = Word::pointer(0x1000, LowTag::Function);
    let instance = Word::pointer(0x2000, LowTag::Instance);
    assert_eq!(classify(function), ObjectRef::Function(function));
    assert_eq!(classify(instance), ObjectRef::Instance(instance));
    let other = Word::pointer(0x3000, LowTag::OtherPointer);
    assert_eq!(
        classify(other),
        ObjectRef::Other {
            word: other,
            widetag: 0,
        }
    );
}

#[test]
fn runtime_and_context_state_report_observed_values() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert_eq!(
        ctx.collect(false),
        Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
    );
    assert_eq!(ctx.unbind(7), Err(ObjectError::Unbound));
    ctx.set_values(&[Word::fixnum(1), Word::NIL]);
    assert_eq!(ctx.values(), &[Word::fixnum(1), Word::NIL]);
    ctx.set_pending(ObjectError::Unsupported);
    assert_eq!(ctx.take_pending(), Some(ObjectError::Unsupported));

    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register failed: {error:?}"));
    assert!(runtime.register_layouts().is_ok());
    assert_eq!(runtime.widetag(Word::NIL), None);
    let value = Word::fixnum(42);
    runtime
        .define_function(&mut ctx, "TEST", "VALUE", value)
        .unwrap_or_else(|error| panic!("define_function failed: {error:?}"));
    assert_eq!(runtime.function(&mut ctx, "TEST", "VALUE"), Some(value));
    assert_eq!(runtime.function(&mut ctx, "TEST", "MISSING"), None);
}

#[test]
fn accessors_return_type_and_bounds_errors() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register failed: {error:?}"));
    assert_eq!(car(&mut ctx, Word::fixnum(1)), Err(ObjectError::TypeError));
    assert_eq!(cdr(&mut ctx, Word::fixnum(1)), Err(ObjectError::TypeError));

    let string = make_string(&mut ctx, &runtime, &['x']).unwrap_or(Word::NIL);
    assert_eq!(string_ref(&ctx, Word::NIL, 0), Err(ObjectError::TypeError));
    assert_eq!(string_ref(&ctx, string, 1), Err(ObjectError::TypeError));
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(9)]).unwrap_or(Word::NIL);
    assert_eq!(
        simple_vector_ref(&ctx, vector, 1),
        Err(ObjectError::TypeError)
    );
    assert_eq!(classify_object(&ctx, Word::UNBOUND), ObjectRef::Fixnum(2));
}
