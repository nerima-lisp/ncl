#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Focused coverage for readtable, dispatch, number, and reader type edges.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify_object};
use ncl_reader::{
    ReadError, ReadOptions, get_dispatch_macro_character, get_macro_character,
    make_dispatch_macro_character, parse_integer, read_from_string, set_dispatch_macro_character,
    standard_readtable,
};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, opts)
}

#[test]
fn dispatch_default_fills_only_empty_slots_and_non_ascii_is_a_noop() {
    let (runtime, mut ctx, opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', Word::fixnum(7)).unwrap();
    make_dispatch_macro_character(&mut ctx, &runtime, table, '#').unwrap();

    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(Word::fixnum(7))
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'z').unwrap(),
        Some(Word::NIL)
    );
    assert_eq!(
        get_macro_character(&mut ctx, table, '\u{100}').unwrap(),
        None
    );
    let mut opts = opts;
    opts.set_readtable(table);
    let name = read_from_string(&mut ctx, &runtime, "name", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, name), ObjectRef::Symbol(_)));
}

#[test]
fn dispatch_type_edges_report_precise_errors() {
    let (runtime, mut ctx, opts) = setup();
    for input in ["#S(UNKNOWN :X 1)", "#S(UNKNOWN :X)", "#S(UNKNOWN :X 1 :Y)"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::StructureSyntax,
            "input: {input}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#c", &opts).unwrap_err(),
        ReadError::InvalidNumber("expected '(' in complex literal".to_owned())
    );
    assert_eq!(
        read_from_string(
            &mut ctx,
            &runtime,
            "#999999999999999999999999999999999999999999=1",
            &opts
        )
        .unwrap_err(),
        ReadError::NumberOutOfRange
    );
}

#[test]
fn integer_boundaries_and_explicit_float_markers_are_observable() {
    let (runtime, mut ctx, opts) = setup();
    let (value, index) =
        parse_integer(&mut ctx, &runtime, "  -2f tail", Some(16), Some(0), Some(5)).unwrap();
    assert_eq!(value.as_fixnum(), Some(-47));
    assert_eq!(index, 5);
    for input in ["1.0s2", "1.0f2"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::FloatFormatUnavailable(input.chars().nth(3).unwrap().to_ascii_lowercase()),
            "input: {input}"
        );
    }
    let word = read_from_string(&mut ctx, &runtime, "1.0d2", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, word),
        ObjectRef::DoubleFloat(_)
    ));
}

#[test]
fn reader_eof_edges_are_not_silently_converted_to_empty_forms() {
    let (runtime, mut ctx, opts) = setup();
    for input in ["'", "`", ",", ",@", "\"abc\\"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::UnexpectedEof,
            "input: {input:?}"
        );
    }
    let empty = read_from_string(&mut ctx, &runtime, "()", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(empty, Word::NIL);
}
