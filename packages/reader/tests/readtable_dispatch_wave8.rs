#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Eighth-wave coverage for valid and invalid reader syntax paths.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify, classify_object, symbol_name};
use ncl_reader::{
    FloatFormat, ReadError, ReadOptions, ReadtableCase, read_from_string, readtable_case,
    standard_readtable,
};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, opts)
}

fn symbol_text(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap();
    let length = ncl_object::string_length(ctx, name).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn all_reachable_readtable_case_modes_fold_symbols_as_specified() {
    let (runtime, mut ctx, mut opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    for (code, expected_mode, expected_name) in [
        (1_i64, ReadtableCase::Downcase, "mixed"),
        (2_i64, ReadtableCase::Preserve, "MiXeD"),
        (3_i64, ReadtableCase::Invert, "mIxEd"),
    ] {
        ctx.write_object_slot(
            table.object().as_word(),
            ncl_object::readtable_offset::CASE,
            Word::fixnum(code),
        )
        .unwrap();
        assert_eq!(readtable_case(&ctx, table).unwrap(), expected_mode);
        opts.set_readtable(table);
        let word = read_from_string(&mut ctx, &runtime, "MiXeD", &opts)
            .unwrap()
            .unwrap();
        assert_eq!(symbol_text(&ctx, word), expected_name);
    }
}

#[test]
fn malformed_readtable_case_is_reported_without_reader_fallback() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    ctx.write_object_slot(
        table.object().as_word(),
        ncl_object::readtable_offset::CASE,
        Word::fixnum(99),
    )
    .unwrap();
    assert!(matches!(
        readtable_case(&ctx, table),
        Err(ReadError::InvalidNumber(_))
    ));
    let mut malformed = opts;
    malformed.set_readtable(table);
    assert!(matches!(
        read_from_string(&mut ctx, &runtime, "name", &malformed),
        Err(ReadError::InvalidNumber(_))
    ));
}

#[test]
fn valid_dispatch_forms_construct_expected_object_kinds() {
    let (runtime, mut ctx, opts) = setup();
    let function = read_from_string(&mut ctx, &runtime, "#'car", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, function),
        ObjectRef::Cons(_)
    ));
    let vector = read_from_string(&mut ctx, &runtime, "#(10 foo #\\space)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(_)
    ));
    let complex = read_from_string(&mut ctx, &runtime, "#c(3 -4)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, complex),
        ObjectRef::Complex(_)
    ));
    let uninterned = read_from_string(&mut ctx, &runtime, "#:local", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, uninterned), "LOCAL");
}

#[test]
fn invalid_dispatch_forms_report_their_specific_syntax_errors() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("#'", ReadError::UnexpectedEof),
        ("#:", ReadError::UnexpectedEof),
        ("#c(1", ReadError::UnexpectedEof),
        ("#S(POINT :X 1", ReadError::UnexpectedEof),
        ("#| nested #| comment", ReadError::UnexpectedEof),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected,
            "input: {text}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#?", &opts).unwrap_err(),
        ReadError::UndefinedDispatchMacro('?')
    );
}

#[test]
fn reader_macro_and_token_boundaries_preserve_valid_forms() {
    let (runtime, mut ctx, opts) = setup();
    let string = read_from_string(&mut ctx, &runtime, "\"a\\\"b\"", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
    let quoted = read_from_string(&mut ctx, &runtime, "(a b)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, quoted), ObjectRef::Cons(_)));
    let escaped = read_from_string(&mut ctx, &runtime, "A\\:B", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, escaped), "A:B");
    let empty = read_from_string(&mut ctx, &runtime, "#()", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, empty),
        ObjectRef::SimpleVector(_)
    ));
}

#[test]
fn number_markers_cover_default_and_explicit_float_behavior() {
    let (runtime, mut ctx, mut opts) = setup();
    let plain = read_from_string(&mut ctx, &runtime, "6.25", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, plain),
        ObjectRef::DoubleFloat(_)
    ));
    opts.set_default_float_format(FloatFormat::SingleFloat);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "6.25", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    let explicit_double = read_from_string(&mut ctx, &runtime, "6.25d0", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, explicit_double),
        ObjectRef::DoubleFloat(_)
    ));
    let integer = read_from_string(&mut ctx, &runtime, "42.", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(integer), ObjectRef::Fixnum(42));
}
