#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Broad input matrices for the remaining reachable reader regions.

use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, Word, car, cdr, classify, classify_object,
    make_cons, make_string, symbol_name, symbol_package,
};
use ncl_reader::{
    FloatFormat, ReadError, ReadEvaluation, ReadOptions, StringSource,
    get_dispatch_macro_character, get_macro_character, make_dispatch_macro_character,
    parse_integer, read, read_delimited_list, read_from_string, read_preserving_whitespace,
    set_dispatch_macro_character, set_syntax_from_char, standard_readtable,
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
fn dispatch_matrix_covers_each_radix_character_and_label_shape() {
    let (runtime, mut ctx, opts) = setup();
    for (input, expected) in [
        ("#b101", 5),
        ("#B-101", -5),
        ("#o77", 63),
        ("#O-10", -8),
        ("#d42", 42),
        ("#D-7", -7),
        ("#xdead", 57005),
        ("#X-ff", -255),
        ("#16rff", 255),
    ] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(
            classify(word),
            ObjectRef::Fixnum(expected),
            "input: {input}"
        );
    }
    for input in ["#1r1", "#37r1", "#2r", "#b2", "#xg"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidBase(_))
                    | Err(ReadError::InvalidNumber(_))
                    | Err(ReadError::InvalidDigit(_))
            ),
            "input: {input}"
        );
    }

    let labelled = read_from_string(&mut ctx, &runtime, "(#0=alpha #0# #1=beta #1#)", &opts)
        .unwrap()
        .unwrap();
    let first = car(&ctx, labelled).unwrap();
    let second = car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap();
    let third = car(&ctx, cdr(&ctx, cdr(&ctx, labelled).unwrap()).unwrap()).unwrap();
    let fourth = car(
        &ctx,
        cdr(&ctx, cdr(&ctx, cdr(&ctx, labelled).unwrap()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(symbol_text(&ctx, first), "ALPHA");
    assert_eq!(symbol_text(&ctx, second), "ALPHA");
    assert_eq!(symbol_text(&ctx, third), "BETA");
    assert_eq!(symbol_text(&ctx, fourth), "BETA");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#9#", &opts).unwrap_err(),
        ReadError::InvalidNumber("undefined label #9".to_owned())
    );
}

#[test]
fn dispatch_object_matrix_covers_vectors_bits_chars_comments_and_errors() {
    let (runtime, mut ctx, mut opts) = setup();
    let function = read_from_string(&mut ctx, &runtime, "#'car", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, function).unwrap()), "FUNCTION");
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#'", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );

    for input in ["#()", "#(1 2 3)", "#*", "#*010110", "#:fresh"] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(
                classify_object(&ctx, word),
                ObjectRef::SimpleVector(_) | ObjectRef::SpecializedArray(_) | ObjectRef::Symbol(_)
            ),
            "input: {input}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#:", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(
            &mut ctx,
            &runtime,
            "#| nested #| comment |# done |# 71",
            &opts
        )
        .unwrap()
        .unwrap()
        .as_fixnum(),
        Some(71)
    );
    for (input, expected) in [
        ("#a", ReadError::ArraySyntax),
        ("#p", ReadError::PathnameSyntax),
        ("#?", ReadError::UndefinedDispatchMacro('?')),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            expected
        );
    }
    opts.set_read_evaluation(ReadEvaluation::Disabled);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err(),
        ReadError::ReadEvalDisabled
    );
}

#[test]
fn character_and_feature_matrices_keep_reader_errors_specific() {
    let (runtime, mut ctx, opts) = setup();
    for (input, code) in [
        ("#\\space", 32),
        ("#\\newline", 10),
        ("#\\tab", 9),
        ("#\\return", 13),
        ("#\\page", 12),
        ("#\\backspace", 8),
        ("#\\delete", 127),
        ("#\\escape", 27),
        ("#\\Q", 81),
        ("#\\?", 63),
    ] {
        let character = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(character.as_character(), Some(code), "input: {input}");
    }
    for input in ["#\\", "#\\unknown", "#\\two-words"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidCharacter) | Err(ReadError::UnknownCharacterName(_))
            ),
            "input: {input}"
        );
    }

    runtime.add_feature("W13-A");
    runtime.add_feature("W13-B");
    for (input, expected) in [
        ("#+w13-a 11", Some(11)),
        ("#-w13-a 12", None),
        ("#+(and w13-a w13-b) 13", Some(13)),
        ("#+(or missing w13-b) 14", Some(14)),
        ("#+(not missing) 15", Some(15)),
    ] {
        let result = read_from_string(&mut ctx, &runtime, input, &opts).unwrap();
        assert_eq!(
            result.and_then(|word| word.as_fixnum()),
            expected,
            "input: {input}"
        );
    }
    for input in ["#+", "#-(and 1)", "#+(unknown w13-a) 1"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidFeatureExpression) | Err(ReadError::Object(_))
            ),
            "input: {input}"
        );
    }
}

#[test]
fn reader_entry_points_and_list_boundaries_are_checked_as_a_matrix() {
    let (runtime, mut ctx, opts) = setup();
    let mut source = StringSource::new("  alpha  beta");
    let first = read_preserving_whitespace(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    let second = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, first), "ALPHA");
    assert_eq!(symbol_text(&ctx, second), "BETA");

    let mut delimited = StringSource::new("one two)");
    let list = read_delimited_list(&mut ctx, &runtime, &mut delimited, &opts).unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, list).unwrap()), "ONE");
    assert_eq!(
        symbol_text(&ctx, car(&ctx, cdr(&ctx, list).unwrap()).unwrap()),
        "TWO"
    );
    assert_eq!(cdr(&ctx, cdr(&ctx, list).unwrap()).unwrap(), Word::NIL);

    for input in ["'", "`", ",", ",@", "\"unterminated", "(one", "(one .)"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::UnexpectedEof)
                    | Err(ReadError::DotWithoutCdr)
                    | Err(ReadError::UnmatchedRightParen)
            ),
            "input: {input}"
        );
    }
    let quoted = read_from_string(&mut ctx, &runtime, "`x ,x ,@x", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, quoted).unwrap()), "QUASIQUOTE");
}

#[test]
fn number_matrix_distinguishes_symbols_ratios_floats_and_integer_limits() {
    let (runtime, mut ctx, mut opts) = setup();
    for (input, expected) in [("0", 0), ("+17", 17), ("-17", -17), ("123.", 123)] {
        let number = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(
            classify(number),
            ObjectRef::Fixnum(expected),
            "input: {input}"
        );
    }
    for input in ["1/2", "-3/+2", "+4/-5"] {
        let ratio = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    }
    for input in ["1.25", "1e2", "1E+2", "1d2", "1l2"] {
        let float = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(matches!(
            classify_object(&ctx, float),
            ObjectRef::DoubleFloat(_)
        ));
    }
    opts.set_default_float_format(FloatFormat::SingleFloat);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "1.0", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    for input in ["/", "1/", "/1", "1/+", "1/2x", "1e", "1e+"] {
        let word = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, word), ObjectRef::Symbol(_)),
            "input: {input}"
        );
    }
    let (integer, index) = parse_integer(&mut ctx, &runtime, "12345", None, None, Some(3)).unwrap();
    assert_eq!(integer.as_fixnum(), Some(123));
    assert_eq!(index, 3);
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "-", None, None, None).unwrap_err(),
        ReadError::InvalidNumber("-".to_owned())
    );
}

#[test]
fn token_package_and_readtable_mutation_matrices_preserve_names() {
    let (runtime, mut ctx, mut opts) = setup();
    for (input, expected) in [
        (":hello", "HELLO"),
        ("COMMON-LISP:CAR", "CAR"),
        ("A\\:B", "A:B"),
    ] {
        let symbol = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(symbol_text(&ctx, symbol), expected, "input: {input}");
    }
    for input in [":", "A:B:C", "A::B:C", "A:::B"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::InvalidSymbolToken(input.to_owned())
        );
    }

    let current = runtime.ensure_package(&mut ctx, "W13-CURRENT").unwrap();
    let target = runtime.ensure_package(&mut ctx, "W13-TARGET").unwrap();
    let nickname = make_string(&mut ctx, &runtime, &['L', 'O', 'C', 'A', 'L']).unwrap();
    let entry = make_cons(&mut ctx, &runtime, nickname, target).unwrap();
    let local_nicknames = make_cons(&mut ctx, &runtime, entry, Word::NIL).unwrap();
    Package::from_word(current)
        .set_local_nicknames(&mut ctx, local_nicknames)
        .unwrap();
    opts.set_current_package("W13-CURRENT").unwrap();
    let local = read_from_string(&mut ctx, &runtime, "LOCAL:item", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_package(&ctx, local).unwrap(), target);
    assert_eq!(symbol_text(&ctx, local), "ITEM");

    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    opts.set_readtable(table);
    set_syntax_from_char(&mut ctx, '~', '(', table, table).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    set_syntax_from_char(&mut ctx, '~', 'a', table, table).unwrap();
    assert_eq!(get_macro_character(&mut ctx, table, '~').unwrap(), None);
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', Word::fixnum(8)).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(Word::fixnum(8))
    );
    make_dispatch_macro_character(&mut ctx, &runtime, table, '#').unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'z').unwrap(),
        Some(Word::NIL)
    );
}
