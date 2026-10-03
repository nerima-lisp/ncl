#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, make_string};

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

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn stream_with_input(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    make_input: FunctionObject,
) -> Word {
    let source = text(ctx, runtime, "ab");
    runtime.call_builtin(ctx, make_input, &[source]).unwrap()
}

#[test]
fn character_input_reports_eof_values_and_rejects_invalid_modes() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let stream = stream_with_input(&runtime, &mut ctx, make_input);

    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream, Word::NIL, Word::fixnum(73)]),
        Ok(Word::fixnum(73))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[Word::NIL, stream, Word::NIL, Word::fixnum(74)]
        ),
        Ok(Word::fixnum(74))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_line, &[stream, Word::NIL, Word::fixnum(75)]),
        Ok(Word::fixnum(75))
    );
    assert_eq!(ctx.values()[1], Word::NIL);

    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_line, &[stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream, Word::NIL, Word::fixnum(76)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::fixnum(1), stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let fresh_stream = stream_with_input(&runtime, &mut ctx, make_input);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('x')), fresh_stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn character_output_preserves_written_results_across_rejected_operations() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let input = stream_with_input(&runtime, &mut ctx, make_input);
    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('x')), output]
        ),
        Ok(Word::character(u32::from('x')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[Word::fixnum(1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::TRUE, output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(-1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('y')), input]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[input]),
        Ok(Word::NIL)
    );

    let value = text(&mut ctx, &runtime, "z");
    let unknown = keyword(&runtime, &mut ctx, "MIDDLE");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, unknown, Word::fixnum(0)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    let written = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, written), Ok(1));
    assert_eq!(ncl_object::string_ref(&ctx, written, 0), Ok('x'));

    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
