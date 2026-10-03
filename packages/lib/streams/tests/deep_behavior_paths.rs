#![allow(clippy::too_many_lines, missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_cons, make_simple_vector,
    make_stream, make_string,
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

fn symbol(runtime: &Runtime, ctx: &mut ThreadContext, package_name: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package_name).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn keyword(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    symbol(runtime, ctx, "KEYWORD", name)
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

fn path_word(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> (String, Word) {
    let path = std::env::temp_dir().join(format!("ncl-stream-deep-{name}-{}", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    (path.clone(), text(ctx, runtime, &path))
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_adapters_cover_bounds_eof_and_stream_kind_paths() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let close = builtin(&runtime, &mut ctx, "CLOSE");

    let value = text(&mut ctx, &runtime, "abcd");
    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");
    let middle = keyword(&runtime, &mut ctx, "MIDDLE");
    let common_start = symbol(&runtime, &mut ctx, "COMMON-LISP", "START");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[
                value,
                output,
                Word::fixnum(1),
                Word::fixnum(2),
                Word::fixnum(3)
            ],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, Word::fixnum(3), Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, common_start, Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, middle, Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, start, Word::fixnum(1), end, Word::fixnum(3)],
        ),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_line,
            &[value, output, Word::fixnum(0), Word::fixnum(1)]
        ),
        Ok(value)
    );
    let written = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    assert_eq!(ncl_object::string_length(&ctx, written), Ok(4));

    let line_source = text(&mut ctx, &runtime, "x\n");
    let line_input = runtime
        .call_builtin(&mut ctx, make_input, &[line_source])
        .unwrap();
    let line = runtime
        .call_builtin(&mut ctx, read_line, &[line_input])
        .unwrap();
    assert_eq!(ncl_object::string_ref(&ctx, line, 0), Ok('x'));
    assert_eq!(ctx.values()[1], Word::NIL);

    let data = data_stream(&runtime, &mut ctx, b"A");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('Z')), data]
        ),
        Ok(Word::character(u32::from('Z')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[data]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[data, Word::NIL, Word::fixnum(8)]),
        Ok(Word::fixnum(8))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[data]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[data]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[data]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), data]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let closed_output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    runtime
        .call_builtin(&mut ctx, close, &[closed_output])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[closed_output]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn file_adapters_cover_missing_policies_metadata_and_file_io_eof() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let open_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let if_missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let if_exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let error = keyword(&runtime, &mut ctx, "ERROR");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let overwrite = keyword(&runtime, &mut ctx, "OVERWRITE");
    let external_format = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let utf8 = keyword(&runtime, &mut ctx, "UTF-8");

    let (missing_path, missing_word) = path_word(&mut ctx, &runtime, "missing");
    let _ = fs::remove_file(&missing_path);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, output, if_missing, error],
        ),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, output, if_missing, nil],
        ),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, io, if_missing, error],
        ),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, io, if_missing, nil],
        ),
        Ok(Word::NIL)
    );
    let created_io = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, io, if_missing, create],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[created_io]),
        Ok(Word::fixnum(0))
    );
    runtime
        .call_builtin(&mut ctx, close, &[created_io])
        .unwrap();

    let (path, path_word) = path_word(&mut ctx, &runtime, "existing");
    fs::write(&path, b"abc").unwrap();
    let output_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, overwrite],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[output_stream]),
        Ok(Word::fixnum(3))
    );
    runtime
        .call_builtin(&mut ctx, write_byte, &[Word::fixnum(90), output_stream])
        .unwrap();
    runtime
        .call_builtin(&mut ctx, close, &[output_stream])
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"Zbc".to_vec());

    let io_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, io, external_format, utf8],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[io_stream]),
        Ok(Word::fixnum(3))
    );
    let one = text(&mut ctx, &runtime, "a");
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[io_stream, one]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[io_stream]),
        Ok(Word::character(u32::from('Z')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[io_stream]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'c')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[io_stream, Word::NIL, Word::fixnum(77)]
        ),
        Ok(Word::fixnum(77))
    );
    runtime.call_builtin(&mut ctx, close, &[io_stream]).unwrap();

    let data = data_stream(&runtime, &mut ctx, b"x");
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_p, &[data]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_p, &[data]),
        Ok(Word::NIL)
    );
    let closed = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, input])
        .unwrap();
    runtime.call_builtin(&mut ctx, close, &[closed]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[closed]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );
    fs::remove_file(path).unwrap();
    fs::remove_file(missing_path).unwrap();
}

#[test]
fn malformed_stream_state_reports_layout_errors_in_state_and_adapters() {
    let (runtime, mut ctx) = setup();
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let output_direction = symbol(&runtime, &mut ctx, "COMMON-LISP", "OUTPUT");

    let invalid_kind_state =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(999), Word::fixnum(0)]).unwrap();
    let invalid_kind_stream = make_stream(
        &mut ctx,
        &runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        invalid_kind_state,
        Word::NIL,
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[invalid_kind_stream.into()]),
        Err(ncl_object::ObjectError::Layout)
    );

    let bad_output = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let bad_output_state = make_simple_vector(
        &mut ctx,
        &runtime,
        &[Word::fixnum(-2), Word::fixnum(1), bad_output],
    )
    .unwrap();
    let bad_output_stream = make_stream(
        &mut ctx,
        &runtime,
        output_direction,
        Word::NIL,
        Word::NIL,
        bad_output_state,
        Word::NIL,
    )
    .unwrap();
    let bad_output_stream: Word = bad_output_stream.into();
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[bad_output_stream]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get_output, &[bad_output_stream]),
        Err(ncl_object::ObjectError::Layout)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_policies_cover_invalid_options_eof_and_close_paths() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let input_stream_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let if_missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let if_exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let invalid = keyword(&runtime, &mut ctx, "INVALID");
    let supersede = keyword(&runtime, &mut ctx, "SUPERSEDE");
    let new_version = keyword(&runtime, &mut ctx, "NEW-VERSION");
    let rename = keyword(&runtime, &mut ctx, "RENAME");
    let path = std::env::temp_dir().join(format!(
        "ncl-stream-deep-policy-cases-{}",
        std::process::id()
    ));
    let path_string = path.to_string_lossy().into_owned();
    let path_word = text(&mut ctx, &runtime, &path_string);
    let missing = std::env::temp_dir().join(format!(
        "ncl-stream-deep-policy-missing-{}",
        std::process::id()
    ));
    let missing_string = missing.to_string_lossy().into_owned();
    let missing_word = text(&mut ctx, &runtime, &missing_string);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&missing);

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, input, if_missing, invalid],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_word, direction, output, if_missing, invalid],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    fs::write(&path, b"xy").unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, if_exists, invalid],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    for policy in [supersede, new_version, rename] {
        fs::write(&path, b"xy").unwrap();
        let stream = runtime
            .call_builtin(
                &mut ctx,
                open,
                &[path_word, direction, output, if_exists, policy],
            )
            .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, file_position, &[stream, Word::fixnum(3)]),
            Err(ncl_object::ObjectError::TypeError)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, input_stream_p, &[stream]),
            Ok(Word::NIL)
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, file_length, &[stream]),
            Ok(Word::fixnum(0))
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, close, &[stream]),
            Ok(Word::TRUE)
        );
    }

    fs::write(&path, b"xy").unwrap();
    let default_input = runtime.call_builtin(&mut ctx, open, &[path_word]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[default_input]),
        Ok(Word::fixnum(2))
    );
    runtime
        .call_builtin(&mut ctx, close, &[default_input])
        .unwrap();

    let input_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, input])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[input_stream]),
        Ok(Word::fixnum(i64::from(b'x')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[input_stream]),
        Ok(Word::character(u32::from('y')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[input_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[input_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

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
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'x')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[io_stream]),
        Ok(Word::character(u32::from('y')))
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
        runtime.call_builtin(&mut ctx, finish_output, &[io_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), io_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );

    fs::remove_file(path).unwrap();
    let _ = fs::remove_file(missing);
}
