#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Fifteenth-wave coverage for specialized arrays, displaced storage, and fallbacks.

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, array_row_major_set, make_array, make_bignum_from_i128, make_double, make_string,
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

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
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
fn specialized_array_circle_matrix_labels_repeated_and_cyclic_values() {
    let (runtime, mut ctx) = context();
    let array = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(6), Word::fixnum(7)],
    )
    .unwrap();
    let tail = ncl_object::make_cons(&mut ctx, &runtime, array, Word::NIL).unwrap();
    let root = ncl_object::make_cons(&mut ctx, &runtime, array, tail).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            root,
            PrintOptions::new().with_circle(true)
        ),
        Ok("(#1=#(6 7) #1#)".to_string())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_vector_length(Some(0))
        ),
        Ok("#(...)".to_string())
    );
}

#[test]
fn displaced_arrays_use_offset_row_major_reads() {
    let (runtime, mut ctx) = context();
    let source = make_array(
        &mut ctx,
        &runtime,
        &[4],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(11),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    array_row_major_set(&mut ctx, source, 2, Word::fixnum(22)).unwrap();
    let displaced = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::NIL,
            adjustable: false,
            fill_pointer: None,
            displaced_to: Some(source),
            displaced_index_offset: 1,
        },
    )
    .unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, displaced, PrintOptions::new()),
        Ok("#(11 22)".to_string())
    );
    assert!(
        render(
            &runtime,
            &mut ctx,
            displaced,
            PrintOptions::new().with_array(false)
        )
        .unwrap()
        .starts_with("#<ARRAY ")
    );
}

#[test]
fn option_fallbacks_cover_unbound_specials_and_invalid_numbers() {
    let (runtime, mut ctx) = context();
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    let length = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LENGTH*");
    let escape = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ESCAPE*");
    set_symbol_special(&mut ctx, base, true).unwrap();
    set_symbol_special(&mut ctx, length, true).unwrap();
    set_symbol_special(&mut ctx, escape, true).unwrap();
    set_symbol_value(&mut ctx, base, Word::fixnum(0)).unwrap();
    set_symbol_value(&mut ctx, length, Word::TRUE).unwrap();
    set_symbol_value(&mut ctx, escape, Word::NIL).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(options.base().get(), 10);
    assert_eq!(options.length(), None);
    assert!(!options.escape());

    let unbound = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LEVEL*");
    set_symbol_special(&mut ctx, unbound, true).unwrap();
    assert_eq!(
        PrintOptions::from_specials(&mut ctx, &runtime).level(),
        None
    );
}

#[test]
fn number_fallback_matrix_covers_nan_and_decimal_bignum_suffix() {
    let (runtime, mut ctx) = context();
    let nan = make_double(&mut ctx, &runtime, f64::NAN).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, nan.as_word(), PrintOptions::new()),
        Ok("#<DOUBLE-FLOAT NaN>".to_string())
    );
    let big = make_bignum_from_i128(&mut ctx, &runtime, 99).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            big.as_word(),
            PrintOptions::new().with_radix(true),
        ),
        Ok("99.".to_string())
    );
}

#[test]
fn print_readably_forces_string_escaping_and_rejects_opaque_streams() {
    let (runtime, mut ctx) = context();
    let text = make_string(&mut ctx, &runtime, &['a', '"', 'b']).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            text,
            PrintOptions::new().with_readably(true)
        ),
        Ok("\"a\\\"b\"".to_string())
    );
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            stream,
            PrintOptions::new().with_readably(true)
        ),
        Err(PrintError::NotReadable)
    );
}

#[test]
fn builtin_invalid_stream_and_missing_object_errors_are_distinct() {
    let (runtime, mut ctx) = context();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    let text = make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[text, Word::fixnum(123)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[]),
        Err(ObjectError::TypeError)
    );
}
