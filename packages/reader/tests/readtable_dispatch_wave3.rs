#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Reachable readtable and dispatch coverage beyond the first two waves.

use ncl_object::{ObjectRef, Runtime, ThreadContext, Word, classify_object, symbol_name};
use ncl_reader::{
    ReadError, ReadOptions, copy_readtable, get_dispatch_macro_character, read_from_string,
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
fn copies_readtable_entries_and_handles_non_bmp_boundaries() {
    let (runtime, mut ctx, opts) = setup();
    let original = opts.readtable();
    let copy = copy_readtable(&mut ctx, &runtime, original).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copy, '#', 'q').unwrap(),
        get_dispatch_macro_character(&mut ctx, original, '#', 'q').unwrap()
    );

    set_syntax_from_char(&mut ctx, 'λ', '(', copy, original).unwrap();
    set_dispatch_macro_character(&mut ctx, &runtime, copy, '#', 'λ', Word::fixnum(9)).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, copy, '#', 'λ').unwrap(),
        None
    );
    let symbol = read_from_string(&mut ctx, &runtime, "λ", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "λ");
}

#[test]
fn reads_empty_vectors_and_bit_vectors_with_exact_object_kinds() {
    let (runtime, mut ctx, opts) = setup();
    let vector = read_from_string(&mut ctx, &runtime, "#()", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, vector),
        ObjectRef::SimpleVector(_)
    ));
    let bits = read_from_string(&mut ctx, &runtime, "#*", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, bits),
        ObjectRef::SpecializedArray(_)
    ));
    let bits = read_from_string(&mut ctx, &runtime, "#*10101 rest", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, bits),
        ObjectRef::SpecializedArray(_)
    ));
}

#[test]
fn covers_character_aliases_and_signed_radix_literals() {
    let (runtime, mut ctx, opts) = setup();
    for (text, code) in [
        ("#\\space", 0x20),
        ("#\\linefeed", 0x0a),
        ("#\\return", 0x0d),
        ("#\\page", 0x0c),
        ("#\\backspace", 0x08),
        ("#\\delete", 0x7f),
        ("#\\null", 0x00),
        ("#\\altmode", 0x1b),
        ("#\\Z", u32::from('Z')),
    ] {
        let word = read_from_string(&mut ctx, &runtime, text, &opts)
            .unwrap()
            .unwrap();
        assert_eq!(word.as_character(), Some(code));
    }
    let hexadecimal = read_from_string(&mut ctx, &runtime, "#x-ff", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify_object(&ctx, hexadecimal), ObjectRef::Fixnum(-255));
    let binary = read_from_string(&mut ctx, &runtime, "#b-101", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify_object(&ctx, binary), ObjectRef::Fixnum(-5));
}

#[test]
fn covers_complex_default_imaginary_part_and_dispatch_number_edges() {
    let (runtime, mut ctx, opts) = setup();
    let complex = read_from_string(&mut ctx, &runtime, "#c(8)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, complex),
        ObjectRef::Complex(_)
    ));
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#=1", &opts).unwrap_err(),
        ReadError::UndefinedDispatchMacro('=')
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#1=", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "(#1=one #2#)", &opts).unwrap_err(),
        ReadError::InvalidNumber("undefined label #2".to_owned())
    );
}

#[test]
fn rejects_malformed_structure_literals_at_each_reader_boundary() {
    let (runtime, mut ctx, opts) = setup();
    for text in ["#S", "#S()", "#S(8)", "#S(UNKNOWN)", "#S(POINT :UNKNOWN 1)"] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            ReadError::StructureSyntax,
            "input: {text}"
        );
    }
}

#[test]
fn rejects_complex_and_feature_forms_without_required_followers() {
    let (runtime, mut ctx, opts) = setup();
    let empty_complex = read_from_string(&mut ctx, &runtime, "#c()", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, empty_complex),
        ObjectRef::Complex(_)
    ));
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+alpha", &opts).unwrap(),
        None
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#-alpha", &opts).unwrap(),
        None
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#+(not)", &opts).unwrap_err(),
        ReadError::Object(ncl_object::ObjectError::TypeError)
    );
}
