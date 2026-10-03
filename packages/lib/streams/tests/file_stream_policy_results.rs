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

fn path_word(ctx: &mut ThreadContext, runtime: &Runtime, suffix: &str) -> (String, Word) {
    let path = std::env::temp_dir().join(format!("ncl-stream-{suffix}-{}", std::process::id()));
    let path = path.to_string_lossy().into_owned();
    let word = text(ctx, runtime, &path);
    (path, word)
}

#[test]
fn open_reports_missing_and_existing_file_policy_results() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let output = keyword(&runtime, &mut ctx, "OUTPUT");
    let nonsense = keyword(&runtime, &mut ctx, "NONSENSE");
    let if_missing = keyword(&runtime, &mut ctx, "IF-DOES-NOT-EXIST");
    let if_exists = keyword(&runtime, &mut ctx, "IF-EXISTS");
    let nil = keyword(&runtime, &mut ctx, "NIL");
    let create = keyword(&runtime, &mut ctx, "CREATE");
    let error = keyword(&runtime, &mut ctx, "ERROR");
    let append = keyword(&runtime, &mut ctx, "APPEND");
    let path = path_word(&mut ctx, &runtime, "policy-missing");

    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path.1, direction, input, if_missing, error],
        ),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path.1, direction, input, if_missing, nil],),
        Ok(Word::NIL)
    );
    let created = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path.1, direction, input, if_missing, create],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[created]),
        Ok(Word::fixnum(0))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[created]),
        Ok(Word::TRUE)
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path.1, direction, nonsense]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path.1, direction]),
        Err(ncl_object::ObjectError::TypeError)
    );

    fs::write(&path.0, b"ab").unwrap();
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path.1, direction, output, if_exists, error],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, open, &[path.1, direction, output, if_exists, nil],),
        Ok(Word::NIL)
    );
    let appended = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path.1, direction, output, if_exists, append],
        )
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[appended]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[appended]),
        Ok(Word::TRUE)
    );

    fs::remove_file(path.0).unwrap();
}

#[test]
fn file_stream_io_returns_bytes_and_rejects_character_only_operations() {
    let (runtime, mut ctx) = setup();
    let open = builtin(&runtime, &mut ctx, "OPEN");
    let close = builtin(&runtime, &mut ctx, "CLOSE");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = builtin(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let output_string = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let direction = keyword(&runtime, &mut ctx, "DIRECTION");
    let input = keyword(&runtime, &mut ctx, "INPUT");
    let io = keyword(&runtime, &mut ctx, "IO");
    let path = path_word(&mut ctx, &runtime, "io-results");
    fs::write(&path.0, b"ab").unwrap();

    let input_stream = runtime
        .call_builtin(&mut ctx, open, &[path.1, direction, input])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[input_stream]),
        Ok(Word::fixnum(i64::from(b'a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[input_stream]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_byte,
            &[input_stream, Word::NIL, Word::fixnum(99)],
        ),
        Ok(Word::fixnum(99))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[input_stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[input_stream, Word::fixnum(3)]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), input_stream],),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('x')), input_stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[input_stream]),
        Ok(Word::TRUE)
    );

    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path.1, direction, io])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(i64::from(b'a')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(90), io_stream]),
        Ok(Word::fixnum(90))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[io_stream]),
        Ok(Word::fixnum(2))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[io_stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(fs::read(&path.0).unwrap(), b"aZ".to_vec());

    let string_output = runtime.call_builtin(&mut ctx, output_string, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[string_output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, get_output, &[input_stream]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[string_output, Word::TRUE],),
        Err(ncl_object::ObjectError::TypeError)
    );

    fs::remove_file(path.0).unwrap();
}
