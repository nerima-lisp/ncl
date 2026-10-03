#![allow(missing_docs)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_cons, make_simple_vector,
    make_stream, make_string, simple_vector_ref, stream_state,
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

fn symbol(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    symbol(runtime, ctx, "KEYWORD", name)
}

fn data_stream(runtime: &Runtime, ctx: &mut ThreadContext, bytes: &[i64]) -> Word {
    let mut values = vec![Word::fixnum(0), Word::fixnum(0)];
    values.extend(bytes.iter().copied().map(Word::fixnum));
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

fn file_path(ctx: &mut ThreadContext, runtime: &Runtime, label: &str) -> (String, Word) {
    let path =
        std::env::temp_dir().join(format!("ncl-streams-wave-{}-{}", label, std::process::id()));
    let path = path.to_string_lossy().into_owned();
    (path.clone(), text(ctx, runtime, &path))
}

#[test]
fn character_input_modes_lines_and_data_errors_have_exact_results() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let output = symbol(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-OUTPUT*");
    let input = symbol(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-INPUT*");
    let output_stream = ncl_object::symbol_value(&ctx, output).unwrap();
    let input_stream = ncl_object::symbol_value(&ctx, input).unwrap();

    let whitespace = text(&mut ctx, &runtime, " \t\n\r\x0cX");
    let stream = runtime
        .call_builtin(&mut ctx, make_input, &[whitespace])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::TRUE, stream]),
        Ok(Word::character(u32::from('X')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[Word::character(u32::from('X')), stream]
        ),
        Ok(Word::character(u32::from('X')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('X')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[
                Word::character(u32::from('X')),
                stream,
                Word::NIL,
                Word::fixnum(91)
            ],
        ),
        Ok(Word::fixnum(91))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from(' ')), stream]
        ),
        Ok(Word::character(u32::from(' ')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('X')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[
                Word::character(u32::from('Z')),
                stream,
                Word::NIL,
                Word::fixnum(92)
            ],
        ),
        Ok(Word::fixnum(92))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::fixnum(1), stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let line_source = text(&mut ctx, &runtime, "first\nlast");
    let line_stream = runtime
        .call_builtin(&mut ctx, make_input, &[line_source])
        .unwrap();
    let first = runtime
        .call_builtin(&mut ctx, read_line, &[line_stream])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, first), Ok(5));
    assert_eq!(ctx.values()[1], Word::NIL);
    let last = runtime
        .call_builtin(&mut ctx, read_line, &[line_stream])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, last), Ok(4));
    assert_eq!(ctx.values()[1], Word::TRUE);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_line,
            &[line_stream, Word::NIL, Word::fixnum(93)]
        ),
        Ok(Word::fixnum(93))
    );
    assert_eq!(ctx.values()[1], Word::NIL);

    let bounded_source = text(&mut ctx, &runtime, "abcdef");
    let bounded = runtime
        .call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, Word::fixnum(2), Word::fixnum(4)],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[bounded]),
        Ok(Word::character(u32::from('c')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[bounded]),
        Ok(Word::character(u32::from('d')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[bounded, Word::NIL, Word::fixnum(94)]),
        Ok(Word::fixnum(94))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, make_input, &[bounded_source, Word::fixnum(-1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, Word::fixnum(4), Word::fixnum(2)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, Word::fixnum(0), Word::fixnum(9)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::NIL, output_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unread_char, &[Word::TRUE, input_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let invalid_character = data_stream(&runtime, &mut ctx, &[0x11_0000]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[invalid_character]),
        Err(ncl_object::ObjectError::Layout)
    );
    let non_fixnum = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(0), Word::fixnum(0), Word::TRUE],
    )
    .unwrap();
    let non_fixnum_stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        non_fixnum,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[non_fixnum_stream.into()]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream]),
        Ok(Word::TRUE)
    );
}

#[test]
fn character_output_adapters_preserve_results_and_reject_bad_bounds() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let terpri = builtin(&runtime, &mut ctx, "TERPRI");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let source = text(&mut ctx, &runtime, "source");
    let input = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap();
    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");
    let common_end = symbol(&runtime, &mut ctx, "COMMON-LISP", "END");
    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[source, output, start, Word::fixnum(1), end, Word::fixnum(4)],
        ),
        Ok(source)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_line,
            &[source, output, Word::fixnum(0), Word::fixnum(1)],
        ),
        Ok(source)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, terpri, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('!')), output]
        ),
        Ok(Word::character(u32::from('!')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[output]),
        Ok(Word::NIL)
    );
    let written = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, written), Ok(8));
    assert_eq!(ncl_object::string_ref(&ctx, written, 0), Ok('o'));
    assert_eq!(ncl_object::string_ref(&ctx, written, 7), Ok('\n'));
    let reset = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, reset), Ok(0));

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[source, output, common_end, Word::fixnum(1)]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[source, output, unknown, Word::fixnum(1)]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[
                source,
                output,
                Word::fixnum(1),
                Word::fixnum(1),
                Word::fixnum(2)
            ],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[source, output, Word::fixnum(4), Word::fixnum(2)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[Word::TRUE, output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(-1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(256), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::TRUE, output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('x')), input]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn file_adapters_cover_policies_positions_metadata_and_closed_states() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let open_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let interactive_p = builtin(&runtime, &mut ctx, "INTERACTIVE-STREAM-P");
    let element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let external_format = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let if_exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let if_missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let append = keyword(&runtime, &mut ctx, "APPEND");
    let overwrite = keyword(&runtime, &mut ctx, "OVERWRITE");
    let error = keyword(&runtime, &mut ctx, "ERROR");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let external = keyword(&runtime, &mut ctx, "UTF-8");
    let (path, path_word) = file_path(&mut ctx, &runtime, "policy");
    let (missing_path, missing_word) = file_path(&mut ctx, &runtime, "missing");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&missing_path);

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, input, if_missing, nil]
        ),
        Ok(Word::NIL)
    );
    let created = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, input, if_missing, create],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[created]),
        Ok(Word::fixnum(0))
    );
    runtime.call_builtin(&mut ctx, close, &[created]).unwrap();

    fs::write(&path, b"abc").unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, error],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, nil],
        ),
        Ok(Word::NIL)
    );

    let appended = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, append],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[appended]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_byte,
            &[Word::fixnum(i64::from(b'd')), appended],
        ),
        Ok(Word::fixnum(i64::from(b'd')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[appended, Word::fixnum(1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('é')), appended]
        ),
        Ok(Word::character(u32::from('é')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[appended]),
        Ok(Word::fixnum(6))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_p, &[appended]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_p, &[appended]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[appended]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, interactive_p, &[appended]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, element_type, &[appended]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, external_format, &[appended]),
        Ok(Word::NIL)
    );
    runtime.call_builtin(&mut ctx, close, &[appended]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[appended]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[appended]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), appended]),
        Err(ncl_object::ObjectError::TypeError)
    );

    fs::write(&path, b"xyz").unwrap();
    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[io_stream, Word::fixnum(3)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[io_stream, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[io_stream]),
        Ok(Word::character(u32::from('x')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'y')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[io_stream, Word::NIL, Word::fixnum(95)]
        ),
        Ok(Word::fixnum(i64::from(b'z')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[io_stream, Word::NIL, Word::fixnum(95)]
        ),
        Ok(Word::fixnum(95))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(65), io_stream]),
        Ok(Word::fixnum(65))
    );
    let unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[io_stream, unicode]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );

    let external_format_option = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let formatted = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[
                path_word,
                direction,
                output,
                if_exists,
                overwrite,
                external_format_option,
                external,
            ],
        )
        .unwrap();
    let formatted_unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            file_string_length,
            &[formatted, formatted_unicode]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    runtime.call_builtin(&mut ctx, close, &[formatted]).unwrap();

    fs::remove_file(path).unwrap();
    fs::remove_file(missing_path).unwrap();
}

#[test]
fn malformed_states_return_layout_or_type_errors_without_false_success() {
    let (runtime, mut ctx) = setup();
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let output_direction = symbol(&runtime, &mut ctx, "COMMON-LISP", "OUTPUT");

    let invalid_kind =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(777), Word::fixnum(0)]).unwrap();
    let invalid_stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        invalid_kind,
        Word::NIL,
    )
    .unwrap();
    let invalid_stream: Word = invalid_stream.into();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[invalid_stream]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[invalid_stream]),
        Err(ncl_object::ObjectError::Layout)
    );

    let bad_position = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(0), Word::TRUE, Word::fixnum(65)],
    )
    .unwrap();
    let bad_position_stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        bad_position,
        Word::NIL,
    )
    .unwrap();
    let bad_position_stream: Word = bad_position_stream.into();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[bad_position_stream]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[bad_position_stream]),
        Ok(Word::TRUE)
    );

    let bad_output_list = make_cons(&mut ctx, &runtime, Word::TRUE, Word::NIL).unwrap();
    let bad_output_state = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(-2), Word::fixnum(1), bad_output_list],
    )
    .unwrap();
    let bad_output = make_stream(
        &mut ctx,
        &runtime,
        output_direction,
        Word::NIL,
        Word::NIL,
        bad_output_state,
        Word::NIL,
    )
    .unwrap();
    let bad_output: Word = bad_output.into();
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[bad_output]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get_output, &[bad_output]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('q')), bad_output],
        ),
        Ok(Word::character(u32::from('q')))
    );

    let state = stream_state(&ctx, ncl_object::Stream::from_word(bad_output)).unwrap();
    assert_eq!(simple_vector_ref(&ctx, state, 0), Ok(Word::fixnum(-2)));

    let closed_state = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(-3), Word::fixnum(0), Word::NIL],
    )
    .unwrap();
    let closed = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        closed_state,
        Word::NIL,
    )
    .unwrap();
    let closed: Word = closed.into();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
