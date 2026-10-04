#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Ninth-wave coverage for remaining reachable reader syntax boundaries.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify, classify_object, symbol_name};
use ncl_reader::{ReadError, ReadOptions, ReadtableCase, read_from_string, readtable_case};

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
fn malformed_readtable_case_covers_non_fixnum_and_unknown_fixnum() {
    let (_runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    for value in [Word::NIL, Word::fixnum(-1), Word::fixnum(4)] {
        ctx.write_object_slot(
            table.object().as_word(),
            ncl_object::readtable_offset::CASE,
            value,
        )
        .unwrap();
        assert!(matches!(
            readtable_case(&ctx, table),
            Err(ReadError::InvalidNumber(_))
        ));
    }
    ctx.write_object_slot(
        table.object().as_word(),
        ncl_object::readtable_offset::CASE,
        Word::fixnum(0),
    )
    .unwrap();
    assert_eq!(readtable_case(&ctx, table).unwrap(), ReadtableCase::Upcase);
}

#[test]
fn package_marker_shapes_accept_valid_forms_and_reject_invalid_colons() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("COMMON-LISP::", "COMMON-LISP"),
        ("COMMON-LISP::CAR", "CAR"),
    ] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        if text.ends_with("::") {
            assert!(matches!(classify_object(&ctx, word), ObjectRef::Package(_)));
        } else {
            assert_eq!(symbol_text(&ctx, word), expected);
        }
    }
    for text in ["::ITEM", "A:B:C", "A::B:C", "A:B::C"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            ReadError::InvalidSymbolToken(text.to_owned()),
            "input: {text}"
        );
    }
}

#[test]
fn character_aliases_and_semicolon_comments_follow_reader_boundaries() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("#\\escape", 0x1b),
        ("#\\rubout", 0x7f),
        ("#\\delete", 0x7f),
    ] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(word.as_character(), Some(expected));
    }
    let word = read_from_string(&mut ctx, &runtime, "; ignore this\n37", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(word), ObjectRef::Fixnum(37));
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#\\rubout-now", &opts).unwrap_err(),
        ReadError::UnknownCharacterName("rubout-now".to_owned())
    );
}

#[test]
fn feature_conditionals_select_and_skip_nested_forms() {
    let (runtime, mut ctx, opts) = setup();
    runtime.add_feature("WAVE9");
    let selected = read_from_string(
        &mut ctx,
        &runtime,
        "#+(or missing (and wave9 (not absent))) 71 72",
        &opts,
    )
    .unwrap()
    .unwrap();
    assert_eq!(classify(selected), ObjectRef::Fixnum(71));
    let skipped = read_from_string(&mut ctx, &runtime, "#-(or wave9 missing) 71 72", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(skipped), ObjectRef::Fixnum(72));
    assert_eq!(
        read_from_string(
            &mut ctx,
            &runtime,
            "#-wave9 missing-package::name 72",
            &opts
        )
        .unwrap()
        .unwrap(),
        Word::fixnum(72)
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+(and wave9)", &opts).unwrap(),
        None
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "(#+missing 1)", &opts).unwrap(),
        Some(Word::NIL)
    );
}

#[test]
fn labels_and_complex_literals_cover_valid_and_invalid_followers() {
    let (runtime, mut ctx, opts) = setup();
    let forms = read_from_string(&mut ctx, &runtime, "(#1=(alpha) #1#)", &opts)
        .unwrap()
        .unwrap();
    let first = ncl_object::car(&ctx, forms).unwrap();
    let rest = ncl_object::cdr(&ctx, forms).unwrap();
    let second = ncl_object::car(&ctx, rest).unwrap();
    assert_eq!(
        symbol_text(&ctx, ncl_object::car(&ctx, first).unwrap()),
        "ALPHA"
    );
    assert_eq!(first, second);
    let complex = read_from_string(&mut ctx, &runtime, "#c(8 2 99)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, complex),
        ObjectRef::Complex(_)
    ));
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#1#", &opts).unwrap_err(),
        ReadError::InvalidNumber("undefined label #1".to_owned())
    );
}

#[test]
fn number_boundaries_cover_radix_digits_and_non_numeric_tokens() {
    let (runtime, mut ctx, opts) = setup();
    let base36 = read_from_string(&mut ctx, &runtime, "#36rZ", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(base36), ObjectRef::Fixnum(35));
    let ratio = read_from_string(&mut ctx, &runtime, "12/0", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));
    for text in ["1e+", "1/", "+", "-"] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(symbol_text(&ctx, word), text.to_ascii_uppercase());
    }
}
