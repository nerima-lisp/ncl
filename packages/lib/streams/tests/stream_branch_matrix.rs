#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_simple_vector, make_stream,
    make_string,
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
fn character_input_branch_matrix_has_exact_values_and_eof_contracts() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_no_hang = builtin(&runtime, &mut ctx, "READ-CHAR-NO-HANG");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");

    let source = text(&mut ctx, &runtime, " \tA\nlast");
    let stream = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::NIL, stream]),
        Ok(Word::character(u32::from(' ')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::TRUE, stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::character(u32::from('A')), stream],),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unread, &[Word::character(u32::from(' ')), stream],),
        Ok(Word::character(u32::from(' ')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_no_hang, &[stream]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('\n')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('l')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream]),
        Ok(Word::character(u32::from('s')))
    );
    let tail = runtime
        .call_builtin(&mut ctx, read_line, &[stream])
        .unwrap();
    assert_eq!(string(&ctx, tail), "t");
    assert_eq!(ctx.values()[1], Word::TRUE);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_line, &[stream, Word::NIL, Word::fixnum(401)],),
        Ok(Word::fixnum(401))
    );
    assert_eq!(ctx.values()[1], Word::NIL);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[stream, Word::NIL, Word::fixnum(402)],),
        Ok(Word::fixnum(402))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[Word::NIL, stream, Word::NIL, Word::fixnum(403)],
        ),
        Ok(Word::fixnum(403))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_no_hang,
            &[stream, Word::NIL, Word::fixnum(404)]
        ),
        Ok(Word::fixnum(404))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[
                Word::character(u32::from('z')),
                stream,
                Word::NIL,
                Word::fixnum(405)
            ],
        ),
        Ok(Word::fixnum(405))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::fixnum(1), stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let bounded_source = text(&mut ctx, &runtime, "012345");
    let bounded = runtime
        .call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, start, Word::fixnum(1), end, Word::fixnum(4)],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded]),
        Ok(Word::character(u32::from('1')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded]),
        Ok(Word::character(u32::from('2')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded]),
        Ok(Word::character(u32::from('3')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded, Word::NIL, Word::fixnum(406)],),
        Ok(Word::fixnum(406))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[bounded]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            make_input,
            &[bounded_source, start, Word::fixnum(5), end, Word::fixnum(2)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_output_and_data_branch_matrix_preserves_results() {
    let (runtime, mut ctx) = setup();
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let terpri = builtin(&runtime, &mut ctx, "TERPRI");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");

    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('A')), output]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, terpri, &[output]),
        Ok(Word::NIL)
    );
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
    let result = runtime.call_builtin(&mut ctx, get, &[output]).unwrap();
    assert_eq!(string(&ctx, result), "A\n\nbcdwxy\n");
    let reset = runtime.call_builtin(&mut ctx, get, &[output]).unwrap();
    assert_eq!(string(&ctx, reset), "");
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(-1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(256), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::fixnum(65), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, start, Word::fixnum(3), end, Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    let data = data_stream(&runtime, &mut ctx, b"AB");
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[data]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::NIL, data]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[data]),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unread, &[Word::character(u32::from('A')), data],),
        Ok(Word::character(u32::from('A')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[data]),
        Ok(Word::fixnum(65))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[data]),
        Ok(Word::fixnum(66))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[data, Word::NIL, Word::fixnum(407)]),
        Ok(Word::fixnum(407))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[data, Word::NIL, Word::fixnum(408)]),
        Ok(Word::fixnum(408))
    );
    let writable_data = data_stream(&runtime, &mut ctx, b"_");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write,
            &[Word::character(u32::from('Q')), writable_data],
        ),
        Ok(Word::character(u32::from('Q')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[writable_data]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), writable_data]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[writable_data]),
        Ok(Word::NIL)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_adapter_branch_matrix_reports_policy_metadata_and_eof() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let stream_p = builtin(&runtime, &mut ctx, "STREAMP");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let open_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let interactive_p = builtin(&runtime, &mut ctx, "INTERACTIVE-STREAM-P");
    let external = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let error = keyword(&runtime, &mut ctx, "ERROR");
    let append = keyword(&runtime, &mut ctx, "APPEND");
    let overwrite = keyword(&runtime, &mut ctx, "OVERWRITE");
    let invalid = keyword(&runtime, &mut ctx, "INVALID");
    let format = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let utf8 = keyword(&runtime, &mut ctx, "UTF-8");
    let path =
        std::env::temp_dir().join(format!("ncl-stream-branch-matrix-{}", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    let path_word = text(&mut ctx, &runtime, &path);
    let _ = fs::remove_file(&path);

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, input, missing, error],
        ),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path_word, direction, input, missing, nil],),
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
        runtime.call_builtin(&mut ctx, stream_p, &[created]),
        Ok(Word::TRUE)
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
        runtime.call_builtin(&mut ctx, length, &[created]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[created]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, external, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, interactive_p, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[created, Word::NIL, Word::fixnum(409)]
        ),
        Ok(Word::fixnum(409))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[created]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[created]),
        Err(ncl_object::ObjectError::TypeError)
    );

    fs::write(&path, b"abc").unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, error],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path_word, direction, output, exists, nil],),
        Ok(Word::NIL)
    );
    let appended = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, append],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[appended]),
        Ok(Word::fixnum(3))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(90), appended]),
        Ok(Word::fixnum(90))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write,
            &[Word::character(u32::from('!')), appended]
        ),
        Ok(Word::character(u32::from('!')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[appended]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), b"abcZ!".to_vec());
    let overwritten = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, overwrite],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[overwritten]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[overwritten]),
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
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[io_stream]),
        Ok(Word::character(u32::from('a')))
    );
    let unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(&mut ctx, string_length, &[io_stream, unicode]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path_word, direction, invalid],),
        Err(ncl_object::ObjectError::TypeError)
    );
    let _ = fs::remove_file(path);
}

#[test]
fn invalid_state_and_option_matrix_returns_layout_or_type_errors() {
    let (runtime, mut ctx) = setup();
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let invalid_tag = state_stream(&runtime, &mut ctx, &[Word::fixnum(-99), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[invalid_tag]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[invalid_tag]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[invalid_tag]),
        Err(ncl_object::ObjectError::Layout)
    );
    let non_numeric = state_stream(&runtime, &mut ctx, &[Word::TRUE, Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[non_numeric]),
        Err(ncl_object::ObjectError::Layout)
    );
    let closed = state_stream(&runtime, &mut ctx, &[Word::fixnum(-3), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('x')), closed],),
        Err(ncl_object::ObjectError::TypeError)
    );
}
