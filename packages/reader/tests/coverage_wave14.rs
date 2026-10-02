#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Wave 14 coverage tests for reachable dispatch, readtable, and token paths.

use ncl_object::{
    ObjectRef, Package, Runtime, ThreadContext, Word, car, cdr, classify_object,
    make_simple_vector, structure_ref, symbol_name,
};
use ncl_reader::{
    FloatFormat, ReadError, ReadOptions, ReadtableCase, StringSource, copy_readtable,
    get_dispatch_macro_character, get_macro_character, make_dispatch_macro_character,
    parse_integer, read_from_string, readtable_case, set_dispatch_macro_character,
    set_syntax_from_char, standard_readtable,
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
    let (name, _) = package.intern(ctx, runtime, "W14-POINT").unwrap();
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
    runtime.define_class(ctx, "W14-POINT", class).unwrap();
    let qualified = runtime.structure_class_name(ctx, name).unwrap();
    runtime.define_class(ctx, qualified, class).unwrap();
    let layout = runtime.register_structure_layout(2).unwrap();
    runtime
        .register_structure_class_with_parent(ctx, layout, None, name)
        .unwrap();
}

#[test]
fn complex_and_label_boundaries_have_specific_results() {
    let (runtime, mut ctx, opts) = setup();
    for input in ["#c(8)", "#c(8 2)", "#c(1 2 3)"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::Complex(_)),
            "{input}"
        );
    }
    for input in ["#c", "#cfoo"] {
        assert!(
            read_from_string(&mut ctx, &runtime, input, &opts).is_err(),
            "{input}"
        );
    }

    let labelled = read_from_string(&mut ctx, &runtime, "(#12=alpha #12#)", &opts)
        .unwrap()
        .unwrap();
    let first = car(&ctx, labelled).unwrap();
    let second = car(&ctx, cdr(&ctx, labelled).unwrap()).unwrap();
    assert_eq!(symbol_text(&ctx, first), "ALPHA");
    assert_eq!(symbol_text(&ctx, second), "ALPHA");
    for input in ["#999999999999999999999999=foo", "(#1=foo #2#)"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::InvalidNumber(_) | ReadError::NumberOutOfRange)
            ),
            "{input}"
        );
    }
}

#[test]
fn structure_shape_errors_and_complete_slots_are_distinct() {
    let (runtime, mut ctx, opts) = setup();
    register_point(&runtime, &mut ctx);
    let value = read_from_string(&mut ctx, &runtime, "#S(W14-POINT :X 8 :Y 9)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(structure_ref(&ctx, value, 0).unwrap(), Word::fixnum(8));
    assert_eq!(structure_ref(&ctx, value, 1).unwrap(), Word::fixnum(9));
    for input in [
        "#S(W14-POINT . X)",
        "#S(W14-POINT :X 8 :Y)",
        "#S(W14-POINT :X 8 :Y 9 :X 10)",
        "#S(W14-POINT 1 2)",
        "#S(UNKNOWN :X 1)",
        "#S(1 :X 1)",
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::StructureSyntax,
            "{input}"
        );
    }
}

#[test]
fn readtable_copy_and_character_boundaries_preserve_entries() {
    let (runtime, mut ctx, mut opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    assert_eq!(readtable_case(&ctx, table).unwrap(), ReadtableCase::Upcase);
    assert_eq!(
        get_macro_character(&mut ctx, table, '\u{100}').unwrap(),
        None
    );
    let copied = copy_readtable(&mut ctx, &runtime, table).unwrap();
    set_syntax_from_char(&mut ctx, '~', '(', copied, table).unwrap();
    opts.set_readtable(copied);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    set_syntax_from_char(&mut ctx, '~', '\u{100}', copied, table).unwrap();
    assert_eq!(get_macro_character(&mut ctx, copied, '~').unwrap(), None);
    make_dispatch_macro_character(&mut ctx, &runtime, copied, '#').unwrap();
    set_dispatch_macro_character(&mut ctx, &runtime, copied, '#', 'q', Word::fixnum(44)).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copied, '#', 'q').unwrap(),
        Some(Word::fixnum(44))
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copied, '#', '\u{100}').unwrap(),
        None
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copied, '+', 'q').unwrap_err(),
        ReadError::NotDispatchMacro('+')
    );
    opts.set_readtable(copied);
    let mixed = read_from_string(&mut ctx, &runtime, "MiXeD", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, mixed), "MIXED");
}

#[test]
fn token_package_markers_and_character_escapes_cover_edges() {
    let (runtime, mut ctx, opts) = setup();
    for (input, expected) in [
        ("COMMON-LISP:", "COMMON-LISP"),
        ("COMMON-LISP::", "COMMON-LISP"),
    ] {
        let package_object = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(matches!(
            classify_object(&ctx, package_object),
            ObjectRef::Package(_)
        ));
        let name = Package::from_word(package_object).name(&ctx).unwrap();
        let length = ncl_object::string_length(&ctx, name).unwrap();
        let text: String = (0..length)
            .map(|index| ncl_object::string_ref(&ctx, name, index).unwrap())
            .collect();
        assert_eq!(text, expected);
    }
    for input in ["A:::B", "A:B:C"] {
        assert!(matches!(
            read_from_string(&mut ctx, &runtime, input, &opts),
            Err(ReadError::InvalidSymbolToken(_) | ReadError::PackageNotFound(_))
        ));
    }
    let escaped = read_from_string(&mut ctx, &runtime, "|MiXeD|", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, escaped), "MiXeD");
    for (input, code) in [("#\\linefeed", 10), ("#\\rubout", 127), ("#\\null", 0)] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts)
                .unwrap()
                .unwrap()
                .as_character(),
            Some(code)
        );
    }
}

#[test]
fn number_markers_and_parse_integer_bounds_are_asserted() {
    let (runtime, mut ctx, mut opts) = setup();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "1s2", &opts).unwrap_err(),
        ReadError::FloatFormatUnavailable('s')
    );
    {
        let input = "1f2";
        assert_eq!(
            read_from_string(&mut ctx, &runtime, input, &opts).unwrap_err(),
            ReadError::FloatFormatUnavailable('f')
        );
    }
    for input in ["1D2", "1L2"] {
        let value = read_from_string(&mut ctx, &runtime, input, &opts)
            .unwrap()
            .unwrap();
        assert!(
            matches!(classify_object(&ctx, value), ObjectRef::DoubleFloat(_)),
            "{input}"
        );
    }
    opts.set_default_float_format(FloatFormat::DoubleFloat);
    assert!(matches!(
        read_from_string(&mut ctx, &runtime, "1.2e+", &opts),
        Ok(Some(word)) if matches!(classify_object(&ctx, word), ObjectRef::Symbol(_))
    ));
    let (value, index) =
        parse_integer(&mut ctx, &runtime, "7f-tail", Some(16), Some(0), Some(2)).unwrap();
    assert_eq!(value.as_fixnum(), Some(127));
    assert_eq!(index, 2);
    assert_eq!(
        parse_integer(&mut ctx, &runtime, "000", None, Some(1), Some(1)).unwrap_err(),
        ReadError::InvalidNumber("000".to_owned())
    );
}

#[test]
fn reader_delimiters_and_dispatch_errors_keep_boundaries() {
    let (runtime, mut ctx, opts) = setup();
    let mut source = StringSource::new("(a . b) trailing");
    let list = ncl_reader::read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, list).unwrap()), "A");
    assert_eq!(symbol_text(&ctx, cdr(&ctx, list).unwrap()), "B");
    let trailing = ncl_reader::read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, trailing), "TRAILING");
    for input in [")", "(a . b c)", "(a . . b)", "#\\unknown-name"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, input, &opts),
                Err(ReadError::UnmatchedRightParen
                    | ReadError::DotWithoutCdr
                    | ReadError::UnknownCharacterName(_),)
            ),
            "{input}"
        );
    }
}
