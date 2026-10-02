#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, make_string};
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

fn string(ctx: &ThreadContext, value: Word) -> String {
    (0..ncl_object::string_length(ctx, value).unwrap())
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
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

#[test]
fn character_edges_preserve_positions_and_report_eof() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let source = text(&mut ctx, &runtime, " \tA\nlast");
    let expected_last = text(&mut ctx, &runtime, "last");
    let stream = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::TRUE, stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('\n')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread,
            &[Word::character(u32::from('\n')), stream]
        ),
        Ok(Word::character(u32::from('\n')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('\n')))
    );
    let read_last = runtime
        .call_builtin(&mut ctx, read_line, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, read_last), string(&ctx, expected_last));
    assert_eq!(ctx.values()[1], Word::TRUE);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream, Word::NIL, Word::fixnum(99)]),
        Ok(Word::fixnum(99))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[Word::NIL, stream, Word::NIL, Word::fixnum(88)]
        ),
        Ok(Word::fixnum(88))
    );
}

#[test]
fn character_output_options_and_side_effects_are_observable() {
    let (runtime, mut ctx) = setup();
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let terpri = builtin(&runtime, &mut ctx, "TERPRI");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let stream = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    let value = text(&mut ctx, &runtime, "abcdef");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, stream, start, Word::fixnum(2), end, Word::fixnum(4)],
        ),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, terpri, &[stream]),
        Ok(Word::NIL)
    );
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, output), "cd\n");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, stream, start, Word::fixnum(4), end, Word::fixnum(2)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    let output_after_error = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, output_after_error), "");
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_policies_io_and_metadata_match_observable_contents() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let stream_p = builtin(&runtime, &mut ctx, "STREAMP");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let external_format = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let if_exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let append = keyword(&runtime, &mut ctx, "APPEND");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let path = std::env::temp_dir().join(format!("ncl-streams-coverage-{}", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    let path_word = text(&mut ctx, &runtime, &path);
    fs::write(&path, b"ab").unwrap();

    let input_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, input])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, stream_p, &[input_stream]),
        Ok(Word::TRUE)
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
        runtime.call_builtin(&mut ctx, file_length, &[input_stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[input_stream]),
        Ok(Word::fixnum(97))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[input_stream]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[input_stream, Word::fixnum(2)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_stream]),
        Ok(Word::TRUE)
    );

    let appended = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, append],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('é')), appended]
        ),
        Ok(Word::character(u32::from('é')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(33), appended]),
        Ok(Word::fixnum(33))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[appended]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), vec![b'a', b'b', 0xc3, 0xa9, b'!']);

    let nil_result = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, nil],
        )
        .unwrap();
    assert_eq!(nil_result, Word::NIL);

    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, external_format, &[io_stream]),
        Ok(Word::NIL)
    );
    let unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[io_stream, unicode]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[io_stream, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(97))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), vec![b'a', b'b', 0xc3, 0xa9, b'!']);
    fs::remove_file(path).unwrap();
}

#[test]
fn character_peek_modes_and_eof_policies_cover_non_default_paths() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");

    let source = text(&mut ctx, &runtime, " a");
    let stream = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[
                Word::character(u32::from('a')),
                stream,
                Word::NIL,
                Word::fixnum(77),
            ],
        ),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_line, &[stream, Word::NIL, Word::fixnum(91)],),
        Ok(Word::fixnum(91))
    );
    assert_eq!(ctx.values()[1], Word::NIL);

    let empty = text(&mut ctx, &runtime, "");
    let empty_stream = runtime
        .call_builtin(&mut ctx, make_input, &[empty])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread,
            &[Word::character(u32::from('x')), empty_stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('x')), empty_stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), empty_stream],),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn file_input_character_reads_update_position_and_honor_eof_value() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_char_no_hang = builtin(&runtime, &mut ctx, "READ-CHAR-NO-HANG");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let path = std::env::temp_dir().join(format!(
        "ncl-streams-character-input-{}",
        std::process::id()
    ));
    fs::write(&path, b"AZ").unwrap();
    let path_word = text(&mut ctx, &runtime, &path.to_string_lossy());
    let stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, input])
        .unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char_no_hang, &[stream]),
        Ok(Word::character(u32::from('Z')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream, Word::NIL, Word::fixnum(77)],),
        Ok(Word::fixnum(77))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream]),
        Ok(Word::TRUE)
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn file_io_writes_and_reads_bytes_after_position_reset() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let io = keyword(&runtime, &mut ctx, "IO");
    let path = std::env::temp_dir().join(format!("ncl-streams-file-io-{}", std::process::id()));
    fs::write(&path, b"_").unwrap();
    let path_word = text(&mut ctx, &runtime, &path.to_string_lossy());
    let stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io])
        .unwrap();

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('é')), stream],
        ),
        Ok(Word::character(u32::from('é')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(33), stream]),
        Ok(Word::fixnum(33))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[stream]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[stream, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream]),
        Ok(Word::TRUE)
    );
    let reopened = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[reopened]),
        Ok(Word::fixnum(0xc3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[reopened]),
        Ok(Word::fixnum(0xa9))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[reopened]),
        Ok(Word::fixnum(33))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[reopened]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), vec![0xc3, 0xa9, 33]);
    fs::remove_file(path).unwrap();
}

#[test]
fn open_policies_report_errors_and_create_missing_input_only_when_requested() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let if_exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let if_does_not_exist = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let error = keyword(&runtime, &mut ctx, "ERROR");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let existing = std::env::temp_dir().join(format!(
        "ncl-streams-policy-existing-{}",
        std::process::id()
    ));
    let missing =
        std::env::temp_dir().join(format!("ncl-streams-policy-missing-{}", std::process::id()));
    fs::write(&existing, b"keep").unwrap();
    let existing_word = text(&mut ctx, &runtime, &existing.to_string_lossy());
    let missing_word = text(&mut ctx, &runtime, &missing.to_string_lossy());

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[existing_word, direction, output, if_exists, error],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(fs::read(&existing).unwrap(), b"keep");
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[missing_word, direction, input],),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, input, if_does_not_exist, nil,],
        ),
        Ok(Word::NIL)
    );
    let created = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, input, if_does_not_exist, create],
        )
        .unwrap();
    assert_eq!(fs::read(&missing).unwrap(), b"");
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[created]),
        Ok(Word::TRUE)
    );
    fs::remove_file(existing).unwrap();
    fs::remove_file(missing).unwrap();
}
