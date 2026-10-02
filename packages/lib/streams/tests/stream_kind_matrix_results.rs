#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_simple_vector, make_stream,
    symbol_value,
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

fn symbol(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn data_stream(runtime: &Runtime, ctx: &mut ThreadContext, bytes: &[u8]) -> Word {
    let mut values = vec![Word::fixnum(0), Word::fixnum(0)];
    values.extend(bytes.iter().map(|byte| Word::fixnum(i64::from(*byte))));
    let state = make_simple_vector(ctx, runtime, &values).unwrap();
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

fn standard_stream(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let stream_symbol = symbol(runtime, ctx, name);
    symbol_value(ctx, stream_symbol).unwrap()
}

#[test]
fn data_stream_operations_return_bytes_positions_and_eof_values() {
    let (runtime, mut ctx) = setup();
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let stream = data_stream(&runtime, &mut ctx, b"AB");
    let character_string = ncl_object::make_string(&mut ctx, &runtime, &['é']).unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::NIL, stream]),
        Ok(Word::character(u32::from('B')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('A')), stream],
        ),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream]),
        Ok(Word::fixnum(i64::from(b'A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream, Word::fixnum(2)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream, Word::NIL, Word::fixnum(91)],),
        Ok(Word::fixnum(i64::from(b'B')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream, Word::NIL, Word::fixnum(91)],),
        Ok(Word::fixnum(91))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream, Word::fixnum(3)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('x')), stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[stream]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[stream, character_string],),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get_output, &[stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn standard_stream_operations_report_directions_metadata_and_wrong_kinds() {
    let (runtime, mut ctx) = setup();
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let streamp = builtin(&runtime, &mut ctx, "STREAMP");
    let input_stream_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_stream_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let open_stream_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let interactive_stream_p = builtin(&runtime, &mut ctx, "INTERACTIVE-STREAM-P");
    let element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let external_format = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let output = standard_stream(&runtime, &mut ctx, "*STANDARD-OUTPUT*");
    let input = standard_stream(&runtime, &mut ctx, "*STANDARD-INPUT*");
    let error = standard_stream(&runtime, &mut ctx, "*ERROR-OUTPUT*");
    let terminal = standard_stream(&runtime, &mut ctx, "*TERMINAL-IO*");
    let character = symbol(&runtime, &mut ctx, "CHARACTER");

    assert_eq!(
        runtime.call_builtin(&mut ctx, streamp, &[Word::NIL]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, streamp, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_stream_p, &[input]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_stream_p, &[input]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_stream_p, &[terminal]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_stream_p, &[terminal]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_stream_p, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, interactive_stream_p, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, element_type, &[output]),
        Ok(character)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, external_format, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('e')), error],
        ),
        Ok(Word::character(u32::from('e')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(33), error]),
        Ok(Word::fixnum(33))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[error]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[input]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('x')), input],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::NIL, output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('x')), input],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
}
