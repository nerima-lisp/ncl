#![allow(clippy::unwrap_used, reason = "tests assert on reader behavior")]

//! Reader regressions for tokens that resemble incomplete numbers.

use ncl_object::{Runtime, ThreadContext, Word, string_length, string_ref, symbol_name};
use ncl_reader::{ReadOptions, ReadSuppression, read_from_string};

fn read_name(ctx: &mut ThreadContext, runtime: &Runtime, text: &str) -> String {
    let options = ReadOptions::standard(ctx, runtime).unwrap();
    let word = read_from_string(ctx, runtime, text, &options)
        .unwrap()
        .unwrap();
    let name = symbol_name(ctx, word).unwrap();
    (0..string_length(ctx, name).unwrap())
        .map(|index| string_ref(ctx, name, index).unwrap())
        .collect()
}

#[test]
fn integer_with_bare_exponent_marker_is_a_symbol() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    assert_eq!(read_name(&mut ctx, &runtime, "456d"), "456D");
}

#[test]
fn read_suppress_discards_integer_with_bare_exponent_marker() {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let mut options = ReadOptions::standard(&mut ctx, &runtime).unwrap();
    options.set_read_suppression(ReadSuppression::Discard);

    assert_eq!(
        read_from_string(&mut ctx, &runtime, "456d", &options).unwrap(),
        Some(Word::NIL)
    );
}
