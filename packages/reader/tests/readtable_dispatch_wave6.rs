#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Sixth-wave coverage for reachable reader boundary branches.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify, classify_object, simple_vector_ref,
    symbol_name,
};
use ncl_reader::{
    FloatFormat, ReadBase, ReadError, ReadEvaluation, ReadOptions, copy_readtable,
    get_dispatch_macro_character, read_from_string, set_dispatch_macro_character,
    set_syntax_from_char,
};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let opts = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, opts)
}

fn symbol_text(ctx: &ThreadContext, word: Word) -> String {
    let name = symbol_name(ctx, word).unwrap_or_else(|_| unreachable!("not a symbol: {word:?}"));
    let length = ncl_object::string_length(ctx, name).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn copied_readtable_mutations_do_not_change_the_source() {
    let (runtime, mut ctx, opts) = setup();
    let original = opts.readtable();
    let copy = copy_readtable(&mut ctx, &runtime, original).unwrap();

    set_syntax_from_char(&mut ctx, ']', '(', copy, original).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "]", &opts)
            .unwrap()
            .map(|word| symbol_text(&ctx, word)),
        Some("]".to_owned())
    );
    let mut copy_opts = opts.clone();
    copy_opts.set_readtable(copy);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "]", &copy_opts).unwrap_err(),
        ReadError::UninvocableMacroFunction(']')
    );

    let function = Word::fixnum(41);
    set_dispatch_macro_character(&mut ctx, &runtime, copy, '#', 'q', function).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, original, '#', 'q').unwrap(),
        None
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copy, '#', 'q').unwrap(),
        Some(function)
    );
}

#[test]
fn reader_reports_each_dotted_list_termination_boundary() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("(.)", ReadError::DotWithoutCdr),
        ("(a .)", ReadError::UnmatchedRightParen),
        ("(a . b c)", ReadError::UnmatchedRightParen),
        ("(a", ReadError::UnexpectedEof),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected,
            "input: {text}"
        );
    }

    let empty = read_from_string(&mut ctx, &runtime, "()", &opts).unwrap();
    assert_eq!(empty, Some(Word::NIL));
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "'", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "\\", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
}

#[test]
fn dispatch_options_cover_disabled_eval_and_invalid_radices() {
    let (runtime, mut ctx, mut opts) = setup();
    opts.set_read_evaluation(ReadEvaluation::Disabled);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err(),
        ReadError::ReadEvalDisabled
    );
    for (text, expected) in [
        ("#0r1", ReadError::InvalidBase(0)),
        ("#37r1", ReadError::InvalidBase(37)),
        ("#b+", ReadError::InvalidNumber("+".to_owned())),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected,
            "input: {text}"
        );
    }

    let character = read_from_string(&mut ctx, &runtime, "#\\escape", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(character.as_character(), Some(0x1b));
}

#[test]
fn numeric_reader_boundaries_keep_numbers_and_symbols_distinct() {
    let (runtime, mut ctx, opts) = setup();
    let integer = read_from_string(&mut ctx, &runtime, "-12", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(integer), ObjectRef::Fixnum(-12));
    let float = read_from_string(&mut ctx, &runtime, "2.5e+1", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, float),
        ObjectRef::DoubleFloat(_)
    ));
    let ratio = read_from_string(&mut ctx, &runtime, "-8/-4", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));

    for text in ["+", "-", "1/", "/2", "1e"] {
        let symbol = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, symbol), ObjectRef::Symbol(_)),
            "input {text:?} produced {symbol:?}"
        );
        let name = symbol_name(&ctx, symbol)
            .unwrap_or_else(|_| unreachable!("input {text:?} produced non-symbol {symbol:?}"));
        let length = ncl_object::string_length(&ctx, name).unwrap();
        let actual: String = (0..length)
            .map(|index| ncl_object::string_ref(&ctx, name, index).unwrap())
            .collect();
        assert_eq!(actual, text.to_ascii_uppercase());
    }

    let mut hex_opts = opts;
    hex_opts.set_read_base(ReadBase::new(16).unwrap());
    let hex = read_from_string(&mut ctx, &runtime, "FACE", &hex_opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(hex), ObjectRef::Fixnum(0xface));
    let float = read_from_string(&mut ctx, &runtime, "1.5", &hex_opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, float),
        ObjectRef::DoubleFloat(_)
    ));
}

#[test]
fn parse_integer_end_and_float_format_edges_are_asserted() {
    let (runtime, mut ctx, mut opts) = setup();
    let (value, index) =
        ncl_reader::parse_integer(&mut ctx, &runtime, "12345 rest", None, None, Some(3)).unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(123));
    assert_eq!(index, 3);
    assert_eq!(
        ncl_reader::parse_integer(&mut ctx, &runtime, "x", Some(10), None, None).unwrap_err(),
        ReadError::InvalidNumber("x".to_owned())
    );
    assert_eq!(
        ncl_reader::parse_integer(&mut ctx, &runtime, "1", Some(1), None, None).unwrap_err(),
        ReadError::InvalidBase(1)
    );

    opts.set_default_float_format(FloatFormat::SingleFloat);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "3.0", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    let double = read_from_string(&mut ctx, &runtime, "3.0d0", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, double),
        ObjectRef::DoubleFloat(_)
    ));
}

#[test]
fn token_package_boundaries_report_missing_packages() {
    let (runtime, mut ctx, opts) = setup();
    let keyword = read_from_string(&mut ctx, &runtime, ":wave6", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, keyword), "WAVE6");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "NO-SUCH-PACKAGE:ITEM", &opts).unwrap_err(),
        ReadError::PackageNotFound("NO-SUCH-PACKAGE".to_owned())
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "NO-SUCH-PACKAGE:", &opts).unwrap_err(),
        ReadError::PackageNotFound("NO-SUCH-PACKAGE".to_owned())
    );

    let package_object = read_from_string(&mut ctx, &runtime, "COMMON-LISP:", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, package_object),
        ObjectRef::Package(_)
    ));
    let name = simple_vector_ref(&ctx, opts.readtable().syntax_table(&ctx).unwrap(), 32);
    assert!(name.is_ok());
}
