#![allow(clippy::too_many_lines, missing_docs)]
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

fn common_symbol(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
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
    let symbol = common_symbol(runtime, ctx, name);
    symbol_value(ctx, symbol).unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_adapter_options_return_exact_values_and_errors() {
    let (runtime, mut ctx) = setup();
    let input_constructor = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let output_constructor = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");
    let unknown = keyword(&runtime, &mut ctx, "NOT-A-BOUND");
    let common_start = common_symbol(&runtime, &mut ctx, "START");
    let value = text(&mut ctx, &runtime, "abcd");

    let output = runtime
        .call_builtin(&mut ctx, output_constructor, &[])
        .unwrap();
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
            &[value, output, Word::fixnum(2), Word::fixnum(4)],
        ),
        Ok(value)
    );
    let output_value = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    assert_eq!(string(&ctx, output_value), "bccd\n");

    let output = runtime
        .call_builtin(&mut ctx, output_constructor, &[])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, Word::fixnum(3), Word::TRUE],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
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
        runtime.call_builtin(&mut ctx, write_string, &[value, output, start],),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[value, output, unknown, Word::fixnum(1)],
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
            write_line,
            &[value, output, Word::fixnum(-1), Word::fixnum(2)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_line,
            &[value, output, Word::fixnum(3), Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    let input = runtime
        .call_builtin(
            &mut ctx,
            input_constructor,
            &[value, start, Word::fixnum(1), end, Word::fixnum(3)],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[input]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[input]),
        Ok(Word::character(u32::from('c')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[input, Word::NIL, Word::fixnum(701)]),
        Ok(Word::fixnum(701))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            input_constructor,
            &[value, start, Word::fixnum(3), end, Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_constructor, &[value, start, Word::TRUE],),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            input_constructor,
            &[value, unknown, Word::fixnum(1)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    let output = runtime
        .call_builtin(&mut ctx, output_constructor, &[])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_line, &[value, output]),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn standard_stream_defaults_and_wrong_directions_are_exact() {
    let (runtime, mut ctx) = setup();
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_no_hang = builtin(&runtime, &mut ctx, "READ-CHAR-NO-HANG");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let terpri = builtin(&runtime, &mut ctx, "TERPRI");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let external = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let input = standard_stream(&runtime, &mut ctx, "*STANDARD-INPUT*");
    let output = standard_stream(&runtime, &mut ctx, "*STANDARD-OUTPUT*");
    let error = standard_stream(&runtime, &mut ctx, "*ERROR-OUTPUT*");
    let terminal = standard_stream(&runtime, &mut ctx, "*TERMINAL-IO*");
    let character = common_symbol(&runtime, &mut ctx, "CHARACTER");

    assert_eq!(
        runtime.call_builtin(&mut ctx, element_type, &[input]),
        Ok(character)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, external, &[input]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[input]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('x')), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(7), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek, &[Word::NIL, input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unread, &[Word::character(u32::from('x')), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[input, Word::NIL, Word::fixnum(702)]),
        Ok(Word::fixnum(702))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_no_hang,
            &[input, Word::NIL, Word::fixnum(703)]
        ),
        Ok(Word::fixnum(703))
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[output]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[error]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[terminal]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, terpri, &[terminal]),
        Ok(Word::NIL)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_adapter_options_and_io_results_are_exact() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let external = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let error = keyword(&runtime, &mut ctx, "ERROR");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let append = keyword(&runtime, &mut ctx, "APPEND");
    let invalid = keyword(&runtime, &mut ctx, "NO-SUCH-POLICY");
    let utf8 = keyword(&runtime, &mut ctx, "UTF-8");
    let external_format = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let path =
        std::env::temp_dir().join(format!("ncl-stream-final-contracts-{}", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    let path_word = text(&mut ctx, &runtime, &path);
    let _ = fs::remove_file(&path);

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, input, missing, invalid],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, missing, error],
        ),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path_word, direction, io, missing, nil],),
        Ok(Word::NIL)
    );
    let created = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io, missing, create])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[created]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(65), created]),
        Ok(Word::fixnum(65))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('B')), created]),
        Ok(Word::character(u32::from('B')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[created]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[created]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), b"AB".to_vec());

    fs::write(&path, b"xyz").unwrap();
    let input_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, input])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[input_stream]),
        Ok(Word::character(u32::from('x')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[input_stream]),
        Ok(Word::fixnum(i64::from(b'y')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, string_length, &[created, path_word]),
        Ok(Word::fixnum(i64::try_from(path.len()).unwrap()))
    );

    let output_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, append],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(90), output_stream]),
        Ok(Word::fixnum(90))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[output_stream]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path).unwrap(), b"xyzZ".to_vec());

    let formatted = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, io, external_format, utf8],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, external, &[formatted]),
        Ok(utf8)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, string_length, &[formatted, path_word]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[formatted]),
        Ok(Word::TRUE)
    );
    let _ = fs::remove_file(path);
}

#[test]
fn registered_builtins_reject_missing_and_extra_arguments_concretely() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let streamp = builtin(&runtime, &mut ctx, "STREAMP");
    let input_p = builtin(&runtime, &mut ctx, "INPUT-STREAM-P");
    let output_p = builtin(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let open_p = builtin(&runtime, &mut ctx, "OPEN-STREAM-P");
    let interactive_p = builtin(&runtime, &mut ctx, "INTERACTIVE-STREAM-P");
    let element_type = builtin(&runtime, &mut ctx, "STREAM-ELEMENT-TYPE");
    let external = builtin(&runtime, &mut ctx, "STREAM-EXTERNAL-FORMAT");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let value = text(&mut ctx, &runtime, "x");
    let data = data_stream(&runtime, &mut ctx, b"x");

    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[data]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, streamp, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, input_p, &[Word::NIL, Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_p, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open_p, &[Word::NIL, Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, interactive_p, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, element_type, &[Word::NIL, Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, external, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_string, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_line, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, make_input, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, make_output, &[Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get_output, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write,
            &[Word::character(u32::from('x')), data, Word::NIL]
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), data, Word::NIL]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_string, &[value, data, Word::fixnum(0)]),
        Ok(value)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_line, &[value, data, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
