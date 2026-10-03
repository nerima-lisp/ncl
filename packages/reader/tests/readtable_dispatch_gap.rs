#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Coverage for readtable mutation, dispatch boundaries, and error contracts.

use ncl_object::{
    ObjectError, ObjectRef, Runtime, ThreadContext, Word, classify_object, symbol_name,
};
use ncl_reader::{
    ReadError, ReadOptions, ReadtableCase, get_dispatch_macro_character, get_macro_character,
    make_dispatch_macro_character, read_from_string, set_dispatch_macro_character,
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

#[test]
fn mutates_and_copies_reader_macro_syntax() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();
    assert_eq!(get_macro_character(&mut ctx, table, 'q').unwrap(), None);
    assert_eq!(get_macro_character(&mut ctx, table, 'é').unwrap(), None);

    set_syntax_from_char(&mut ctx, '[', '(', table, table).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "[x", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('[')
    );
    set_syntax_from_char(&mut ctx, ']', 'é', table, table).unwrap();
    let word = read_from_string(&mut ctx, &runtime, "]", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, word), "]");
}

#[test]
fn mutates_dispatch_table_and_preserves_existing_entries() {
    let (runtime, mut ctx, _opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    let function = Word::fixnum(88);

    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'z').unwrap(),
        None
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '!', 'z').unwrap_err(),
        ReadError::NotDispatchMacro('!')
    );
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'z', function).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'z').unwrap(),
        Some(function)
    );
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'λ', function).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'λ').unwrap(),
        None
    );

    make_dispatch_macro_character(&mut ctx, &runtime, table, '#').unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'z').unwrap(),
        Some(function)
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'y').unwrap(),
        Some(Word::NIL)
    );
    assert_eq!(
        make_dispatch_macro_character(&mut ctx, &runtime, table, '!').unwrap_err(),
        ReadError::NotDispatchMacro('!')
    );
}

#[test]
fn readtable_predicates_and_case_modes_are_observable() {
    let (runtime, mut ctx, _opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    assert!(ncl_reader::readtablep(&ctx, table.object().as_word()));
    assert!(!ncl_reader::readtablep(&ctx, Word::fixnum(1)));
    assert_eq!(
        ncl_reader::readtable_case(&ctx, table).unwrap(),
        ReadtableCase::Upcase
    );
    assert_ne!(table.syntax_table(&ctx).unwrap(), Word::NIL);
    assert_ne!(table.dispatch_table(&ctx).unwrap(), Word::NIL);
}

#[test]
fn dispatch_reader_reports_uncovered_syntax_errors() {
    let (runtime, mut ctx, opts) = setup();
    for (text, expected) in [
        ("#", ReadError::UnexpectedEof),
        ("#'", ReadError::UnexpectedEof),
        ("#:", ReadError::UnexpectedEof),
        ("#\\", ReadError::InvalidCharacter),
        ("#a", ReadError::ArraySyntax),
        ("#p", ReadError::PathnameSyntax),
        (
            "#c",
            ReadError::InvalidNumber("expected '(' in complex literal".to_owned()),
        ),
        (
            "#1",
            ReadError::InvalidNumber(
                "expected '=', '#', or 'r' after a radix or label number".to_owned(),
            ),
        ),
        (
            "#1#",
            ReadError::InvalidNumber("undefined label #1".to_owned()),
        ),
        ("#+", ReadError::InvalidFeatureExpression),
    ] {
        assert_eq!(
            read_from_string(&mut ctx, &runtime, text, &opts).unwrap_err(),
            expected
        );
    }
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#999999999999999999999=1", &opts).unwrap_err(),
        ReadError::NumberOutOfRange
    );
    let complex = read_from_string(&mut ctx, &runtime, "#c(8 2)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, complex),
        ObjectRef::Complex(_)
    ));
}

#[test]
fn read_errors_have_stable_display_and_source_contracts() {
    let cases = [
        (ReadError::UnexpectedEof, "unexpected end of input"),
        (
            ReadError::UnmatchedRightParen,
            "unmatched right parenthesis",
        ),
        (
            ReadError::DotWithoutCdr,
            "dot in list is not followed by a cdr form",
        ),
        (
            ReadError::InvalidSymbolToken("X".to_owned()),
            "invalid symbol token: X",
        ),
        (
            ReadError::PackageNotFound("P".to_owned()),
            "package not found: P",
        ),
        (ReadError::InvalidBase(1), "invalid read base: 1"),
        (ReadError::InvalidDigit('z'), "invalid digit for base: z"),
        (
            ReadError::InvalidNumber("N".to_owned()),
            "invalid number token: N",
        ),
        (ReadError::NumberOutOfRange, "number out of range"),
        (
            ReadError::FloatFormatUnavailable('f'),
            "float format is unavailable: f",
        ),
        (
            ReadError::ReadEvalDisabled,
            "#. requires *read-eval* to be true",
        ),
        (
            ReadError::ReadEvalUnavailable,
            "#. has no evaluator available",
        ),
        (ReadError::InvalidCharacter, "invalid character literal"),
        (
            ReadError::UnknownCharacterName("X".to_owned()),
            "unknown character name: X",
        ),
        (
            ReadError::UndefinedDispatchMacro('x'),
            "undefined dispatch macro: #x",
        ),
        (
            ReadError::UninvocableMacroFunction('x'),
            "macro character x has no invocable function",
        ),
        (
            ReadError::InvalidFeatureExpression,
            "invalid feature expression",
        ),
        (ReadError::ArraySyntax, "array reader syntax is unavailable"),
        (
            ReadError::StructureSyntax,
            "invalid structure reader syntax",
        ),
        (
            ReadError::PathnameSyntax,
            "pathname reader syntax is unavailable",
        ),
        (
            ReadError::NotDispatchMacro('x'),
            "x is not a dispatch macro character",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert!(std::error::Error::source(&error).is_none());
    }
    let object = ReadError::from(ObjectError::TypeError);
    assert_eq!(object.to_string(), "object error: TypeError");
    assert!(std::error::Error::source(&object).is_some());
}
