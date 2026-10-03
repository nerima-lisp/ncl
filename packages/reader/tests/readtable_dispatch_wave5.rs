#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Fifth-wave coverage for reachable readtable, reader, dispatch, and number paths.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, car, cdr, classify, classify_object,
    simple_vector_length, simple_vector_ref, symbol_name,
};
use ncl_reader::{
    ReadBase, ReadError, ReadOptions, StringSource, get_dispatch_macro_character,
    get_macro_character, make_dispatch_macro_character, read, read_delimited_list,
    read_from_string, read_preserving_whitespace, set_dispatch_macro_character,
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
fn readtable_mutations_cover_boundaries_and_removal() {
    let (runtime, mut ctx, opts) = setup();
    let table = opts.readtable();

    assert_eq!(get_macro_character(&mut ctx, table, 'q').unwrap(), None);
    set_syntax_from_char(&mut ctx, 'q', '(', table, table).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "q", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction('q')
    );
    set_syntax_from_char(&mut ctx, 'q', 'a', table, table).unwrap();
    assert_eq!(get_macro_character(&mut ctx, table, 'q').unwrap(), None);
    set_syntax_from_char(&mut ctx, 'λ', '(', table, table).unwrap();
    assert_eq!(get_macro_character(&mut ctx, table, 'λ').unwrap(), None);

    set_syntax_from_char(&mut ctx, ']', '(', table, table).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "]", &opts).unwrap_err(),
        ReadError::UninvocableMacroFunction(']')
    );
    set_syntax_from_char(&mut ctx, ']', 'a', table, table).unwrap();
    assert_eq!(get_macro_character(&mut ctx, table, ']').unwrap(), None);
}

#[test]
fn dispatch_table_handles_installation_and_non_ascii_noops() {
    let (runtime, mut ctx, _opts) = setup();
    let table = standard_readtable(&mut ctx, &runtime).unwrap();
    let function = Word::fixnum(88);

    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', function).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(function)
    );
    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'λ', function).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'λ').unwrap(),
        None
    );
    assert_eq!(
        set_dispatch_macro_character(&mut ctx, &runtime, table, '!', 'q', function).unwrap_err(),
        ReadError::NotDispatchMacro('!')
    );

    make_dispatch_macro_character(&mut ctx, &runtime, table, '#').unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(function)
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'z').unwrap(),
        Some(Word::NIL)
    );
}

#[test]
fn reader_entry_points_preserve_source_and_build_dotted_lists() {
    let (runtime, mut ctx, opts) = setup();
    let mut source = StringSource::new("42  7");
    let first = read_preserving_whitespace(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(first), ObjectRef::Fixnum(42));
    let second = read(&mut ctx, &runtime, &mut source, &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(second), ObjectRef::Fixnum(7));

    let mut source = StringSource::new("a . b)");
    let list = read_delimited_list(&mut ctx, &runtime, &mut source, &opts).unwrap();
    assert_eq!(symbol_text(&ctx, car(&ctx, list).unwrap()), "A");
    assert_eq!(symbol_text(&ctx, cdr(&ctx, list).unwrap()), "B");

    let quoted = read_from_string(&mut ctx, &runtime, "`(a ,b ,@c)", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, quoted), ObjectRef::Cons(_)));
}

#[test]
fn dispatch_nested_comments_vectors_and_character_errors_are_observable() {
    let (runtime, mut ctx, opts) = setup();
    let value = read_from_string(&mut ctx, &runtime, "#| outer #| inner |# done |# 19", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(19));

    let vector = read_from_string(&mut ctx, &runtime, "#(1 foo 3)", &opts)
        .unwrap()
        .unwrap();
    let length = simple_vector_length(&ctx, vector).unwrap();
    assert_eq!(length, 3);
    assert_eq!(
        classify(simple_vector_ref(&ctx, vector, 0).unwrap()),
        ObjectRef::Fixnum(1)
    );
    assert_eq!(
        symbol_text(&ctx, simple_vector_ref(&ctx, vector, 1).unwrap()),
        "FOO"
    );
    assert_eq!(
        classify(simple_vector_ref(&ctx, vector, 2).unwrap()),
        ObjectRef::Fixnum(3)
    );

    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#\\NoSuchCharacter", &opts).unwrap_err(),
        ReadError::UnknownCharacterName("NoSuchCharacter".to_owned())
    );
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "#| unterminated", &opts).unwrap_err(),
        ReadError::UnexpectedEof
    );
}

#[test]
fn configured_base_and_number_shapes_distinguish_symbols_and_numbers() {
    let (runtime, mut ctx, mut opts) = setup();
    opts.set_read_base(ReadBase::new(16).unwrap());
    let value = read_from_string(&mut ctx, &runtime, "dead", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(classify(value), ObjectRef::Fixnum(0xdead));

    opts.set_read_base(ReadBase::new(10).unwrap());
    let bignum = read_from_string(&mut ctx, &runtime, "4611686018427387904", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, bignum),
        ObjectRef::Bignum(_)
    ));
    let ratio = read_from_string(&mut ctx, &runtime, "7/-3", &opts)
        .unwrap()
        .unwrap();
    assert!(matches!(classify_object(&ctx, ratio), ObjectRef::Ratio(_)));

    let symbol = read_from_string(&mut ctx, &runtime, "1e+", &opts)
        .unwrap()
        .unwrap();
    assert_eq!(symbol_text(&ctx, symbol), "1E+");
    let malformed = read_from_string(&mut ctx, &runtime, "1.2.3", &opts).unwrap();
    assert_eq!(symbol_text(&ctx, malformed.unwrap()), "1.2.3");
}
