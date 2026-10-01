#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Wave 15 tests for reachable reader edge paths.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify, classify_object, make_readtable,
    readtable_dispatch, readtable_syntax, symbol_name,
};
use ncl_reader::{
    ReadError, ReadOptions, ReadSuppression, Readtable, ReadtableCase, StringSource,
    copy_readtable, get_dispatch_macro_character, get_macro_character,
    make_dispatch_macro_character, read, read_from_string, readtable_case, readtablep,
    set_dispatch_macro_character, set_syntax_from_char,
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
fn readtable_case_variants_and_raw_accessors_are_observable() {
    let (runtime, mut ctx, mut opts) = setup();
    let standard = opts.readtable();
    assert!(readtablep(&ctx, standard.object().as_word()));
    assert!(!readtablep(&ctx, Word::fixnum(0)));
    assert!(readtablep(&ctx, standard.object().as_word()));
    let syntax = standard.syntax_table(&ctx).unwrap();
    let dispatch = standard.dispatch_table(&ctx).unwrap();
    for (code, expected) in [
        (0, ReadtableCase::Upcase),
        (1, ReadtableCase::Downcase),
        (2, ReadtableCase::Preserve),
        (3, ReadtableCase::Invert),
    ] {
        let raw = make_readtable(&mut ctx, &runtime, syntax, dispatch, Word::fixnum(code)).unwrap();
        let table = Readtable::from_object(raw);
        assert_eq!(readtable_case(&ctx, table).unwrap(), expected);
        opts.set_readtable(table);
        let word = read_from_string(&mut ctx, &runtime, "AbC", &opts)
            .unwrap()
            .unwrap();
        let expected_name = match expected {
            ReadtableCase::Upcase => "ABC",
            ReadtableCase::Downcase => "abc",
            ReadtableCase::Preserve => "AbC",
            ReadtableCase::Invert => "aBc",
            _ => unreachable!("unknown readtable case"),
        };
        assert_eq!(symbol_text(&ctx, word), expected_name);
    }
    assert_eq!(readtable_syntax(&ctx, standard.object()).unwrap(), syntax);
    assert_eq!(
        readtable_dispatch(&ctx, standard.object()).unwrap(),
        dispatch
    );
}

#[test]
fn readtable_non_ascii_and_dispatch_mutations_cover_no_index_paths() {
    let (runtime, mut ctx, mut opts) = setup();
    let table = copy_readtable(&mut ctx, &runtime, opts.readtable()).unwrap();
    assert_eq!(
        get_macro_character(&mut ctx, table, '\u{100}').unwrap(),
        None
    );
    set_syntax_from_char(&mut ctx, '\u{100}', '(', table, table).unwrap();
    set_syntax_from_char(&mut ctx, '~', '\u{100}', table, table).unwrap();
    assert_eq!(get_macro_character(&mut ctx, table, '~').unwrap(), None);
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', '\u{100}', Word::fixnum(6))
        .unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', '\u{100}').unwrap(),
        None
    );
    assert_eq!(
        set_dispatch_macro_character(&mut ctx, &runtime, table, 'x', 'q', Word::NIL).unwrap_err(),
        ReadError::NotDispatchMacro('x')
    );
    assert_eq!(
        make_dispatch_macro_character(&mut ctx, &runtime, table, 'x').unwrap_err(),
        ReadError::NotDispatchMacro('x')
    );
    opts.set_readtable(table);
    let name = read_from_string(&mut ctx, &runtime, "name", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, name), "NAME");
}

#[test]
fn reader_suppression_comments_strings_and_dotted_lists_are_checked() {
    let (runtime, mut ctx, mut opts) = setup();
    opts.set_read_suppression(ReadSuppression::Discard);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "(alpha 42)", &opts)
            .unwrap()
            .unwrap(),
        Word::NIL
    );
    opts.set_read_suppression(ReadSuppression::Keep);
    for (input, expected) in [("; comment without newline", None), ("; c\n17", Some(17))] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .and_then(|value| value.as_fixnum());
        assert_eq!(word, expected, "{input:?}");
    }
    let string = read_from_string(&mut ctx, &runtime, r#""a\"b\\c""#, &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
    let mut source = StringSource::new("(left . right) next");
    let pair = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, pair).unwrap()), "LEFT");
    assert_eq!(symbol_text(&ctx, cdr(&ctx, pair).unwrap()), "RIGHT");
    let next = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, next), "NEXT");
    for input in ["(. x)", "(x .", "(x . y z)"] {
        assert!(matches!(
            read_from_string(&mut ctx, &runtime, input, &opts),
            Err(ReadError::DotWithoutCdr)
                | Err(ReadError::UnexpectedEof)
                | Err(ReadError::UnmatchedRightParen)
        ));
    }
}

#[test]
fn dispatch_feature_skip_labels_and_character_errors_are_distinct() {
    let (runtime, mut ctx, opts) = setup();
    runtime.add_feature("W15-FEATURE");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#-w15-feature 1 2", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(2)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+missing 3 4", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(4)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#| #| nested |# |# 21", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(21)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#| unterminated", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    let labelled = read_from_string(&mut ctx, &runtime, "(#7=(a b) #7#)", &opts)
        .unwrap()
        .unwrap();
    let first = car(&ctx, labelled).unwrap();
    let second = car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap();
    assert_eq!(classify(first), classify(second));
    assert_eq!(symbol_text(&ctx, car(&ctx, first).unwrap()), "A");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#8#", &opts).unwrap_err(),
        ReadError::InvalidNumber("undefined label #8".to_owned())
    );
    for input in ["#\\", "#\\unknown", "#\\A-B"] {
        assert!(matches!(
            read_from_string(&mut ctx, &runtime, input, &opts),
            Err(ReadError::InvalidCharacter) | Err(ReadError::UnknownCharacterName(_))
        ));
    }
}

#[test]
fn number_shapes_cover_bignum_radix_and_invalid_float_paths() {
    let (runtime, mut ctx, mut opts) = setup();
    let big = read_from_string(&mut ctx, &runtime, "4611686018427387904", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, big), ObjectRef::Bignum(_)));
    let negative = read_from_string(&mut ctx, &runtime, "-4611686018427387904", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(negative.as_fixnum(), Some(-4611686018427387904));
    opts.set_read_base(ncl_reader::ReadBase::new(16).unwrap());
    let hex = read_from_string(&mut ctx, &runtime, "7f", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(hex.as_fixnum(), Some(127));
    for input in ["1.2e+", "1/", "+/2"] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, word), ObjectRef::Symbol(_)),
            "{input}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#36rZ", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(35)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#36r-zz", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(-1295)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#2r2", &opts).unwrap_err(),
        ReadError::InvalidNumber("".to_owned())
    );
}
