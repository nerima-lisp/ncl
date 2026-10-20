#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Bulk reachable coverage for the reader's syntax and dispatch branches.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify, classify_object, make_cons,
    simple_vector_set, symbol_name,
};
use ncl_reader::{
    ReadError, ReadOptions, ReadtableCase, read_from_string, readtable_case, standard_readtable,
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
fn syntax_table_entries_cover_whitespace_macros_escapes_custom_and_invalid() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    let syntax = table.syntax_table(&ctx).unwrap();

    simple_vector_set(&mut ctx, syntax, b'~'.into(), Word::fixnum(1)).unwrap();
    let value = read_from_string(&mut ctx, &runtime, "~ 41", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(41));

    simple_vector_set(&mut ctx, syntax, b'~'.into(), Word::fixnum(2)).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    simple_vector_set(&mut ctx, syntax, b'~'.into(), Word::fixnum(3)).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );

    simple_vector_set(&mut ctx, syntax, b'~'.into(), Word::fixnum(4)).unwrap();
    let single = read_from_string(&mut ctx, &runtime, "~A", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, single), "A");
    simple_vector_set(&mut ctx, syntax, b'~'.into(), Word::fixnum(5)).unwrap();
    let multiple = read_from_string(&mut ctx, &runtime, "~A|", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, multiple), "A");

    let function = Word::fixnum(7);
    for flag in [Word::NIL, Word::fixnum(1)] {
        let custom = make_cons(&mut ctx, &runtime, function, flag).unwrap();
        simple_vector_set(&mut ctx, syntax, b'~'.into(), custom).unwrap();
        assert_eq!(
            read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
            ReadError::UninvocableMacroFunction('~')
        );
    }
    simple_vector_set(&mut ctx, syntax, b'~'.into(), Word::fixnum(99)).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
}

#[test]
fn readtable_case_and_table_boundaries_are_observable() {
    let (runtime, mut ctx, opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    assert_eq!(readtable_case(&ctx, table).unwrap(), ReadtableCase::Upcase);
    assert_eq!(
        ncl_reader::get_macro_character(&mut ctx, table, 'λ').unwrap(),
        None
    );
    assert_eq!(
        ncl_reader::get_dispatch_macro_character(&mut ctx, table, '#', 'λ').unwrap(),
        None
    );
    assert_eq!(
        ncl_reader::get_dispatch_macro_character(&mut ctx, table, '!', 'x').unwrap_err(),
        ReadError::NotDispatchMacro('!')
    );
    assert_eq!(
        ncl_reader::set_dispatch_macro_character(
            &mut ctx,
            &runtime,
            table,
            '!',
            'x',
            Word::fixnum(1),
        )
        .unwrap_err(),
        ReadError::NotDispatchMacro('!')
    );
    assert!(!ncl_reader::readtablep(&ctx, Word::fixnum(1)));
    assert!(ncl_reader::readtablep(&ctx, table.object().as_word()));
    let mut copy_opts = opts;
    copy_opts.set_readtable(table);
    let name = read_from_string(&mut ctx, &runtime, "name", &copy_opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, name), "NAME");
}

#[test]
fn dispatch_radices_characters_vectors_and_comments_cover_valid_forms() {
    let (runtime, mut ctx, opts) = setup();
    for (text, value) in [("#b101", 5), ("#o17", 15), ("#d-12", -12), ("#XfF", 255)] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(classify(word), ObjectRef::Fixnum(value));
    }
    for (text, value) in [
        ("#\\tab", 9),
        ("#\\return", 13),
        ("#\\null", 0),
        ("#\\A", 65),
    ] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(word.as_character(), Some(value));
    }
    let vector = read_from_string(&mut ctx, &runtime, "#(1 #*10 #c(2 3))", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(_)
    ));
    let after_comment = read_from_string(&mut ctx, &runtime, "#| one #| two |# |# 88", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(after_comment), ObjectRef::Fixnum(88));
}

#[test]
fn dispatch_invalid_forms_cover_macro_specific_errors() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("#a", ReadError::ArraySyntax),
        ("#p", ReadError::PathnameSyntax),
        ("#.", ReadError::ReadEvalUnavailable),
        (
            "#cfoo",
            ReadError::InvalidNumber("expected '(' in complex literal".to_owned()),
        ),
        (
            "#\\unknown",
            ReadError::UnknownCharacterName("unknown".to_owned()),
        ),
        (
            "#1z",
            ReadError::InvalidNumber(
                "expected '=', '#', or 'r' after a radix or label number".to_owned(),
            ),
        ),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected,
            "input: {text}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#|", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
}

#[test]
fn reader_lists_strings_and_quotes_cover_termination_edges() {
    let (runtime, mut ctx, opts) = setup();
    let list = read_from_string(&mut ctx, &runtime, "(a b . c)", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, ncl_object::car(&ctx, list).unwrap()), "A");
    let tail = ncl_object::cdr(&ctx, list).unwrap();
    assert_eq!(symbol_text(&ctx, ncl_object::car(&ctx, tail).unwrap()), "B");
    assert_eq!(symbol_text(&ctx, ncl_object::cdr(&ctx, tail).unwrap()), "C");
    for text in ["(a", "(a .)", "(.)", "(a . b c)"] {
        assert!(
            matches!(
                read_from_string(&mut ctx, &runtime, text, &opts),
                Err(ReadError::UnexpectedEof
                    | ReadError::DotWithoutCdr
                    | ReadError::UnmatchedRightParen)
            ),
            "input: {text}"
        );
    }
    let quoted = read_from_string(&mut ctx, &runtime, "'x", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(
        symbol_text(&ctx, ncl_object::car(&ctx, quoted).unwrap()),
        "QUOTE"
    );
    let string = read_from_string(&mut ctx, &runtime, "\"a\\\"b\"", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
}

#[test]
fn token_markers_and_number_shapes_keep_errors_specific() {
    let (runtime, mut ctx, opts) = setup();
    let keyword = read_from_string(&mut ctx, &runtime, ":hello", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, keyword), "HELLO");
    for text in ["::x", "A:B:C", "A::B:C"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            ReadError::InvalidSymbolToken(text.to_owned())
        );
    }
    let ratio = read_from_string(&mut ctx, &runtime, "-3/+2", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    let integer = read_from_string(&mut ctx, &runtime, "99.", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(integer), ObjectRef::Fixnum(99));
    let symbol = read_from_string(&mut ctx, &runtime, "2e+", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "2E+");
}
