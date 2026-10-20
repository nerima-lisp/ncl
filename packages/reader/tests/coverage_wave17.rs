#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Wave 17 coverage tests for reachable reader dispatch and lexical edges.

use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, Word, car, cdr, classify, classify_object,
    make_simple_vector, structure_ref, symbol_name,
};
use ncl_reader::{
    FloatFormat, ReadError, ReadOptions, ReadSuppression, StringSource, copy_readtable,
    get_dispatch_macro_character, get_macro_character, make_dispatch_macro_character,
    parse_integer, read, read_from_string, readtable_case, set_dispatch_macro_character,
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
    let name = symbol_name(ctx, word).unwrap();
    let length = ncl_object::string_length(ctx, name).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, name, index).unwrap())
        .collect()
}

fn register_point(runtime: &Runtime, ctx: &mut ThreadContext) {
    let package = runtime.ensure_package(ctx, "COMMON-LISP-USER").unwrap();
    let package = Package::from_word(package);
    let (name, _) = package.intern(ctx, runtime, "W17-POINT").unwrap();
    let (x_name, _) = package.intern(ctx, runtime, "X").unwrap();
    let (y_name, _) = package.intern(ctx, runtime, "Y").unwrap();
    let x_descriptor = make_simple_vector(ctx, runtime, &[x_name, Word::NIL, Word::NIL]).unwrap();
    let y_descriptor = make_simple_vector(ctx, runtime, &[y_name, Word::NIL, Word::NIL]).unwrap();
    let slots = make_simple_vector(ctx, runtime, &[x_descriptor, y_descriptor]).unwrap();
    let class = make_simple_vector(
        ctx,
        runtime,
        &[name, Word::NIL, slots, Word::fixnum(12), slots],
    )
    .unwrap();
    runtime.define_class(ctx, "W17-POINT", class).unwrap();
    let qualified = runtime.structure_class_name(ctx, name).unwrap();
    runtime.define_class(ctx, qualified, class).unwrap();
    let layout = runtime.register_structure_layout(2).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
}

#[test]
fn structure_dispatch_accepts_slots_and_rejects_each_field_shape() {
    let (runtime, mut ctx, opts) = setup();
    register_point(&runtime, &mut ctx);
    let value = read_from_string(&mut ctx, &runtime, "#S(W17-POINT :X 31 :Y 47)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(structure_ref(&ctx, value, 0).unwrap(), Word::fixnum(31));
    assert_eq!(structure_ref(&ctx, value, 1).unwrap(), Word::fixnum(47));
    for input in [
        "#S",
        "#S W17-POINT",
        "#S()",
        "#S(W17-POINT :X 1 :Y 2 :Y 3)",
        "#S(W17-POINT :Z 1)",
        "#S(W17-POINT 1 2)",
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::StructureSyntax,
            "{input}"
        );
    }
}

#[test]
fn dispatch_vectors_bits_uninterned_and_read_eval_paths_are_asserted() {
    let (runtime, mut ctx, mut opts) = setup();
    for input in ["#()", "#(alpha 2)"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::SimpleVector(_)),
            "{input}"
        );
    }
    for input in ["#*", "#*001101", "#*101x"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::SpecializedArray(_)),
            "{input}"
        );
    }
    let uninterned = read_from_string(&mut ctx, &runtime, "#:wave17", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, uninterned),
        ObjectRef::Symbol(_)
    ));
    assert_eq!(symbol_text(&ctx, uninterned), "WAVE17");
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Enabled);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err(),
        ReadError::ReadEvalUnavailable
    );
    opts.set_read_evaluation(ncl_reader::ReadEvaluation::Disabled);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#.", &opts).unwrap_err(),
        ReadError::ReadEvalDisabled
    );
    for (input, error) in [
        ("#a", ReadError::ArraySyntax),
        ("#p", ReadError::PathnameSyntax),
        ("#?", ReadError::UndefinedDispatchMacro('?')),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            error
        );
    }
}

#[test]
fn radix_label_complex_and_character_dispatches_cover_error_edges() {
    let (runtime, mut ctx, opts) = setup();
    for (input, expected) in [("#b1010", 10), ("#o17", 15), ("#d19", 19), ("#x1f", 31)] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts)
                .unwrap()
                .unwrap()
                .as_fixnum(),
            Some(expected),
            "{input}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#9r1", &opts)
            .unwrap()
            .unwrap()
            .as_fixnum(),
        Some(1)
    );
    for input in ["#9", "#2r2", "#999999999999999999999999#"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidNumber(_))
                    | Err(ReadError::InvalidBase(_))
                    | Err(ReadError::NumberOutOfRange)
            ),
            "{input}"
        );
    }
    let complex = read_from_string(&mut ctx, &runtime, "#c(6 7)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, complex),
        ObjectRef::Complex(_)
    ));
    let labelled = read_from_string(&mut ctx, &runtime, "(#2=(x y) #2#)", &opts)
        .unwrap()
        .unwrap();
    let first = car(&ctx, labelled).unwrap();
    let second = car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap();
    assert_eq!(classify(first), classify(second));
    assert_eq!(symbol_text(&ctx, car(&ctx, first).unwrap()), "X");
    for input in ["#\\space", "#\\newline", "#\\A", "#\\unknown"] {
        let result = read_from_string(&mut ctx, &runtime, input, &opts);
        if input == "#\\unknown" {
            assert_eq!(
                result.unwrap_err(),
                ReadError::UnknownCharacterName("unknown".to_owned())
            );
        } else {
            assert!(result.unwrap().unwrap().as_character().is_some(), "{input}");
        }
    }
}

#[test]
fn reader_quotes_strings_suppression_and_list_boundaries_are_concrete() {
    let (runtime, mut ctx, mut opts) = setup();
    let quoted = read_from_string(&mut ctx, &runtime, "'alpha `beta ,gamma ,@delta", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, quoted).unwrap()), "QUOTE");
    let quasi = read_from_string(&mut ctx, &runtime, "`beta", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, quasi).unwrap()), "QUASIQUOTE");
    let string = read_from_string(&mut ctx, &runtime, r#""line\nquote\"slash\\""#, &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
    opts.set_read_suppression(ReadSuppression::Discard);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "(a b)", &opts).unwrap(),
        Some(Word::NIL)
    );
    opts.set_read_suppression(ReadSuppression::Keep);
    let mut source = StringSource::new("(a . b) c");
    let pair = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, pair).unwrap()), "A");
    assert_eq!(symbol_text(&ctx, cdr(&ctx, pair).unwrap()), "B");
    let tail = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, tail), "C");
    for input in ["(a .)", "(a . b c)", "(a", "'"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::DotWithoutCdr)
                    | Err(ReadError::UnexpectedEof)
                    | Err(ReadError::UnmatchedRightParen)
            ),
            "{input}"
        );
    }
}

#[test]
fn package_tokens_numbers_and_readtable_copy_keep_exact_contracts() {
    let (runtime, mut ctx, mut opts) = setup();
    for (input, expected) in [
        (":wave", "WAVE"),
        ("COMMON-LISP:CAR", "CAR"),
        ("A\\:B", "A:B"),
        ("|Keep|", "Keep"),
    ] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(symbol_text(&ctx, value), expected, "{input}");
    }
    for input in [":", "A:B:C", "A::B:C"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::InvalidSymbolToken(input.to_owned())
        );
    }
    for input in ["1/2", "-3/+2", "+4/-5"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::Ratio(_)),
            "{input}"
        );
    }
    for input in ["1.25", "1e2", "1d2", "1l2"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::DoubleFloat(_)),
            "{input}"
        );
    }
    opts.set_default_float_format(FloatFormat::SingleFloat);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "2.0", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    let table = copy_readtable(&mut ctx, &runtime, opts.readtable()).unwrap();
    assert_eq!(
        readtable_case(&ctx, table).unwrap(),
        ncl_reader::ReadtableCase::Upcase
    );
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
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', Word::fixnum(17)).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(Word::fixnum(17))
    );
    assert_eq!(
        get_macro_character(&mut ctx, table, '\u{100}').unwrap(),
        None
    );
}

#[test]
fn parse_integer_ranges_and_unterminated_tokens_report_precise_errors() {
    let (runtime, mut ctx, _opts) = setup();
    let (value, index) =
        parse_integer(&mut ctx, &runtime, "-2f tail", Some(16), Some(0), Some(3)).unwrap();
    assert_eq!(value.as_fixnum(), Some(-47));
    assert_eq!(index, 3);
    let (value, index) =
        parse_integer(&mut ctx, &runtime, "123xyz", None, Some(0), Some(3)).unwrap();
    assert_eq!(value.as_fixnum(), Some(123));
    assert_eq!(index, 3);
    for (radix, error) in [
        (Some(1), ReadError::InvalidBase(1)),
        (Some(37), ReadError::InvalidBase(37)),
    ] {
        assert_eq!(
            parse_integer(&mut ctx, &runtime, "10", radix, None, None).unwrap_err(),
            error
        );
    }
    for input in ["", "+", "-", "abc"] {
        assert_eq!(
            parse_integer(&mut ctx, &runtime, input, None, None, None).unwrap_err(),
            ReadError::InvalidNumber(input.to_owned())
        );
    }
}
