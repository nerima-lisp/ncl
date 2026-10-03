#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, make_simple_vector, make_stream, make_string,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn text(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).unwrap()
}

fn stream_with_state(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    let state = make_simple_vector(ctx, runtime, values).unwrap();
    make_stream(
        ctx,
        runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        state,
        Word::NIL,
    )
    .unwrap()
    .into()
}

#[test]
fn file_position_recognizes_every_state_code_without_reading_standard_input() {
    let (runtime, mut ctx) = setup();
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");

    for code in [0, -1, -2, -4, -5, -6, -7, -8, -9] {
        let stream = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(code), Word::fixnum(0)]);
        assert_eq!(
            runtime.call_builtin(&mut ctx, file_position, &[stream]),
            Ok(Word::fixnum(0)),
            "state code {code} should be recognized",
        );
    }

    let closed = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(-3), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn invalid_state_codes_return_layout_errors() {
    let (runtime, mut ctx) = setup();
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");

    let invalid = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(-10), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[invalid]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[invalid]),
        Err(ncl_object::ObjectError::Layout)
    );

    let non_numeric = stream_with_state(&runtime, &mut ctx, &[Word::TRUE, Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[non_numeric]),
        Err(ncl_object::ObjectError::Layout)
    );
}

#[test]
fn character_adapters_return_explicit_eof_values_and_wrong_kind_errors() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let value = text(&mut ctx, &runtime, "");
    let character = Word::character(u32::from('x'));

    let input = runtime
        .call_builtin(&mut ctx, make_input, &[value])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[input, Word::NIL, Word::fixnum(41)]),
        Ok(Word::fixnum(41))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[Word::NIL, input, Word::NIL, Word::fixnum(42)]
        ),
        Ok(Word::fixnum(42))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[character, input]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn registration_metadata_is_complete_and_unknown_stream_names_fail() {
    let (runtime, mut ctx) = setup();
    for name in ncl_lib_streams::registration::REGISTERED_BUILTINS {
        assert!(
            runtime.function(&mut ctx, "COMMON-LISP", name).is_some(),
            "registered builtin {name} should be findable"
        );
    }
    assert!(
        runtime
            .function(&mut ctx, "COMMON-LISP", "NOT-A-STREAM-BUILTIN")
            .is_none()
    );

    let streamp = builtin(&runtime, &mut ctx, "STREAMP");
    assert_eq!(
        runtime.call_builtin(&mut ctx, streamp, &[Word::NIL]),
        Ok(Word::NIL)
    );
}
