#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    make_simple_vector, make_stream, make_string, symbol_value, FunctionObject, Package, Runtime,
    ThreadContext, Word,
};
use std::fs;

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

#[test]
fn character_adapters_return_concrete_eof_and_string_results() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let terpri = builtin(&runtime, &mut ctx, "TERPRI");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let stream_element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let value = text(&mut ctx, &runtime, "abcd");
    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    let start = keyword(&runtime, &mut ctx, "START");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, start, Word::fixnum(1)],
        ),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_line,
            &[value, output, Word::fixnum(0), Word::fixnum(0)]
        ),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, terpri, &[output]),
        Ok(Word::NIL)
    );
    let written = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    let written_length = ncl_object::string_length(&ctx, written).unwrap();
    assert_eq!(written_length, 5);
    assert_eq!(ncl_object::string_ref(&ctx, written, 0), Ok('b'));
    assert_eq!(ncl_object::string_ref(&ctx, written, 1), Ok('c'));
    assert_eq!(ncl_object::string_ref(&ctx, written, 2), Ok('d'));
    assert_eq!(ncl_object::string_ref(&ctx, written, 3), Ok('\n'));
    assert_eq!(ncl_object::string_ref(&ctx, written, 4), Ok('\n'));
    assert_eq!(
        runtime.call_builtin(&mut ctx, stream_element_type, &[output]),
        Ok(Word::NIL)
    );
    let common_lisp = runtime.ensure_package(&mut ctx, "COMMON-LISP").unwrap();
    let standard_output = Package::from_word(common_lisp)
        .intern(&mut ctx, &runtime, "*STANDARD-OUTPUT*")
        .unwrap()
        .0;
    let standard_output = symbol_value(&ctx, standard_output).unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_byte,
            &[Word::fixnum(i64::from(b'!')), standard_output],
        ),
        Ok(Word::fixnum(i64::from(b'!')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[standard_output]),
        Ok(Word::NIL)
    );

    let empty = text(&mut ctx, &runtime, "");
    let empty_stream = runtime
        .call_builtin(&mut ctx, make_input, &[empty])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[Word::NIL, empty_stream, Word::NIL, Word::fixnum(41)],
        ),
        Ok(Word::fixnum(41))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::NIL, empty_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_char,
            &[empty_stream, Word::NIL, Word::fixnum(42)],
        ),
        Ok(Word::fixnum(42))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_line,
            &[empty_stream, Word::NIL, Word::fixnum(43)],
        ),
        Ok(Word::fixnum(43))
    );
    assert_eq!(ctx.values()[1], Word::NIL);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('x')), empty_stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get_output, &[empty_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, Word::TRUE, Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, Word::fixnum(0), Word::TRUE],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn file_and_data_adapters_return_positions_element_types_and_close_errors() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let stream_element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let stream_external_format = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let external_format = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let utf8 = keyword(&runtime, &mut ctx, "UTF-8");
    let path =
        std::env::temp_dir().join(format!("ncl-streams-coverage-edges-{}", std::process::id()));
    let path_string = path.to_string_lossy().into_owned();
    let path_word = text(&mut ctx, &runtime, &path_string);
    fs::write(&path, b"ab").unwrap();

    let input_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, input])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[input_stream]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[input_stream, Word::fixnum(2)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[input_stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, stream_element_type, &[input_stream]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_p, &[input_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_p, &[input_stream]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let io_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, io, external_format, utf8],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, stream_external_format, &[io_stream]),
        Ok(utf8)
    );
    let unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[io_stream, unicode],),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[io_stream],),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_char,
            &[io_stream, Word::NIL, Word::fixnum(53)],
        ),
        Ok(Word::fixnum(53))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[io_stream, Word::NIL, Word::fixnum(52)],
        ),
        Ok(Word::fixnum(52))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );

    let data = data_stream(&runtime, &mut ctx, b"q");
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[data]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[data]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[data]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, stream_element_type, &[data]),
        Ok(Word::NIL)
    );

    fs::remove_file(path).unwrap();
}

#[test]
fn short_string_input_and_data_output_expose_boundary_values() {
    let (runtime, mut ctx) = setup();
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let short_text = text(&mut ctx, &runtime, "h");
    let state = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(-1), Word::fixnum(0), short_text],
    )
    .unwrap();
    let stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        state,
        Word::NIL,
    )
    .unwrap()
    .into();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('h')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream, Word::NIL, Word::fixnum(61)]),
        Ok(Word::fixnum(61))
    );

    let data = data_stream(&runtime, &mut ctx, b"A");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('Z')), data],
        ),
        Ok(Word::character(u32::from('Z')))
    );
    let readable_data = data_stream(&runtime, &mut ctx, b"A");
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[readable_data]),
        Ok(Word::fixnum(i64::from(b'A')))
    );
}
