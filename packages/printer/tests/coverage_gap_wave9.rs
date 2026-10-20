#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Ninth-wave coverage for self-referential arrays and option/error branches.

use ncl_object::{
    ArrayElementType, ArrayOptions, ObjectError, Package, Runtime, ThreadContext, Word,
    array_row_major_set, make_array, make_bignum_from_i128, make_double, make_string,
    set_symbol_special, set_symbol_value,
};
use ncl_printer::{PrintError, PrintOptions, StringSink, write};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn render(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: PrintOptions,
) -> Result<String, PrintError> {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options)?;
    Ok(sink.into_string())
}

#[test]
fn circle_option_labels_a_self_referential_general_array() {
    let (runtime, mut ctx) = context();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    array_row_major_set(&mut ctx, array, 0, array).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, array, PrintOptions::new()),
        Err(PrintError::Circularity)
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_circle(true),
        ),
        Ok("#1=#(#1#)".to_string())
    );
}

#[test]
fn ambient_boolean_and_numeric_specials_use_fallbacks_for_other_values() {
    let (runtime, mut ctx) = context();
    let readable = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-READABLY*");
    let array = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ARRAY*");
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    let level = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LEVEL*");
    for symbol in [readable, array, base, level] {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    set_symbol_value(&mut ctx, readable, Word::fixnum(1)).unwrap();
    set_symbol_value(&mut ctx, array, Word::NIL).unwrap();
    let string = make_string(&mut ctx, &runtime, &['x']).unwrap();
    set_symbol_value(&mut ctx, base, string).unwrap();
    set_symbol_value(&mut ctx, level, Word::fixnum(-1)).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(options.readably());
    assert!(!options.array());
    assert_eq!(options.base().get(), 10);
    assert_eq!(options.level(), None);
}

#[test]
fn numbers_cover_small_float_and_negative_bignum_in_binary() {
    let (runtime, mut ctx) = context();
    let tiny = make_double(&mut ctx, &runtime, 1.0e-100).unwrap();
    let tiny_text = render(&runtime, &mut ctx, tiny.as_word(), PrintOptions::new()).unwrap();
    assert!(tiny_text.starts_with("0.000"), "{tiny_text}");
    let negative = make_bignum_from_i128(&mut ctx, &runtime, -255).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            negative.as_word(),
            PrintOptions::new().with_base(2).with_radix(true),
        ),
        Ok("#b-11111111".to_string())
    );
}

#[test]
fn print_error_from_unreadable_object_is_preserved() {
    let (runtime, mut ctx) = context();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[1],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(4),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_readably(true).with_array(false),
        ),
        Err(PrintError::NotReadable)
    );
}

#[test]
fn invalid_stream_is_reported_by_print_builtin_without_output() {
    let (runtime, mut ctx) = context();
    let text = make_string(&mut ctx, &runtime, &['z']).unwrap();
    let print = ncl_object::FunctionObject::try_from(
        runtime.function(&mut ctx, "COMMON-LISP", "PRINT").unwrap(),
    )
    .unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[text, Word::fixnum(99)]),
        Err(ObjectError::TypeError)
    );
}
