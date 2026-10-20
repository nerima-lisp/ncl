#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Seventh-wave coverage for reachable readtable and reader classification paths.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify_object, make_cons, simple_vector_set,
    symbol_name,
};
use ncl_reader::{FloatFormat, ReadError, ReadOptions, read_from_string, set_syntax_from_char};

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
fn readtable_classifies_custom_macro_entries_and_exposes_functions() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    let syntax = table.syntax_table(&ctx).unwrap();
    let function = Word::fixnum(123);
    let terminating = make_cons(&mut ctx, &runtime, function, Word::NIL).unwrap();
    simple_vector_set(&mut ctx, syntax, usize::from(b'~'), terminating).unwrap();
    assert_eq!(
        ncl_reader::get_macro_character(&mut ctx, table, '~').unwrap(),
        Some(function)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );

    let non_terminating = make_cons(&mut ctx, &runtime, function, Word::fixnum(1)).unwrap();
    simple_vector_set(&mut ctx, syntax, usize::from(b'!'), non_terminating).unwrap();
    assert_eq!(
        ncl_reader::get_macro_character(&mut ctx, table, '!').unwrap(),
        Some(function)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "!", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('!')
    );
}

#[test]
fn readtable_invalid_entries_are_rejected_by_the_reader() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    let syntax = table.syntax_table(&ctx).unwrap();
    for (character, entry) in [('~', Word::fixnum(99)), ('!', Word::NIL)] {
        simple_vector_set(&mut ctx, syntax, character as usize, entry).unwrap();
        assert_eq!(
            read_from_string(&mut ctx, &runtime, &character.to_string(), &opts).unwrap_err(),
            ReadError::UninvocableMacroFunction(character)
        );
    }
    set_syntax_from_char(&mut ctx, '~', 'a', table, table).unwrap();
    let symbol = read_from_string(&mut ctx, &runtime, "~", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "~");
}

#[test]
fn reader_distinguishes_dotted_tokens_from_dotted_pairs() {
    let (runtime, mut ctx, opts) = setup();
    let list = read_from_string(&mut ctx, &runtime, "(a .b)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, list), ObjectRef::Cons(_)));
    let first = ncl_object::car(&ctx, list).unwrap();
    let rest = ncl_object::cdr(&ctx, list).unwrap();
    assert_eq!(symbol_text(&ctx, first), "A");
    let second = ncl_object::car(&ctx, rest).unwrap();
    assert_eq!(symbol_text(&ctx, second), ".B");

    assert_eq!(
        read_from_string(&mut ctx, &runtime, "(a . b c)", &opts).unwrap_err(),
        ReadError::UnmatchedRightParen
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "(. a)", &opts).unwrap_err(),
        ReadError::DotWithoutCdr
    );
}

#[test]
fn token_escapes_preserve_case_and_delimiters() {
    let (runtime, mut ctx, opts) = setup();
    let multiple = read_from_string(&mut ctx, &runtime, "|MiXeD Name|", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, multiple), "MiXeD Name");
    let single = read_from_string(&mut ctx, &runtime, "A\\ B", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, single), "A B");
    let escaped_pipe = read_from_string(&mut ctx, &runtime, "|A|B", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, escaped_pipe), "AB");
}

#[test]
fn dispatch_numeric_and_character_errors_cover_remaining_shapes() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("#a", ReadError::ArraySyntax),
        ("#p", ReadError::PathnameSyntax),
        (
            "#cfoo",
            ReadError::InvalidNumber("expected '(' in complex literal".to_owned()),
        ),
        ("#\\", ReadError::InvalidCharacter),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected,
            "input: {text}"
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#18446744073709551616r10", &opts).unwrap_err(),
        ReadError::NumberOutOfRange
    );
    let lower = read_from_string(&mut ctx, &runtime, "#\\linefeed", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(lower.as_character(), Some(0x0a));
}

#[test]
fn numeric_markers_select_float_formats_and_reject_single_float() {
    let (runtime, mut ctx, mut opts) = setup();
    for marker in ['s', 'S', 'f', 'F'] {
        let text = format!("1.0{marker}2");
        assert_eq!(
            read_from_string(&mut ctx, &runtime, &text, &opts).unwrap_err(),
            ReadError::FloatFormatUnavailable(marker),
            "input: {text}"
        );
    }
    for marker in ['d', 'D', 'l', 'L'] {
        let text = format!("1.0{marker}2");
        let value = read_from_string(&mut ctx, &runtime, &text, &opts)
            .unwrap()
            .unwrap();
        assert!(matches!(
            classify_object(&ctx, value),
            ObjectRef::DoubleFloat(_)
        ));
    }
    opts.set_default_float_format(FloatFormat::DoubleFloat);
    let exponent = read_from_string(&mut ctx, &runtime, "1e2", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, exponent),
        ObjectRef::DoubleFloat(_)
    ));
}
