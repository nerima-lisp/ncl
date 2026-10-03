#![allow(
    clippy::unwrap_used,
    missing_docs,
    reason = "coverage tests assert on reader behavior"
)]

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify_object, make_cons, simple_vector_set,
};
use ncl_reader::{
    ReadError, ReadOptions, copy_readtable, get_dispatch_macro_character, get_macro_character,
    read_from_string, set_dispatch_macro_character, set_syntax_from_char,
};

fn setup() -> (Runtime, ThreadContext, ReadOptions) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let options = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    (runtime, ctx, options)
}

#[test]
fn readtable_mutations_preserve_macro_lookup_and_non_ascii_boundaries() {
    let (runtime, mut ctx, mut options) = setup();
    let table = copy_readtable(&mut ctx, &runtime, options.readtable()).unwrap();

    assert_eq!(
        get_macro_character(&mut ctx, table, '\u{100}').unwrap(),
        None
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', '\u{100}').unwrap(),
        None
    );
    set_syntax_from_char(&mut ctx, '\u{100}', '(', table, table).unwrap();
    set_syntax_from_char(&mut ctx, '!', '\u{100}', table, table).unwrap();

    let function = Word::fixnum(99);
    set_syntax_from_char(&mut ctx, '~', '(', table, table).unwrap();
    options.set_readtable(table);
    assert_eq!(get_macro_character(&mut ctx, table, '~').unwrap(), None);
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &options).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );
    set_syntax_from_char(&mut ctx, '~', 'a', table, table).unwrap();
    let symbol = read_from_string(&mut ctx, &runtime, "~", &options)
        .unwrap()
        .unwrap();
    assert!(matches!(
        classify_object(&ctx, symbol),
        ObjectRef::Symbol(_)
    ));

    set_dispatch_macro_character(&mut ctx, &runtime, table, '#', 'q', function).unwrap();
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '#', 'q').unwrap(),
        Some(function)
    );
    assert_eq!(
        get_dispatch_macro_character(&mut ctx, table, '!', 'q').unwrap_err(),
        ReadError::NotDispatchMacro('!')
    );
}

#[test]
fn malformed_readtable_entries_are_reported_without_panics() {
    let (runtime, mut ctx, options) = setup();
    let table = options.readtable();
    let syntax = table.syntax_table(&ctx).unwrap();
    simple_vector_set(&mut ctx, syntax, usize::from(b'~'), Word::fixnum(99)).unwrap();
    assert_eq!(
        read_from_string(&mut ctx, &runtime, "~", &options).unwrap_err(),
        ReadError::UninvocableMacroFunction('~')
    );

    let malformed = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    simple_vector_set(&mut ctx, syntax, usize::from(b'!'), malformed).unwrap();
    assert_eq!(
        get_macro_character(&mut ctx, table, '!').unwrap(),
        Some(Word::fixnum(1))
    );
}
