#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Wave 18 coverage tests for exact reachable reader branches.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify_object, symbol_name};
use ncl_reader::{
    FloatFormat, ReadError, ReadOptions, ReadSuppression, StringSource, copy_readtable,
    get_dispatch_macro_character, get_macro_character, make_dispatch_macro_character,
    parse_integer, read, read_from_string, set_dispatch_macro_character, set_syntax_from_char,
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
fn syntax_table_copy_exercises_whitespace_escape_and_macro_kinds() {
    let (runtime, mut ctx, mut opts) = setup();
    let table = copy_readtable(&mut ctx, &runtime, opts.readtable()).unwrap();
    set_syntax_from_char(&mut ctx, '~', ' ', table, table).unwrap();
    opts.set_readtable(table);
    let word = read_from_string(&mut ctx, &runtime, "~ alpha", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, word), "ALPHA");

    set_syntax_from_char(&mut ctx, '~', '\\', table, table).unwrap();
    let escaped = read_from_string(&mut ctx, &runtime, "~a", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, escaped), "a");

    set_syntax_from_char(&mut ctx, '~', '|', table, table).unwrap();
    let multiple = read_from_string(&mut ctx, &runtime, "~a b|", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, multiple), "a b");

    set_syntax_from_char(&mut ctx, '~', '#', table, table).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    set_syntax_from_char(&mut ctx, '~', '(', table, table).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    assert_eq!(get_macro_character(&mut ctx, table, '~').unwrap(), None);
}

#[test]
fn dispatch_labels_vectors_uninterned_and_error_branches_are_asserted() {
    let (runtime, mut ctx, opts) = setup();
    let function = read_from_string(&mut ctx, &runtime, "#'car", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, function).unwrap()), "FUNCTION");
    let vector = read_from_string(&mut ctx, &runtime, "#(one 2)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(_)
    ));
    let fresh = read_from_string(&mut ctx, &runtime, "#:fresh", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, fresh), "FRESH");
    let labelled = read_from_string(&mut ctx, &runtime, "(#5=(left right) #5#)", &opts)
        .unwrap()
        .unwrap();
    let first = car(&ctx, labelled).unwrap();
    let second = car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, first).unwrap()), "LEFT");
    assert_eq!(symbol_text(&ctx, car(&ctx, second).unwrap()), "LEFT");
    for input in ["#'", "#:", "#5#", "#=", "#?", "#a", "#p"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::UnexpectedEof
                    | ReadError::InvalidNumber(_)
                    | ReadError::UndefinedDispatchMacro(_)
                    | ReadError::ArraySyntax
                    | ReadError::PathnameSyntax,)
            ),
            "{input}"
        );
    }
    assert!(matches!(
        read_from_string(&mut ctx, &runtime, "#c(1)", &opts),
        Ok(Some(word)) if matches!(classify_object(&ctx, word), ObjectRef::Complex(_))
    ));
}

#[test]
fn numbers_cover_bignum_ratio_float_and_parse_integer_bounds() {
    let (runtime, mut ctx, mut opts) = setup();
    for input in [
        "4611686018427387904",
        "4611686018427387905",
        "-4611686018427387905",
    ] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, word), ObjectRef::Bignum(_)),
            "{input}"
        );
    }
    for input in ["1/2", "-3/+2", "+4/-5"] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, word), ObjectRef::Ratio(_)),
            "{input}"
        );
    }
    for input in ["1.0", "2e3", "3d-2", "4l+1"] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, word), ObjectRef::DoubleFloat(_)),
            "{input}"
        );
    }
    opts.set_default_float_format(FloatFormat::SingleFloat);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "1.0", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    let (word, index) = parse_integer(&mut ctx, &runtime, "42xyz", None, Some(0), Some(2)).unwrap();
    assert_eq!(word.as_fixnum(), Some(42));
    assert_eq!(index, 2);
    let (word, index) =
        parse_integer(&mut ctx, &runtime, "00123", None, Some(2), Some(99)).unwrap();
    assert_eq!(word.as_fixnum(), Some(123));
    assert_eq!(index, 5);
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "no digits", None, Some(8), None).unwrap_err(),
        ReadError::InvalidNumber("no digits".to_owned())
    );
}

#[test]
fn reader_comments_strings_quotes_suppression_and_dotted_edges_are_specific() {
    let (runtime, mut ctx, mut opts) = setup();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "; comment\n17", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(17)
    );
    let string = read_from_string(&mut ctx, &runtime, r#""a\nb\"c\\d""#, &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
    let quote = read_from_string(&mut ctx, &runtime, "'item", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, quote).unwrap()), "QUOTE");
    opts.set_read_suppression(ReadSuppression::Discard);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "item", &opts).unwrap(),
        Some(Word::NIL)
    );
    opts.set_read_suppression(ReadSuppression::Keep);
    let mut source = StringSource::new("(a . b) c");
    let pair = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, pair).unwrap()), "A");
    assert_eq!(symbol_text(&ctx, cdr(&ctx, pair).unwrap()), "B");
    let trailing = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, trailing), "C");
    for input in ["(a .)", "(a . b c)", "(a", "`", ",@"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::DotWithoutCdr
                    | ReadError::UnexpectedEof
                    | ReadError::UnmatchedRightParen,)
            ),
            "{input}"
        );
    }
}

#[test]
fn readtable_dispatch_mutators_cover_non_ascii_and_existing_slots() {
    let (runtime, mut ctx, mut opts) = setup();
    let table = copy_readtable(&mut ctx, &runtime, opts.readtable()).unwrap();
    set_syntax_from_char(&mut ctx, '~', '(', table, table).unwrap();
    opts.set_readtable(table);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    set_syntax_from_char(&mut ctx, '~', 'x', table, table).unwrap();
    let tilde = read_from_string(&mut ctx, &runtime, "~", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, tilde), "~");
    make_dispatch_macro_character(&mut ctx, &runtime, table, '#').unwrap();
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', Word::fixnum(18)).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(Word::fixnum(18))
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', '\u{100}').unwrap(),
        None
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '+', 'q').unwrap_err(),
        ReadError::NotDispatchMacro('+')
    );
    assert_eq!(
        get_macro_character(&mut ctx, table, '\u{100}').unwrap(),
        None
    );
}
