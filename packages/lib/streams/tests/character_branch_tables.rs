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

#[test]
fn character_input_modes_are_exact_for_table_cases() {
    let (runtime, mut ctx) = setup();
    let make_input = builtin(&runtime, &mut ctx, "MAKE-STRING-INPUT-STREAM");
    let peek = builtin(&runtime, &mut ctx, "PEEK-CHAR");
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_line = builtin(&runtime, &mut ctx, "READ-LINE");
    let unread = builtin(&runtime, &mut ctx, "UNREAD-CHAR");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");

    let peek_cases = [
        (Word::NIL, ' ', " ", ' '),
        (Word::TRUE, 'A', " \tA", 'A'),
        (Word::character(u32::from('A')), 'A', "  A", 'A'),
    ];
    for (peek_type, expected, source, read_expected) in peek_cases {
        let source_text = text(&mut ctx, &runtime, source);
        let stream = runtime
            .call_builtin(&mut ctx, make_input, &[source_text])
            .unwrap();
        assert_eq!(
            runtime.call_builtin(&mut ctx, peek, &[peek_type, stream]),
            Ok(Word::character(u32::from(expected))),
            "peek type {peek_type:?} over {source:?}",
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, read, &[stream]),
            Ok(Word::character(u32::from(read_expected))),
            "peek must preserve the input position for {source:?}",
        );
    }

    let bounded_text = text(&mut ctx, &runtime, "012345");
    let bounded = runtime
        .call_builtin(
            &mut ctx,
            make_input,
            &[
                bounded_text,
                start,
                Word::fixnum(2),
                end,
                Word::fixnum(4),
            ],
        )
        .unwrap();
    for expected in ['2', '3'] {
        assert_eq!(
            runtime.call_builtin(&mut ctx, read, &[bounded]),
            Ok(Word::character(u32::from(expected))),
        );
    }
    assert_eq!(
        runtime.call_builtin(&mut ctx, read, &[bounded, Word::NIL, Word::fixnum(17)]),
        Ok(Word::fixnum(17)),
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, unread, &[Word::character(u32::from('x')), bounded]),
        Ok(Word::character(u32::from('x'))),
    );

    let line_cases = [("line\n", "line", Word::NIL), ("tail", "tail", Word::TRUE)];
    for (source, expected, more) in line_cases {
        let source_text = text(&mut ctx, &runtime, source);
        let stream = runtime
            .call_builtin(&mut ctx, make_input, &[source_text])
            .unwrap();
        let line = runtime.call_builtin(&mut ctx, read_line, &[stream]).unwrap();
        assert_eq!(string(&ctx, line), expected);
        assert_eq!(ctx.values()[1], more, "line termination for {source:?}");
    }
    let empty_text = text(&mut ctx, &runtime, "");
    let empty = runtime
        .call_builtin(&mut ctx, make_input, &[empty_text])
        .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, read_line, &[empty, Word::NIL, Word::fixnum(18)]),
        Ok(Word::fixnum(18)),
    );
    assert_eq!(ctx.values()[1], Word::NIL);
}

#[test]
fn character_output_ranges_and_line_states_are_exact_for_table_cases() {
    let (runtime, mut ctx) = setup();
    let make_output = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let write_string = builtin(&runtime, &mut ctx, "WRITE-STRING");
    let write_line = builtin(&runtime, &mut ctx, "WRITE-LINE");
    let fresh_line = builtin(&runtime, &mut ctx, "FRESH-LINE");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let start = keyword(&runtime, &mut ctx, "START");
    let end = keyword(&runtime, &mut ctx, "END");

    let range_cases = [
        (vec![], "abcdef", "abcdef"),
        (vec![Word::fixnum(1), Word::fixnum(4)], "abcdef", "bcd"),
        (
            vec![start, Word::fixnum(2), end, Word::fixnum(5)],
            "abcdef",
            "cde",
        ),
    ];
    for (options, source, expected) in range_cases {
        let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
        let mut args = vec![text(&mut ctx, &runtime, source), output];
        args.extend(options.iter().copied());
        assert_eq!(
            runtime.call_builtin(&mut ctx, write_string, &args),
            Ok(args[0]),
            "write-string range {options:?}",
        );
        let value = runtime.call_builtin(&mut ctx, get_output, &[output]).unwrap();
        assert_eq!(string(&ctx, value), expected);
        let reset = runtime.call_builtin(&mut ctx, get_output, &[output]).unwrap();
        assert_eq!(string(&ctx, reset), "");
    }

    let output = runtime.call_builtin(&mut ctx, make_output, &[]).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL),
    );
    let line_text = text(&mut ctx, &runtime, "xy");
    assert_eq!(
        runtime.call_builtin(
            &mut ctx,
            write_line,
            &[line_text, output],
        ),
        Ok(line_text),
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, fresh_line, &[output]),
        Ok(Word::NIL),
    );
    let value = runtime.call_builtin(&mut ctx, get_output, &[output]).unwrap();
    assert_eq!(string(&ctx, value), "xy\n");
}

#[test]
fn data_stream_character_and_byte_paths_return_exact_table_values() {
    let (runtime, mut ctx) = setup();
    let read = builtin(&runtime, &mut ctx, "READ-CHAR");
    let read_byte = builtin(&runtime, &mut ctx, "READ-BYTE");
    let write = builtin(&runtime, &mut ctx, "WRITE-CHAR");
    let write_byte = builtin(&runtime, &mut ctx, "WRITE-BYTE");

    let cases = [('X', 88_i64), ('Y', 89_i64)];
    for (character, byte) in cases {
        let stream = data_stream(&runtime, &mut ctx, b"ab");
        assert_eq!(
            runtime.call_builtin(
                &mut ctx,
                write,
                &[Word::character(u32::from(character)), stream],
            ),
            Ok(Word::character(u32::from(character))),
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, read, &[stream]),
            Ok(Word::character(u32::from('b'))),
        );
        assert_eq!(
            runtime.call_builtin(&mut ctx, read_byte, &[stream, Word::NIL, Word::fixnum(29)]),
            Ok(Word::fixnum(29)),
        );

        let writable = data_stream(&runtime, &mut ctx, b"_");
        assert_eq!(
            runtime.call_builtin(&mut ctx, write_byte, &[Word::fixnum(byte), writable]),
            Err(ncl_object::ObjectError::TypeError),
        );
    }
}
