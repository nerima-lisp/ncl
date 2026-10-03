use ncl_object::{
    make_simple_vector, make_stream, make_string, simple_vector_ref, stream_state, string_length,
    string_ref, FunctionObject, Package, Runtime, ThreadContext, Word,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register: {error:?}"));
    crate::register(&runtime).unwrap_or_else(|error| panic!("streams: {error:?}"));
    (runtime, ctx)
}

fn function(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(
        runtime
            .function(ctx, "COMMON-LISP", name)
            .unwrap_or_else(|| panic!("missing builtin {name}")),
    )
    .unwrap_or_else(|error| panic!("invalid builtin {name}: {error:?}"))
}

fn string(ctx: &ThreadContext, value: Word) -> String {
    (0..string_length(ctx, value).unwrap_or_else(|error| panic!("length: {error:?}")))
        .map(|index| string_ref(ctx, value, index).unwrap_or_else(|error| panic!("ref: {error:?}")))
        .collect()
}

fn text(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>())
        .unwrap_or_else(|error| panic!("string: {error:?}"))
}

fn keyword(ctx: &mut ThreadContext, runtime: &Runtime, name: &str) -> Word {
    let package = runtime
        .ensure_package(ctx, "KEYWORD")
        .unwrap_or_else(|error| panic!("keyword package: {error:?}"));
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap_or_else(|error| panic!("keyword: {error:?}"))
        .0
}

fn data_stream(ctx: &mut ThreadContext, runtime: &Runtime, bytes: &[u8]) -> Word {
    let values = std::iter::once(Word::fixnum(0))
        .chain(std::iter::once(Word::fixnum(0)))
        .chain(bytes.iter().map(|byte| Word::fixnum(i64::from(*byte))))
        .collect::<Vec<_>>();
    let state = make_simple_vector(ctx, runtime, &values)
        .unwrap_or_else(|error| panic!("state: {error:?}"));
    make_stream(
        ctx,
        runtime,
        Word::NIL,
        Word::NIL,
        Word::NIL,
        state,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("stream: {error:?}"))
    .into()
}

#[test]
fn input_options_cover_peek_eof_unread_and_data_bytes() {
    let (runtime, mut ctx) = setup();
    let make_input = function(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let read_char = function(&runtime, &mut ctx, "READ-CHAR");
    let peek_char = function(&runtime, &mut ctx, "PEEK-CHAR");
    let unread_char = function(&runtime, &mut ctx, "UNREAD-CHAR");
    let read_line = function(&runtime, &mut ctx, "READ-LINE");
    let source = text(&mut ctx, &runtime, "  a\tb");
    let stream = runtime
        .call_builtin(&mut ctx, make_input, &[source])
        .unwrap_or_else(|error| panic!("input: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::TRUE, stream]),
        Ok(Word::character(u32::from('a')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[
                Word::character(u32::from('b')),
                stream,
                Word::NIL,
                Word::TRUE
            ],
        ),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream]),
        Ok(Word::character(u32::from('b')))
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('b')), stream]
        ),
        Ok(Word::character(u32::from('b')))
    );
    let line = runtime
        .call_builtin(&mut ctx, read_line, &[stream, Word::NIL, Word::TRUE])
        .unwrap_or_else(|error| panic!("read line: {error:?}"));
    assert_eq!(string(&ctx, line), "b");
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[stream, Word::NIL, Word::TRUE]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('x')), stream]
        ),
        Ok(Word::character(u32::from('x')))
    );

    let data = data_stream(&mut ctx, &runtime, b"A");
    let read_byte = function(&runtime, &mut ctx, "READ-BYTE");
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[data]),
        Ok(Word::fixnum(65))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[data, Word::NIL, Word::TRUE]),
        Ok(Word::TRUE)
    );
}

#[test]
fn output_and_predicates_cover_data_and_invalid_streams() {
    let (runtime, mut ctx) = setup();
    let data = data_stream(&mut ctx, &runtime, b"_");
    let write_char = function(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = function(&runtime, &mut ctx, "WRITE-BYTE");
    let finish = function(&runtime, &mut ctx, "FINISH-OUTPUT");
    let streamp = function(&runtime, &mut ctx, "STREAMP");
    let inputp = function(&runtime, &mut ctx, "INPUT-STREAM-P");
    let outputp = function(&runtime, &mut ctx, "OUTPUT-STREAM-P");
    let length = function(&runtime, &mut ctx, "FILE-LENGTH");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[Word::character(u32::from('Z')), data]
        ),
        Ok(Word::character(u32::from('Z')))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(1), data]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish, &[data]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, streamp, &[data]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, streamp, &[Word::NIL]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, inputp, &[data]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, outputp, &[data]),
        Ok(Word::NIL)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, length, &[data]),
        Ok(Word::fixnum(1))
    );
}

#[test]
fn file_options_cover_missing_existing_io_and_metadata() {
    let (runtime, mut ctx) = setup();
    let open = function(&runtime, &mut ctx, "OPEN");
    let close = function(&runtime, &mut ctx, "CLOSE");
    let read_byte = function(&runtime, &mut ctx, "READ-BYTE");
    let write_byte = function(&runtime, &mut ctx, "WRITE-BYTE");
    let file_position = function(&runtime, &mut ctx, "FILE-POSITION");
    let file_length = function(&runtime, &mut ctx, "FILE-LENGTH");
    let file_string_length = function(&runtime, &mut ctx, "FILE-STRING-LENGTH");
    let path = format!("/tmp/ncl-streams-coverage-{}", std::process::id());
    let path_word = text(&mut ctx, &runtime, &path);
    let direction = keyword(&mut ctx, &runtime, "DIRECTION");
    let input = keyword(&mut ctx, &runtime, "INPUT");
    let output = keyword(&mut ctx, &runtime, "OUTPUT");
    let io = keyword(&mut ctx, &runtime, "IO");
    let missing = keyword(&mut ctx, &runtime, "IF-DOES-NOT-EXIST");
    let nil = keyword(&mut ctx, &runtime, "NIL");
    let create = keyword(&mut ctx, &runtime, "CREATE");
    let exists = keyword(&mut ctx, &runtime, "IF-EXISTS");
    let append = keyword(&mut ctx, &runtime, "APPEND");
    let error = keyword(&mut ctx, &runtime, "ERROR");
    let missing_path = text(&mut ctx, &runtime, &format!("{path}-missing"));
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[missing_path, direction, input, missing, nil]
        ),
        Ok(Word::NIL)
    );
    let stream = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, missing, create],
        )
        .unwrap_or_else(|error| panic!("open output: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(65), stream]),
        Ok(Word::fixnum(65))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, close, &[stream]),
        Ok(Word::TRUE)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, error],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    let appended = runtime
        .call_builtin(
            &mut ctx,
            open,
            &[path_word, direction, output, exists, append],
        )
        .unwrap_or_else(|error| panic!("open append: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[appended]),
        Ok(Word::fixnum(1))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[appended]),
        Ok(Word::fixnum(1))
    );
    runtime
        .call_builtin(&mut ctx, close, &[appended])
        .unwrap_or_else(|error| unreachable!("close append: {error:?}"));

    let io_stream = runtime
        .call_builtin(&mut ctx, open, &[path_word, direction, io])
        .unwrap_or_else(|error| panic!("open io: {error:?}"));
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[io_stream]),
        Ok(Word::fixnum(65))
    );
    let unicode = text(&mut ctx, &runtime, "é");
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_string_length, &[io_stream, unicode],),
        Ok(Word::fixnum(2))
    );
    runtime
        .call_builtin(&mut ctx, close, &[io_stream])
        .unwrap_or_else(|error| unreachable!("close io: {error:?}"));
    std::fs::remove_file(path).unwrap_or_else(|error| panic!("remove: {error:?}"));
}

#[test]
fn state_and_registration_values_are_stable() {
    assert_eq!(crate::registration::REGISTERED_BUILTINS.len(), 28);
    assert_eq!(crate::character::StreamKind::Data.code(), 0);
    assert_eq!(crate::character::StreamKind::Closed.code(), -3);
    assert_eq!(crate::character::StreamKind::StandardTwoWay.code(), -9);
    let (runtime, mut ctx) = setup();
    let stream = data_stream(&mut ctx, &runtime, b"");
    let state = stream_state(&ctx, ncl_object::Stream::from_word(stream))
        .unwrap_or_else(|error| unreachable!("state: {error:?}"));
    assert_eq!(simple_vector_ref(&ctx, state, 0), Ok(Word::fixnum(0)));
}

#[test]
fn character_input_boundaries_return_values_instead_of_errors() {
    let (runtime, mut ctx) = setup();
    let make_input = function(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let read_char = function(&runtime, &mut ctx, "READ-CHAR");
    let peek_char = function(&runtime, &mut ctx, "PEEK-CHAR");
    let read_line = function(&runtime, &mut ctx, "READ-LINE");
    let unread_char = function(&runtime, &mut ctx, "UNREAD-CHAR");

    let newline_source = text(&mut ctx, &runtime, "\n");
    let newline = runtime
        .call_builtin(&mut ctx, make_input, &[newline_source])
        .unwrap_or_else(|error| panic!("input: {error:?}"));
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            peek_char,
            &[Word::character(u32::from('\n')), newline],
        ),
        Ok(Word::character(u32::from('\n')))
    );
    let empty_line = runtime
        .call_builtin(&mut ctx, read_line, &[newline, Word::NIL, Word::NIL])
        .unwrap_or_else(|error| panic!("read line: {error:?}"));
    assert_eq!(string(&ctx, empty_line), "");
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[newline, Word::NIL, Word::fixnum(99)]),
        Ok(Word::fixnum(99))
    );
    let empty_source = text(&mut ctx, &runtime, "");
    let empty_stream = runtime
        .call_builtin(&mut ctx, make_input, &[empty_source])
        .unwrap_or_else(|error| panic!("input: {error:?}"));
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            unread_char,
            &[Word::character(u32::from('x')), empty_stream],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );

    let final_source = text(&mut ctx, &runtime, "last");
    let final_line = runtime
        .call_builtin(&mut ctx, make_input, &[final_source])
        .unwrap_or_else(|error| panic!("input: {error:?}"));
    let line = runtime
        .call_builtin(&mut ctx, read_line, &[final_line])
        .unwrap_or_else(|error| panic!("read line: {error:?}"));
    assert_eq!(string(&ctx, line), "last");
    assert_eq!(ctx.values().len(), 2);
    assert_eq!(ctx.values()[1], Word::TRUE);
}
