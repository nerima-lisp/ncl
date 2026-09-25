#![allow(clippy::unwrap_used, reason = "tests assert on printer output")]

//! Printing tests for each object kind and the `*print-*` options.
//!
//! `ncl-reader` is not on `main` yet, so readable output is checked against
//! fixed expected strings rather than by reading it back. The round-trip test
//! lands when L2 does.

use ncl_object::{
    ArrayElementType, ArrayOptions, Package, Runtime, ThreadContext, Word, make_array,
    make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio, make_simple_vector,
    make_specialized_array, make_string, make_symbol, rplacd, set_symbol_special, set_symbol_value,
};
use ncl_printer::{PrintCase, PrintError, PrintOptions, StringSink, write};

fn print(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: &PrintOptions,
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, options).unwrap();
    sink.into_string()
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.find_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn list(runtime: &Runtime, ctx: &mut ThreadContext, values: &[Word]) -> Word {
    let mut result = Word::NIL;
    for value in values.iter().rev() {
        result = make_cons(ctx, runtime, *value, result).unwrap();
    }
    result
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<char>>()).unwrap()
}

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn prints_fixnums_in_every_base() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    assert_eq!(
        print(&runtime, &mut ctx, Word::fixnum(255), &options),
        "255"
    );
    assert_eq!(
        print(&runtime, &mut ctx, Word::fixnum(-10), &options),
        "-10"
    );
    assert_eq!(
        print(&runtime, &mut ctx, Word::fixnum(255), &options.with_base(2)),
        "11111111"
    );
    assert_eq!(
        print(&runtime, &mut ctx, Word::fixnum(255), &options.with_base(8)),
        "377"
    );
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            Word::fixnum(255),
            &options.with_base(16)
        ),
        "FF"
    );
}

#[test]
fn prints_radix_prefixes() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            Word::fixnum(255),
            &options.with_base(16).with_radix(true)
        ),
        "#xFF"
    );
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            Word::fixnum(255),
            &options.with_base(2).with_radix(true)
        ),
        "#b11111111"
    );
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            Word::fixnum(255),
            &options.with_base(10).with_radix(true)
        ),
        "255."
    );
}

#[test]
fn prints_bignums_ratios_floats_and_complexes() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let big =
        make_bignum_from_i128(&mut ctx, &runtime, 123_456_789_012_345_678_901_234_567_890).unwrap();
    assert_eq!(
        print(&runtime, &mut ctx, big.as_word(), &options),
        "123456789012345678901234567890"
    );
    let negative = make_bignum_from_i128(&mut ctx, &runtime, -9_876_543_210_987_654_321).unwrap();
    assert_eq!(
        print(&runtime, &mut ctx, negative.as_word(), &options),
        "-9876543210987654321"
    );
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    assert_eq!(print(&runtime, &mut ctx, ratio.as_word(), &options), "1/2");
    let double = make_double(&mut ctx, &runtime, 1.5).unwrap();
    assert_eq!(print(&runtime, &mut ctx, double.as_word(), &options), "1.5");
    let whole = make_double(&mut ctx, &runtime, 2.0).unwrap();
    assert_eq!(print(&runtime, &mut ctx, whole.as_word(), &options), "2.0");
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(2)).unwrap();
    assert_eq!(
        print(&runtime, &mut ctx, complex.as_word(), &options),
        "#C(1 2)"
    );
}

#[test]
fn prints_characters_and_strings() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let upper_a = Word::character(u32::from('A'));
    assert_eq!(print(&runtime, &mut ctx, upper_a, &options), "#\\A");
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            Word::character(u32::from(' ')),
            &options
        ),
        "#\\Space"
    );
    assert_eq!(
        print(&runtime, &mut ctx, upper_a, &options.with_escape(false)),
        "A"
    );
    let text = string(&runtime, &mut ctx, "hi");
    assert_eq!(print(&runtime, &mut ctx, text, &options), "\"hi\"");
    assert_eq!(
        print(&runtime, &mut ctx, text, &options.with_escape(false)),
        "hi"
    );
    let quoted = string(&runtime, &mut ctx, "a\"b");
    assert_eq!(print(&runtime, &mut ctx, quoted, &options), "\"a\\\"b\"");
}

#[test]
fn prints_symbols_with_package_prefixes() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let car = intern(&runtime, &mut ctx, "COMMON-LISP", "CAR");
    assert_eq!(print(&runtime, &mut ctx, car, &options), "CAR");
    let foo = intern(&runtime, &mut ctx, "COMMON-LISP-USER", "FOO");
    assert_eq!(
        print(&runtime, &mut ctx, foo, &options),
        "COMMON-LISP-USER:FOO"
    );
    let keyword = intern(&runtime, &mut ctx, "KEYWORD", "BAR");
    assert_eq!(print(&runtime, &mut ctx, keyword, &options), ":BAR");
}

#[test]
fn prints_symbol_names_with_escaping_and_case() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let lower = intern(&runtime, &mut ctx, "COMMON-LISP-USER", "lower");
    assert_eq!(
        print(&runtime, &mut ctx, lower, &options),
        "COMMON-LISP-USER:|lower|"
    );
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            lower,
            &options.with_case(PrintCase::Downcase)
        ),
        "COMMON-LISP-USER:lower"
    );
    let name = string(&runtime, &mut ctx, "UNINTERNED");
    let uninterned = make_symbol(&mut ctx, &runtime, name).unwrap();
    assert_eq!(
        print(&runtime, &mut ctx, uninterned, &options),
        "#:UNINTERNED"
    );
    assert_eq!(
        print(&runtime, &mut ctx, uninterned, &options.with_gensym(false)),
        "UNINTERNED"
    );
}

#[test]
fn prints_cons_forms() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let one = Word::fixnum(1);
    let proper = list(&runtime, &mut ctx, &[one, Word::fixnum(2), Word::fixnum(3)]);
    assert_eq!(print(&runtime, &mut ctx, proper, &options), "(1 2 3)");
    let dotted = make_cons(&mut ctx, &runtime, one, Word::fixnum(2)).unwrap();
    assert_eq!(print(&runtime, &mut ctx, dotted, &options), "(1 . 2)");
    assert_eq!(
        print(&runtime, &mut ctx, proper, &options.with_length(Some(2))),
        "(1 2 ...)"
    );
    assert_eq!(
        print(&runtime, &mut ctx, proper, &options.with_length(Some(0))),
        "(...)"
    );
}

#[test]
fn prints_quote_abbreviations() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let quote = intern(&runtime, &mut ctx, "COMMON-LISP", "QUOTE");
    let quoted = list(&runtime, &mut ctx, &[quote, Word::fixnum(1)]);
    assert_eq!(print(&runtime, &mut ctx, quoted, &options), "'1");
    let function = intern(&runtime, &mut ctx, "COMMON-LISP", "FUNCTION");
    let named = list(&runtime, &mut ctx, &[function, Word::fixnum(1)]);
    assert_eq!(print(&runtime, &mut ctx, named, &options), "#'1");
}

#[test]
fn print_level_truncates_nesting() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let inner = list(&runtime, &mut ctx, &[Word::fixnum(1)]);
    let outer = list(&runtime, &mut ctx, &[inner]);
    assert_eq!(
        print(&runtime, &mut ctx, outer, &options.with_level(Some(1))),
        "(#)"
    );
    assert_eq!(
        print(&runtime, &mut ctx, outer, &options.with_level(Some(2))),
        "((#))"
    );
}

#[test]
fn prints_vectors() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let vector =
        make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1), Word::fixnum(2)]).unwrap();
    assert_eq!(print(&runtime, &mut ctx, vector, &options), "#(1 2)");
    let opaque = print(&runtime, &mut ctx, vector, &options.with_array(false));
    assert!(opaque.starts_with("#<VECTOR "), "{opaque}");
}

#[test]
fn prints_specialized_arrays() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let array = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(1), Word::fixnum(2)],
    )
    .unwrap();
    assert_eq!(print(&runtime, &mut ctx, array, &options), "#(1 2)");
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            array,
            &options.with_vector_length(Some(1))
        ),
        "#(1 ...)"
    );
}

#[test]
fn prints_non_simple_arrays_by_rank() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(0),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert_eq!(print(&runtime, &mut ctx, array, &options), "#2A(0 0 0 0)");
}

#[test]
fn print_circle_labels_shared_structure() {
    let (runtime, mut ctx) = context();
    let shared = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let outer = list(&runtime, &mut ctx, &[shared, shared]);
    let options = PrintOptions::new().with_circle(true);
    assert_eq!(print(&runtime, &mut ctx, outer, &options), "(#1=(1) #1#)");
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            outer,
            &PrintOptions::new()
                .with_circle(true)
                .with_circle_not_shared(true)
        ),
        "((1) (1))"
    );
}

#[test]
fn print_circle_labels_cycles() {
    let (runtime, mut ctx) = context();
    let node = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    rplacd(&mut ctx, node, node).unwrap();
    let options = PrintOptions::new().with_circle(true);
    assert_eq!(print(&runtime, &mut ctx, node, &options), "#1=(1 . #1#)");
}

#[test]
fn a_cycle_without_print_circle_is_an_error() {
    let (runtime, mut ctx) = context();
    let node = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    rplacd(&mut ctx, node, node).unwrap();
    let mut sink = StringSink::new();
    let error = write(&mut ctx, &runtime, node, &mut sink, &PrintOptions::new()).unwrap_err();
    assert_eq!(error, PrintError::Circularity);
}

#[test]
fn readable_output_is_reader_syntax() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new().with_readably(true);
    assert_eq!(print(&runtime, &mut ctx, Word::fixnum(42), &options), "42");
    assert_eq!(
        print(
            &runtime,
            &mut ctx,
            Word::character(u32::from('A')),
            &options
        ),
        "#\\A"
    );
    let text = string(&runtime, &mut ctx, "hi");
    assert_eq!(print(&runtime, &mut ctx, text, &options), "\"hi\"");
    let list = list(&runtime, &mut ctx, &[Word::fixnum(1), Word::fixnum(2)]);
    assert_eq!(print(&runtime, &mut ctx, list, &options), "(1 2)");
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    assert_eq!(print(&runtime, &mut ctx, vector, &options), "#(1)");
}

#[test]
fn unreadable_objects_are_rejected_when_readable() {
    let (runtime, mut ctx) = context();
    let options = PrintOptions::new().with_readably(true);
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let mut sink = StringSink::new();
    let error = write(
        &mut ctx,
        &runtime,
        vector,
        &mut sink,
        &options.with_array(false),
    )
    .unwrap_err();
    assert_eq!(error, PrintError::NotReadable);
}

#[test]
fn register_marks_owned_variables_special() {
    let (runtime, mut ctx) = context();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    let readably = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-READABLY*");
    assert!(ncl_object::symbol_is_special(&ctx, readably).unwrap());
    let dispatch = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-PPRINT-DISPATCH*");
    assert!(ncl_object::symbol_is_special(&ctx, dispatch).unwrap());
    assert_ne!(ncl_object::symbol_value(&ctx, dispatch).unwrap(), Word::NIL);
}

#[test]
fn options_come_from_the_ambient_variables() {
    let (runtime, mut ctx) = context();
    let escape = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ESCAPE*");
    set_symbol_special(&mut ctx, escape, true).unwrap();
    set_symbol_value(&mut ctx, escape, Word::NIL).unwrap();
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    set_symbol_special(&mut ctx, base, true).unwrap();
    set_symbol_value(&mut ctx, base, Word::fixnum(16)).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(!options.escape());
    assert_eq!(options.base().get(), 16);
}

#[test]
fn dispatch_table_survives_gc_stress() {
    let (runtime, mut ctx) = context();
    ctx.set_gc_stress(true);
    let table = ncl_printer::set_pprint_dispatch(
        &mut ctx,
        &runtime,
        Word::TRUE,
        Word::fixnum(7),
        Word::NIL,
    )
    .unwrap();
    let copied = ncl_printer::copy_pprint_dispatch(&mut ctx, &runtime, table).unwrap();
    let found = ncl_printer::pprint_dispatch(&mut ctx, Word::fixnum(9), copied).unwrap();
    assert_eq!(found, Word::fixnum(7));
}
