#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Wave 16 coverage tests for reachable reader branches.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify, classify_object, symbol_name,
};
use ncl_reader::{
    FloatFormat, ReadBase, ReadError, ReadEvaluation, ReadOptions, ReadSuppression, Readtable,
    ReadtableCase, StringSource, copy_readtable, get_dispatch_macro_character, get_macro_character,
    make_dispatch_macro_character, parse_integer, read, read_delimited_list, read_from_string,
    read_preserving_whitespace, readtable_case, set_dispatch_macro_character, set_syntax_from_char,
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
fn read_options_setters_and_getters_preserve_each_reader_setting() {
    let (runtime, mut ctx, mut opts) = setup();
    assert_eq!(opts.read_base().value(), 10);
    assert_eq!(opts.read_evaluation(), ReadEvaluation::Enabled);
    assert_eq!(opts.read_suppression(), ReadSuppression::Keep);
    assert_eq!(opts.default_float_format(), FloatFormat::DoubleFloat);
    assert!(opts.current_package().is_none());
    opts.set_read_base(ReadBase::new(16).unwrap());
    opts.set_read_evaluation(ReadEvaluation::Disabled);
    opts.set_read_suppression(ReadSuppression::Discard);
    opts.set_default_float_format(FloatFormat::DoubleFloat);
    opts.set_current_package("COMMON-LISP").unwrap();
    assert_eq!(opts.read_base().value(), 16);
    assert_eq!(opts.read_evaluation(), ReadEvaluation::Disabled);
    assert_eq!(opts.read_suppression(), ReadSuppression::Discard);
    assert_eq!(opts.default_float_format(), FloatFormat::DoubleFloat);
    assert_eq!(opts.current_package().unwrap().as_str(), "COMMON-LISP");
    assert_eq!(
        opts.set_current_package(""),
        Err(ReadError::InvalidSymbolToken(String::new()))
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "ff", &opts)
            .unwrap()
            .unwrap(),
        Word::NIL
    );
    opts.set_read_suppression(ReadSuppression::Keep);
    let value = read_from_string(&mut ctx, &runtime, "ff", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(value.as_fixnum(), Some(255));
}

#[test]
fn dispatch_macro_forms_cover_function_vector_complex_and_radix_errors() {
    let (runtime, mut ctx, opts) = setup();
    let function = read_from_string(&mut ctx, &runtime, "#'car", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, function).unwrap()), "FUNCTION");
    assert_eq!(
        symbol_text(&ctx, car(&ctx, cdr(&ctx, function).unwrap()).unwrap()),
        "CAR"
    );
    let vector = read_from_string(&mut ctx, &runtime, "#(1 2)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(_)
    ));
    for input in ["#'", "#(", "#cfoo", "#2r2"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::UnexpectedEof | ReadError::InvalidNumber(_) | ReadError::Object(_),)
            ),
            "{input}"
        );
    }
    for input in ["#c(4)", "#c(4 5)"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::Complex(_)),
            "{input}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#37r1", &opts).unwrap_err(),
        ReadError::InvalidBase(37)
    );
}

#[test]
fn feature_labels_comments_and_character_names_cover_dispatch_boundaries() {
    let (runtime, mut ctx, opts) = setup();
    runtime.add_feature("W16");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+w16 10", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(10)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#-w16 11 12", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(12)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+(and w16 missing) 13", &opts).unwrap(),
        None
    );
    for input in ["#+", "#-(and 1)", "#+(xor w16) 1"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidFeatureExpression | ReadError::Object(_))
            ),
            "{input}"
        );
    }
    let labelled = read_from_string(&mut ctx, &runtime, "(#4=foo #4#)", &opts)
        .unwrap()
        .unwrap();
    let left = car(&ctx, labelled).unwrap();
    let right = car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap();
    assert_eq!(classify(left), classify(right));
    assert_eq!(symbol_text(&ctx, left), "FOO");
    for (input, expected) in [("#\\space", 32), ("#\\return", 13), ("#\\altmode", 27)] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts)
                .unwrap()
                .unwrap()
                .as_character(),
            Some(expected)
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#| nested #| x |# y |# 14", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(14)
    );
}

#[test]
fn reader_entrypoints_and_list_edges_assert_consumption_and_errors() {
    let (runtime, mut ctx, opts) = setup();
    let mut source = StringSource::new(" \n alpha   beta");
    let alpha = read_preserving_whitespace(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, alpha), "ALPHA");
    let beta = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, beta), "BETA");
    assert_eq!(read(&mut ctx, &runtime, &mut source, &opts).unwrap(), None);
    let mut delimited = StringSource::new("one ; ignored\ntwo)");
    let list = read_delimited_list(&mut ctx, &runtime, &mut delimited, &opts).unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, list).unwrap()), "ONE");
    assert_eq!(
        symbol_text(&ctx, car(&ctx, cdr(&ctx, list).unwrap()).unwrap()),
        "TWO"
    );
    for input in ["'", "`", ",", ",@", "\"abc\\", "(a"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::UnexpectedEof,
            "{input:?}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, ")", &opts).unwrap_err(),
        ReadError::UnmatchedRightParen
    );
}

#[test]
fn token_package_markers_escapes_and_numeric_forms_are_distinct() {
    let (runtime, mut ctx, opts) = setup();
    for (input, expected) in [
        (":keyword", "KEYWORD"),
        ("COMMON-LISP:CAR", "CAR"),
        ("COMMON-LISP::CAR", "CAR"),
        ("A\\:B", "A:B"),
        ("|MiXeD|", "MiXeD"),
    ] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(symbol_text(&ctx, word), expected, "{input}");
    }
    for input in [":", "A:B:C", "A:::B"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::InvalidSymbolToken(input.to_owned())
        );
    }
    for input in ["1/2", "-3/+2"] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, word), ObjectRef::Ratio(_)),
            "{input}"
        );
    }
}

#[test]
fn parse_integer_and_readtable_copy_cover_bounds_and_table_slots() {
    let (runtime, mut ctx, mut opts) = setup();
    let (value, index) =
        parse_integer(&mut ctx, &runtime, "-2f tail", Some(16), None, Some(3)).unwrap();
    assert_eq!(value.as_fixnum(), Some(-47));
    assert_eq!(index, 3);
    let (value, index) =
        parse_integer(&mut ctx, &runtime, "123xyz", Some(10), Some(0), Some(3)).unwrap();
    assert_eq!(value.as_fixnum(), Some(123));
    assert_eq!(index, 3);
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "none", Some(1), None, None).unwrap_err(),
        ReadError::InvalidBase(1)
    );
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "+", None, None, None).unwrap_err(),
        ReadError::InvalidNumber("+".to_owned())
    );
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    let copied = copy_readtable(&mut ctx, &runtime, table).unwrap();
    set_syntax_from_char(&mut ctx, '~', '(', copied, table).unwrap();
    opts.set_readtable(copied);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    set_syntax_from_char(&mut ctx, '~', 'x', copied, table).unwrap();
    let symbol = read_from_string(&mut ctx, &runtime, "~", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "~");
    make_dispatch_macro_character(&mut ctx, &runtime, copied, '#').unwrap();
    set_dispatch_macro_character(&mut ctx, &runtime, copied, '#', 'q', Word::fixnum(9)).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copied, '#', 'q').unwrap(),
        Some(Word::fixnum(9))
    );
    assert_eq!(
        get_macro_character(&mut ctx, copied, '\u{100}').unwrap(),
        None
    );
    assert_eq!(readtable_case(&ctx, copied).unwrap(), ReadtableCase::Upcase);
    assert_eq!(Readtable::from_object(copied.object()), copied);
}
