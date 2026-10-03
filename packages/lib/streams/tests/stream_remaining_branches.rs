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

fn string(ctx: &ThreadContext, value: Word) -> String {
    (0..ncl_object::string_length(ctx, value).unwrap())
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
}

#[test]
#[allow(clippy::too_many_lines)]
fn character_remaining_file_and_data_results_are_exact() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let close = builtin(&runtime, &mut ctx, "CLOSE");

    let empty = text(&mut ctx, &runtime, "");
    let input = runtime
        .call_builtin(&mut ctx, make_input, &[empty])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[input, Word::NIL, Word::fixnum(501)]),
        Ok(Word::fixnum(501))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek,
            &[Word::NIL, input, Word::NIL, Word::fixnum(502)]
        ),
        Ok(Word::fixnum(502))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('x')), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[input]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('Q')), output]),
        Ok(Word::character(u32::from('Q')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(65), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[output]),
        Ok(Word::NIL)
    );
    let value = runtime
        .call_builtin(&mut ctx, get_output, &[output])
        .unwrap();
    assert_eq!(string(&ctx, value), "Q");
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let data = data_stream(&runtime, &mut ctx, b"xy");
    assert_eq!(
        runtime.call_builtin(&mut ctx, write, &[Word::character(u32::from('Z')), data]),
        Ok(Word::character(u32::from('Z')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[data]),
        Ok(Word::character(u32::from('y')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[data, Word::NIL, Word::fixnum(504)]),
        Ok(Word::fixnum(504))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[data]),
        Ok(Word::NIL)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_remaining_position_io_and_policy_results_are_exact() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let append = keyword(&runtime, &mut ctx, "APPEND");
    let missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let format = keyword(&runtime, &mut ctx, "EXTERNAL-FORMAT");
    let utf8 = keyword(&runtime, &mut ctx, "UTF-8");
    let path = std::env::temp_dir().join(format!(
        "ncl-stream-remaining-branches-{}",
        std::process::id()
    ));
    let path = path.to_string_lossy().into_owned();
    let path_word = text(&mut ctx, &runtime, &path);
    let _ = fs::remove_file(&path);

    let input_missing =
        runtime.call_builtin(&mut ctx, open, &[path_word, direction, input, missing, nil]);
    assert_eq!(input_missing, Ok(Word::NIL));
    let input_created = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, input, missing, create],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[input_created]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[input_created, Word::fixnum(0)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[input_created, Word::fixnum(1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_created]),
        Ok(Word::TRUE)
    );

    fs::write(&path, b"ab").unwrap();
    let output_stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, append],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[output_stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[output_stream, Word::fixnum(1)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write,
            &[Word::character(u32::from('!')), output_stream]
        ),
        Ok(Word::character(u32::from('!')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(63), output_stream]),
        Ok(Word::fixnum(63))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[output_stream]),
        Ok(Word::NIL)
    );
    assert_eq!(fs::read(&path).unwrap(), b"ab!?".to_vec());
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[output_stream]),
        Ok(Word::TRUE)
    );

    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io, format, utf8])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[io_stream]),
        Ok(Word::fixnum(4))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[io_stream]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, position, &[io_stream, Word::fixnum(4)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[io_stream, Word::NIL, Word::fixnum(503)]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(90), io_stream]),
        Ok(Word::fixnum(90))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write,
            &[Word::character(u32::from('K')), io_stream]
        ),
        Ok(Word::character(u32::from('K')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, string_length, &[io_stream, path_word]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn remaining_stream_state_and_option_errors_are_concrete() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let finish = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let one_character = text(&mut ctx, &runtime, "r");
    let input = runtime
        .call_builtin(&mut ctx, make_input, &[one_character])
        .unwrap();
    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();

    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[input, one_character]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[input]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[input]),
        Err(ncl_object::ObjectError::TypeError)
    );
}
