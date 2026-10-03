#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Tenth-wave coverage for remaining reader macro and dispatch branches.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify, classify_object, symbol_name,
};
use ncl_reader::{
    ReadError, ReadOptions, copy_readtable, get_dispatch_macro_character, get_macro_character,
    read_from_string, readtable_case, readtablep, set_syntax_from_char,
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
fn readtable_accessors_and_copy_keep_standard_entries() {
    let (runtime, mut ctx, opts) = setup();
    let original = opts.readtable();
    let copy = copy_readtable(&mut ctx, &runtime, original).unwrap();
    assert!(readtablep(&ctx, original.object().as_word()));
    assert!(readtablep(&ctx, copy.object().as_word()));
    assert!(!readtablep(&ctx, Word::fixnum(0)));
    assert_eq!(
        readtable_case(&ctx, original).unwrap(),
        ncl_reader::ReadtableCase::Upcase
    );
    assert_ne!(original.syntax_table(&ctx).unwrap(), Word::NIL);
    assert_ne!(original.dispatch_table(&ctx).unwrap(), Word::NIL);
    assert_eq!(get_macro_character(&mut ctx, copy, 'q').unwrap(), None);
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copy, '#', 'q').unwrap(),
        None
    );
}

#[test]
fn readtable_syntax_copy_changes_macro_behavior_only_in_copy() {
    let (runtime, mut ctx, opts) = setup();
    let original = opts.readtable();
    let copy = copy_readtable(&mut ctx, &runtime, original).unwrap();
    set_syntax_from_char(&mut ctx, '~', '(', copy, original).unwrap();
    assert_eq!(get_macro_character(&mut ctx, original, '~').unwrap(), None);
    let mut copy_opts = opts.clone();
    copy_opts.set_readtable(copy);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &copy_opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    let original_symbol = read_from_string(&mut ctx, &runtime, "~", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, original_symbol), "~");
}

#[test]
fn reader_macro_characters_build_their_expected_forms() {
    let (runtime, mut ctx, opts) = setup();
    for (text, operator) in [
        ("'x", "QUOTE"),
        ("`x", "QUASIQUOTE"),
        (",x", "UNQUOTE"),
        (",@x", "UNQUOTE-SPLICING"),
    ] {
        let form = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(symbol_text(&ctx, car(&ctx, form).unwrap()), operator);
        assert_eq!(
            symbol_text(&ctx, car(&ctx, cdr(&ctx, form).unwrap()).unwrap()),
            "X"
        );
    }
    let string = read_from_string(&mut ctx, &runtime, "\"hello\"", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, string),
        ObjectRef::String(_)
    ));
}

#[test]
fn reader_entry_points_handle_empty_input_comments_and_unmatched_macros() {
    let (runtime, mut ctx, opts) = setup();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "", &opts).unwrap(),
        None
    );
    assert_eq!(
        classify(
            read_from_string(&mut ctx, &runtime, "; comment\n9", &opts)
                .unwrap()
                .unwrap()
        ),
        ObjectRef::Fixnum(9)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, ")", &opts).unwrap_err(),
        ReadError::UnmatchedRightParen
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "'", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "\"unterminated", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
}

#[test]
fn sharp_dispatch_stops_at_non_digits_and_reports_eof_errors() {
    let (runtime, mut ctx, opts) = setup();
    let bits = read_from_string(&mut ctx, &runtime, "#*101x", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, bits),
        ObjectRef::SpecializedArray(_)
    ));
    let radix = read_from_string(&mut ctx, &runtime, "#2r-101", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(radix), ObjectRef::Fixnum(-5));
    for (text, expected) in [
        ("#", ReadError::UnexpectedEof),
        ("#'", ReadError::UnexpectedEof),
        ("#:", ReadError::UnexpectedEof),
        ("#\\", ReadError::InvalidCharacter),
        (
            "#1",
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
}

#[test]
fn token_package_errors_and_number_shapes_remain_distinct() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        (
            "NO-SUCH:ITEM",
            ReadError::PackageNotFound("NO-SUCH".to_owned()),
        ),
        ("A:B:C", ReadError::InvalidSymbolToken("A:B:C".to_owned())),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected,
            "input: {text}"
        );
    }
    let integer = read_from_string(&mut ctx, &runtime, "123.", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(integer), ObjectRef::Fixnum(123));
    let float = read_from_string(&mut ctx, &runtime, "1.0e-2", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, float),
        ObjectRef::DoubleFloat(_)
    ));
    let symbol = read_from_string(&mut ctx, &runtime, "1e+", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "1E+");
}
