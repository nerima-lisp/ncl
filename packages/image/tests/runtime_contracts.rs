#![allow(
    clippy::expect_used,
    missing_docs,
    reason = "tests assert on image runtime contracts"
)]

use ncl_image::{load, save};
use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_string, set_symbol_value, string_length,
    string_ref, symbol_name, symbol_value,
};

#[test]
fn unicode_string_round_trip_preserves_character_sequence() {
    let runtime = Runtime::new().expect("source runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("source thread registration");
    let value = make_string(&mut ctx, &runtime, &['A', '界', '🦀', 'Z'])
        .expect("unicode string");

    let bytes = save(&runtime, &mut ctx, &[value], &[]).expect("save unicode string");

    let destination = Runtime::new().expect("destination runtime");
    let mut destination_ctx = ThreadContext::new();
    destination_ctx
        .register(&destination)
        .expect("destination thread registration");
    let loaded = load(&bytes, &destination, &mut destination_ctx).expect("load unicode string");
    let loaded_value = loaded.roots[0];

    assert_eq!(
        string_length(&destination_ctx, loaded_value).expect("string length"),
        4
    );
    let characters: String = (0..4)
        .map(|index| string_ref(&destination_ctx, loaded_value, index).expect("string character"))
        .collect();
    assert_eq!(characters, "A界🦀Z");
}

#[test]
fn loading_the_same_symbol_twice_reuses_interned_identity() {
    let runtime = Runtime::new().expect("source runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("source thread registration");
    let package = runtime.find_package(&ctx, "NCL").expect("NCL package");
    let (symbol, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "IMAGE-RELOAD-ID")
        .expect("intern source symbol");
    set_symbol_value(&mut ctx, symbol, Word::fixnum(73)).expect("set source value");
    let bytes = save(&runtime, &mut ctx, &[symbol], &[]).expect("save symbol");

    let destination = Runtime::new().expect("destination runtime");
    let mut destination_ctx = ThreadContext::new();
    destination_ctx
        .register(&destination)
        .expect("destination thread registration");
    let first = load(&bytes, &destination, &mut destination_ctx)
        .expect("first symbol load")
        .roots[0];
    let second = load(&bytes, &destination, &mut destination_ctx)
        .expect("second symbol load")
        .roots[0];

    assert_eq!(first, second);
    assert_eq!(
        string_length(
            &destination_ctx,
            symbol_name(&destination_ctx, first).expect("symbol name"),
        )
        .expect("name length"),
        15
    );
    assert_eq!(
        symbol_value(&destination_ctx, second).expect("symbol value"),
        Word::fixnum(73)
    );
}
