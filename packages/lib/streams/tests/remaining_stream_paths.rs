#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_simple_vector, make_stream,
    make_string, symbol_value,
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

fn string(ctx: &ThreadContext, value: Word) -> String {
    (0..ncl_object::string_length(ctx, value).unwrap())
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
}

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "KEYWORD").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn path_word(ctx: &mut ThreadContext, runtime: &Runtime, suffix: &str) -> (String, Word) {
    let path = std::env::temp_dir().join(format!(
        "ncl-streams-remaining-{suffix}-{}",
        std::process::id()
    ));
    let path = path.to_string_lossy().into_owned();
    (path.clone(), text(ctx, runtime, &path))
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
    let package = runtime.ensure_package(ctx, "COMMON-LISP").unwrap();
    let symbol = Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0;
    symbol_value(ctx, symbol).unwrap()
}

fn state_stream(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
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
#[allow(clippy::too_many_lines)]
fn character_input_remaining_peek_line_and_bound_results() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");

    let source = text(&mut ctx, &runtime, " \t\nalpha");
    let stream = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[Word::TRUE, stream, Word::NIL, Word::fixnum(70)],
        ),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::character(u32::from('a')), stream],),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unread, &[Word::character(u32::from('a')), stream],),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('a')))
    );
    let line = runtime
        .call_builtin(&mut ctx, read_line, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, line), "lpha");
    assert_eq!(ctx.values()[1], Word::TRUE);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[Word::NIL, stream, Word::NIL, Word::fixnum(71)],
        ),
        Ok(Word::fixnum(71))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream, Word::NIL, Word::fixnum(72)],),
        Ok(Word::fixnum(72))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_line, &[stream, Word::NIL, Word::fixnum(73)],),
        Ok(Word::fixnum(73))
    );
    assert_eq!(ctx.values()[1], Word::NIL);
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::fixnum(1), stream],),
        Err(ncl_object::ObjectError::TypeError)
    );

    let bounded_source = text(&mut ctx, &runtime, "012345");
    let bounded = runtime
        .call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, start, Word::fixnum(2), end, Word::fixnum(5)],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded]),
        Ok(Word::character(u32::from('2')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded]),
        Ok(Word::character(u32::from('3')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded]),
        Ok(Word::character(u32::from('4')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded, Word::NIL, Word::fixnum(74)],),
        Ok(Word::fixnum(74))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, start, Word::fixnum(5), end, Word::fixnum(2)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread,
            &[Word::character(u32::from('x')), bounded],
        ),
        Ok(Word::character(u32::from('x')))
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_output_remaining_line_state_and_range_results() {
    let (runtime, mut ctx) = setup();
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let terpri = builtin(&runtime, &mut ctx, "TERPRI");
    let fresh = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");
    let unknown = keyword(&runtime, &mut ctx, "UNKNOWN");

    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('A')), output]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, terpri, &[output]),
        Ok(Word::NIL)
    );
    let first_output = runtime.call_builtin(&mut ctx, get, &[output]).unwrap();
    assert_eq!(string(&ctx, first_output), "A\n\n");
    let value = text(&mut ctx, &runtime, "abcdef");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, start, Word::fixnum(1), end, Word::fixnum(4)],
        ),
        Ok(value)
    );
    let line_value = text(&mut ctx, &runtime, "uvwxyz");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_line,
            &[line_value, output, Word::fixnum(2), Word::fixnum(5)],
        ),
        Ok(line_value)
    );
    let second_output = runtime.call_builtin(&mut ctx, get, &[output]).unwrap();
    assert_eq!(string(&ctx, second_output), "bcdwxy\n");
    let short_value = text(&mut ctx, &runtime, "abc");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[short_value, output, unknown, Word::fixnum(0)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[
                short_value,
                output,
                Word::fixnum(0),
                Word::fixnum(1),
                Word::fixnum(2)
            ],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::fixnum(65), output],),
        Err(ncl_object::ObjectError::TypeError)
    );
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(-1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let input_value = text(&mut ctx, &runtime, "x");
    let input = runtime
        .call_builtin(&mut ctx, make_input, &[input_value])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, get, &[input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let invalid_input = text(&mut ctx, &runtime, "x");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            make_input,
            &[invalid_input, unknown, Word::fixnum(0)]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn data_stream_remaining_byte_character_and_position_results() {
    let (runtime, mut ctx) = setup();
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let stream = data_stream(&runtime, &mut ctx, b"A\n");

    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::NIL, stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream]),
        Ok(Word::fixnum(10))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[stream]),
        Ok(Word::fixnum(2))
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
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream, Word::NIL, Word::fixnum(81)]),
        Ok(Word::fixnum(81))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream, Word::NIL, Word::fixnum(82)]),
        Ok(Word::fixnum(82))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[Word::NIL, stream, Word::NIL, Word::fixnum(83)]
        ),
        Ok(Word::fixnum(83))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('z')), stream]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[stream]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[stream, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_remaining_policy_metadata_and_close_results() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let open_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let interactive_p = builtin(&runtime, &mut ctx, "INTERACTIVE-STREAM-P");
    let element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let external = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let overwrite = keyword(&runtime, &mut ctx, "OVERWRITE");
    let supersede = keyword(&runtime, &mut ctx, "SUPERSEDE");
    let rename = keyword(&runtime, &mut ctx, "RENAME");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let format = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let utf8 = keyword(&runtime, &mut ctx, "UTF-8");
    let (path, path_word) = path_word(&mut ctx, &runtime, "matrix");
    let _ = fs::remove_file(&path);

    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path_word, direction, input, missing, nil]),
        Ok(Word::NIL)
    );
    let created = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, input, missing, create],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[created]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[created, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_p, &[created]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_p, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[created]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, interactive_p, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, external, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, element_type, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[created]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[created]),
        Ok(Word::NIL)
    );

    fs::write(&path, b"abc").unwrap();
    let output_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, overwrite],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[output_stream]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(90), output_stream]),
        Ok(Word::fixnum(90))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[output_stream]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), b"Zbc".to_vec());

    fs::write(&path, b"old").unwrap();
    let superseded = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, supersede],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(65), superseded]),
        Ok(Word::fixnum(65))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[superseded]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), b"A".to_vec());

    let renamed = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, rename],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[renamed]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[renamed]),
        Ok(Word::TRUE)
    );

    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io, format, utf8])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_p, &[io_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_p, &[io_stream]),
        Ok(Word::TRUE)
    );
    let unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(&mut ctx, string_length, &[io_stream, unicode]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let _ = fs::remove_file(path);
}

#[test]
#[allow(clippy::too_many_lines)]
fn remaining_file_io_data_standard_and_layout_results() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let overwrite = keyword(&runtime, &mut ctx, "OVERWRITE");
    let (path, path_word) = path_word(&mut ctx, &runtime, "remaining-io");
    fs::write(&path, b"ab").unwrap();

    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::NIL, io_stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[io_stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'b')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[io_stream, Word::NIL, Word::fixnum(84)]
        ),
        Ok(Word::fixnum(84))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );

    let output_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, overwrite],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[output_stream, Word::fixnum(2)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write,
            &[Word::character(u32::from('Z')), output_stream],
        ),
        Ok(Word::character(u32::from('Z')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output_stream]),
        Ok(Word::TRUE)
    );

    let data = data_stream(&runtime, &mut ctx, b"_");
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('Q')), data],),
        Ok(Word::character(u32::from('Q')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[data]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[data]),
        Ok(Word::fixnum(1))
    );

    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let output_for_finish = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output_for_finish]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[output_for_finish]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let standard_input = standard_stream(&runtime, &mut ctx, "*STANDARD-INPUT*");
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::NIL, standard_input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[standard_input, Word::NIL, Word::fixnum(85)],
        ),
        Ok(Word::fixnum(85))
    );

    let invalid_kind = state_stream(&runtime, &mut ctx, &[Word::fixnum(-99), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[invalid_kind]),
        Err(ncl_object::ObjectError::Layout)
    );
    let invalid_tag = state_stream(&runtime, &mut ctx, &[Word::TRUE, Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[invalid_tag]),
        Err(ncl_object::ObjectError::Layout)
    );
    let _ = fs::remove_file(path);
}
