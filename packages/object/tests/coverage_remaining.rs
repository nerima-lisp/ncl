//! Integration coverage for runtime, structure, stream, and object boundary APIs.

use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, WordView, classify, classify_object, code_slot,
    make_cons, make_instance, rplaca, rplacd, slot_ref, slot_set,
};
use ncl_readtables::{make_readtable, readtable_slot};
use ncl_streams::{make_stream, stream_slot};
use ncl_sys::Word;

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut context = ThreadContext::new();
    context
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    (runtime, context)
}

#[test]
fn runtime_context_roots_and_classification_report_values() {
    let (runtime, mut context) = setup();
    assert_eq!(runtime.widetag(Word::NIL), None);
    assert!(
        runtime
            .define_function(&mut context, "COVERAGE", "FUNCTION", Word::TRUE)
            .is_ok()
    );
    assert_eq!(
        runtime.function(&mut context, "COVERAGE", "FUNCTION"),
        Some(Word::TRUE)
    );
    assert_eq!(runtime.function(&mut context, "COVERAGE", "MISSING"), None);
    context.bind(7, Word::fixnum(4));
    context.bind(7, Word::fixnum(5));
    assert_eq!(context.unbind(7), Ok(Word::fixnum(5)));
    assert_eq!(context.unbind(7), Ok(Word::fixnum(4)));
    assert_eq!(context.unbind(7), Err(ObjectError::Unbound));
    context.set_values(&[Word::fixnum(1), Word::TRUE]);
    assert_eq!(context.values(), &[Word::fixnum(1), Word::TRUE]);
    context.set_pending(ObjectError::Layout);
    assert_eq!(context.take_pending(), Some(ObjectError::Layout));
    assert_eq!(context.take_pending(), None);
    context.set_strict_forwarding(true);
    context.set_strict_forwarding(false);
    context.set_gc_stress(true);
    assert!(ncl_object::make_cons(&mut context, &runtime, Word::NIL, Word::TRUE).is_ok());
    context.set_gc_stress(false);
    assert_eq!(ncl_object::car(&context, Word::NIL), Ok(Word::NIL));
    assert_eq!(ncl_object::cdr(&context, Word::NIL), Ok(Word::NIL));
    assert_eq!(
        ncl_object::car(&context, Word::TRUE),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        ncl_object::cdr(&context, Word::TRUE),
        Err(ObjectError::TypeError)
    );
    assert_eq!(classify(Word::character(65)), ObjectRef::Character(65));
    assert_eq!(classify(Word::fixnum(-2)), ObjectRef::Fixnum(-2));
    assert_eq!(
        classify_object(&context, Word::NIL),
        ObjectRef::Symbol(Word::NIL)
    );
    assert_eq!(WordView::from(classify(Word::TRUE)).as_word(), Word::TRUE);
}

#[test]
fn raw_slots_and_mutators_report_boundary_errors() {
    let (runtime, mut context) = setup();
    let cons = make_cons(&mut context, &runtime, Word::NIL, Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(
        rplaca(&mut context, Word::TRUE, Word::NIL),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        rplacd(&mut context, Word::TRUE, Word::NIL),
        Err(ObjectError::TypeError)
    );

    let mut unregistered = ThreadContext::new();
    assert_eq!(
        rplaca(&mut unregistered, cons, Word::TRUE),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
    assert_eq!(
        rplacd(&mut unregistered, cons, Word::TRUE),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );

    let instance = make_instance(&mut context, &runtime, Word::NIL, &[Word::NIL])
        .unwrap_or_else(|error| panic!("instance: {error:?}"));
    assert_eq!(slot_ref(&context, instance, 1), Err(ObjectError::TypeError));
    assert_eq!(
        slot_set(&mut context, instance, 1, Word::TRUE),
        Err(ObjectError::TypeError)
    );

    let readtable = make_readtable(&mut context, &runtime, Word::NIL, Word::NIL, Word::NIL)
        .unwrap_or_else(|error| panic!("readtable: {error:?}"));
    assert_eq!(
        readtable_slot(&context, readtable, 3),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
    let stream = make_stream(
        &mut context,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("stream: {error:?}"));
    assert_eq!(
        stream_slot(&context, stream, 5),
        Err(ObjectError::Storage(
            ncl_sys::StorageCondition::ThreadNotRegistered
        ))
    );
    assert_eq!(
        code_slot(&context, ncl_object::CodeObject::from_word(Word::TRUE), 0),
        Err(ObjectError::TypeError)
    );
}
