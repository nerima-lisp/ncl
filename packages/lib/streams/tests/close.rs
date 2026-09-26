#![allow(missing_docs)]

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, make_simple_vector, make_stream, pop_root,
    push_root, simple_vector_ref, stream_state,
};

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    let word = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing builtin {name}"));
    FunctionObject::try_from(word)
        .unwrap_or_else(|error| panic!("invalid builtin {name}: {error:?}"))
}

#[test]
fn close_marks_stream_closed_under_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_lib_streams::register(&runtime).unwrap_or_else(|error| panic!("streams: {error:?}"));
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    let state = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(0), Word::fixnum(0), Word::fixnum(65)],
    )
    .unwrap_or_else(|error| panic!("state: {error:?}"));
    let mut stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        state,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("stream: {error:?}"))
    .into();
    let stream_token = push_root(&mut ctx, &mut stream);

    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream]),
        Ok(Word::TRUE)
    );
    let closed_state = stream_state(&ctx, ncl_object::Stream::from_word(stream))
        .unwrap_or_else(|error| panic!("closed state: {error:?}"));
    assert_eq!(
        simple_vector_ref(&ctx, closed_state, 0),
        Ok(Word::fixnum(-3))
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert!(pop_root(&mut ctx, stream_token));
}

#[test]
fn binary_and_state_predicates_run_under_gc_stress_and_strict_forwarding() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    ncl_lib_streams::register(&runtime).unwrap_or_else(|error| panic!("streams: {error:?}"));
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let open_stream_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let interactive_stream_p = builtin(&runtime, &mut ctx, "INTERACTIVE-STREAM-P");
    let state = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(0), Word::fixnum(0), Word::fixnum(65)],
    )
    .unwrap_or_else(|error| panic!("state: {error:?}"));
    let mut stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        state,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("stream: {error:?}"))
    .into();
    let stream_token = push_root(&mut ctx, &mut stream);
    ctx.set_gc_stress(true);
    ctx.set_strict_forwarding(true);

    assert_eq!(
        runtime.call_builtin(&mut ctx, open_stream_p, &[stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, interactive_stream_p, &[stream]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream]),
        Ok(Word::fixnum(65))
    );
    assert!(pop_root(&mut ctx, stream_token));
}
