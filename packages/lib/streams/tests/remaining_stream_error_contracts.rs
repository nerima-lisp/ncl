#![allow(missing_docs)]
#![allow(clippy::unwrap_used, reason = "tests assert on builtin behavior")]

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_simple_vector, make_stream,
    make_string,
};

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

fn stream_with_state(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
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

fn standard_stream(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "COMMON-LISP").unwrap();
    let symbol = Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0;
    ncl_object::symbol_value(ctx, symbol).unwrap()
}

fn text(ctx: &mut ThreadContext, runtime: &Runtime, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn malformed_stream_states_return_specific_layout_and_type_errors() {
    let (runtime, mut ctx) = setup();
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let peek_char = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let finish_output = builtin(&runtime, &mut ctx, "FINISH-OUTPUT");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let character = Word::character(u32::from('x'));

    let unknown_kind = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(-99), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[unknown_kind]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[character, unknown_kind]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[unknown_kind]),
        Err(ncl_object::ObjectError::Layout)
    );

    let non_numeric_kind = stream_with_state(&runtime, &mut ctx, &[Word::TRUE, Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[non_numeric_kind]),
        Err(ncl_object::ObjectError::Layout)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, peek_char, &[Word::NIL, non_numeric_kind]),
        Err(ncl_object::ObjectError::Layout)
    );

    let empty_data = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(0), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            read_char,
            &[empty_data, Word::NIL, Word::fixnum(91)],
        ),
        Ok(Word::fixnum(91))
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[character, empty_data]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let short_input = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(-1), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[short_input]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[character, short_input]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let short_output = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(-2), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, write_char, &[character, short_output]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[short_output]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let closed = stream_with_state(&runtime, &mut ctx, &[Word::fixnum(-3), Word::fixnum(0)]);
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_byte, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, finish_output, &[closed]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let standard_input = standard_stream(&runtime, &mut ctx, "*STANDARD-INPUT*");
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[standard_input]),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn file_state_paths_reject_non_text_paths_and_invalid_stream_shapes() {
    let (runtime, mut ctx) = setup();
    let read_char = builtin(&runtime, &mut ctx, "READ-CHAR");
    let write_char = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let file_length = builtin(&runtime, &mut ctx, "FILE-LENGTH");
    let file_position = builtin(&runtime, &mut ctx, "FILE-POSITION");
    let character = Word::character(u32::from('q'));

    let file_output_with_bad_path = stream_with_state(
        &runtime,
        &mut ctx,
        &[Word::fixnum(-4), Word::fixnum(0), Word::TRUE],
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_char,
            &[character, file_output_with_bad_path],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[file_output_with_bad_path]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[file_output_with_bad_path],),
        Ok(Word::fixnum(0))
    );

    let file_io_with_bad_path = stream_with_state(
        &runtime,
        &mut ctx,
        &[Word::fixnum(-5), Word::fixnum(0), Word::TRUE],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_char, &[file_io_with_bad_path]),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_length, &[file_io_with_bad_path]),
        Err(ncl_object::ObjectError::TypeError)
    );

    let invalid_position = stream_with_state(
        &runtime,
        &mut ctx,
        &[Word::fixnum(0), Word::fixnum(0), Word::fixnum(65)],
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, file_position, &[invalid_position, Word::TRUE],),
        Err(ncl_object::ObjectError::TypeError)
    );
}

#[test]
fn adapter_option_values_report_type_errors_without_false_results() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let source = text(&mut ctx, &runtime, "abc");
    let output = runtime.call_builtin(&mut ctx, stream, &[]).unwrap();
    let start = Package::from_word(runtime.ensure_package(&mut ctx, "KEYWORD").unwrap())
        .intern(&mut ctx, &runtime, "START")
        .unwrap()
        .0;
    let end = Package::from_word(runtime.ensure_package(&mut ctx, "KEYWORD").unwrap())
        .intern(&mut ctx, &runtime, "END")
        .unwrap()
        .0;

    assert_eq!(
        runtime.call_builtin(&mut ctx, write_string, &[source, output, start, Word::TRUE],),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_string,
            &[source, output, end, Word::fixnum(4)],
        ),
        Err(ncl_object::ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, make_input, &[source, Word::fixnum(-1)],),
        Err(ncl_object::ObjectError::TypeError)
    );
}
