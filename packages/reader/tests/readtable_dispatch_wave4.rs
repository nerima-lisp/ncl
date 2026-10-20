#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Fourth-wave coverage for reachable numeric, token, readtable, and dispatch paths.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify, classify_object, symbol_name};
use ncl_reader::{
    FloatFormat, ReadError, ReadOptions, ReadSuppression, parse_integer, read_from_string,
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
fn parses_integer_ranges_signs_and_radix_boundaries() {
    let (runtime, mut ctx, _opts) = setup();
    let (word, index) = parse_integer(&mut ctx, &runtime, "-1 rest", None, None, None).unwrap();
    assert_eq!(classify(word), ObjectRef::Fixnum(-1));
    assert_eq!(index, 2);

    let (word, index) =
        parse_integer(&mut ctx, &runtime, "+FF yy", Some(16), Some(0), Some(3)).unwrap();
    assert_eq!(classify(word), ObjectRef::Fixnum(255));
    assert_eq!(index, 3);
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "123", Some(1), None, None).unwrap_err(),
        ReadError::InvalidBase(1)
    );
    assert_eq!(
        parse_integer(&mut ctx, &runtime, " + ", None, None, None).unwrap_err(),
        ReadError::InvalidNumber(" + ".to_owned())
    );
}

#[test]
fn covers_numeric_shapes_and_default_float_selection() {
    let (runtime, mut ctx, mut opts) = setup();
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "123.", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(123)
    );
    let ratio = read_from_string(&mut ctx, &runtime, "-3/+2", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    for text in ["1e2", "1d2", "1l2"] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert!(matches!(
            classify_object(&ctx, word),
            ObjectRef::DoubleFloat(_)
        ));
    }
    opts.set_default_float_format(FloatFormat::SingleFloat);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "1.25", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#x", &opts).unwrap_err(),
        ReadError::InvalidNumber(String::new())
    );
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "#36rZ", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(35)
    );
}

#[test]
fn distinguishes_escaped_and_unescaped_package_markers() {
    let (runtime, mut ctx, opts) = setup();
    let escaped = read_from_string(&mut ctx, &runtime, "A\\:B", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, escaped), "A:B");
    let package = read_from_string(&mut ctx, &runtime, "COMMON-LISP:", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, package),
        ObjectRef::Package(_)
    ));
    let symbol = read_from_string(&mut ctx, &runtime, "COMMON-LISP::CAR", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "CAR");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, ":", &opts).unwrap_err(),
        ReadError::InvalidSymbolToken(":".to_owned())
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "A:B:C", &opts).unwrap_err(),
        ReadError::InvalidSymbolToken("A:B:C".to_owned())
    );
}

#[test]
fn reports_escape_eof_and_preserves_non_numeric_symbols() {
    let (runtime, mut ctx, opts) = setup();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "\\", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "|unterminated", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    let symbol = read_from_string(&mut ctx, &runtime, "1e+", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "1E+");
    let symbol = read_from_string(&mut ctx, &runtime, "1/", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "1/");
}

#[test]
fn exercises_feature_short_circuit_and_read_suppression() {
    let (runtime, mut ctx, mut opts) = setup();
    runtime.add_feature("ALPHA");
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "#+(or beta alpha) 7", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(7)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+(and) 7", &opts).unwrap(),
        Some(Word::fixnum(7))
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#-(or alpha beta) 7", &opts).unwrap(),
        None
    );
    opts.set_read_suppression(ReadSuppression::Discard);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "99", &opts).unwrap(),
        Some(Word::NIL)
    );
}

#[test]
fn covers_dispatch_signs_and_malformed_numeric_suffixes() {
    let (runtime, mut ctx, opts) = setup();
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "#d+42", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(42)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#b+", &opts).unwrap_err(),
        ReadError::InvalidNumber("+".to_owned())
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#12z1", &opts).unwrap_err(),
        ReadError::InvalidNumber(
            "expected '=', '#', or 'r' after a radix or label number".to_owned()
        )
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#999999999999999999999r1", &opts).unwrap_err(),
        ReadError::NumberOutOfRange
    );
}
