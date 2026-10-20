#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Thirteenth-wave matrix coverage for object kinds and print options.

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_bignum_from_i128, make_complex, make_cons, make_double, make_ratio,
    make_simple_vector, make_string, set_symbol_special, set_symbol_value,
};
use ncl_printer::{
    PrintError, PrintOptions, StringSink, copy_pprint_dispatch, pprint_dispatch, write,
};

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
fn opaque_object_matrix_covers_function_package_stream_and_array_modes() {
    let (runtime, mut ctx) = context();
    let function = builtin(&runtime, &mut ctx, "PRINC");
    let package = runtime.ensure_package(&mut ctx, "W13-PACKAGE").unwrap();
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let options = PrintOptions::new();
    for (object, prefix) in [
        (function.as_word(), "#<FUNCTION "),
        (package, "#<PACKAGE "),
        (stream, "#<STREAM "),
    ] {
        let output = render(&runtime, &mut ctx, object, options).unwrap();
        assert!(
            output.starts_with(prefix),
            "expected {prefix}, got {output}"
        );
    }
    assert!(
        render(&runtime, &mut ctx, vector, options.with_array(false))
            .unwrap()
            .starts_with("#<VECTOR ")
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            function.as_word(),
            options.with_readably(true),
        ),
        Err(PrintError::NotReadable)
    );
}

#[test]
fn options_matrix_reads_boolean_numeric_and_invalid_special_values() {
    let (runtime, mut ctx) = context();
    let names = [
        "*PRINT-ESCAPE*",
        "*PRINT-READABLY*",
        "*PRINT-RADIX*",
        "*PRINT-CIRCLE*",
        "*PRINT-PRETTY*",
        "*PRINT-ARRAY*",
        "*PRINT-GENSYM*",
        "*PRINT-BASE*",
        "*PRINT-LENGTH*",
        "*PRINT-LEVEL*",
    ];
    let symbols: Vec<_> = names
        .iter()
        .map(|name| intern(&runtime, &mut ctx, "COMMON-LISP", name))
        .collect();
    for symbol in symbols.iter().copied() {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    let values = [
        Word::NIL,
        Word::TRUE,
        Word::NIL,
        Word::TRUE,
        Word::NIL,
        Word::TRUE,
        Word::TRUE,
        Word::fixnum(16),
        Word::fixnum(-1),
        Word::fixnum(0),
    ];
    for (symbol, value) in symbols.iter().copied().zip(values) {
        set_symbol_value(&mut ctx, symbol, value).unwrap();
    }
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(!options.escape());
    assert!(options.readably());
    assert!(!options.radix());
    assert!(options.circle());
    assert!(!options.pretty());
    assert!(options.array());
    assert!(options.gensym());
    assert_eq!(options.base().get(), 16);
    assert_eq!(options.length(), None);
    assert_eq!(options.level().map(ncl_printer::NonNegative::get), Some(0));
}

#[test]
fn numeric_matrix_covers_zero_sign_float_ratio_and_complex_paths() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        render(&runtime, &mut ctx, Word::fixnum(0), PrintOptions::new()),
        Ok("0".to_string())
    );
    let negative_zero = make_double(&mut ctx, &runtime, -0.0).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            negative_zero.as_word(),
            PrintOptions::new()
        ),
        Ok("-0.0".to_string())
    );
    let bignum = make_bignum_from_i128(&mut ctx, &runtime, -1024).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            bignum.as_word(),
            PrintOptions::new().with_base(2)
        ),
        Ok("-10000000000".to_string())
    );
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(7), Word::fixnum(-3)).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, ratio.as_word(), PrintOptions::new()),
        Ok("7/-3".to_string())
    );
    let complex = make_complex(&mut ctx, &runtime, Word::fixnum(1), ratio.as_word()).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, complex.as_word(), PrintOptions::new()),
        Ok("#C(1 7/-3)".to_string())
    );
}

#[test]
fn array_matrix_covers_rank_one_rank_two_specialized_and_opaque_results() {
    let (runtime, mut ctx) = context();
    let vector = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(2),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let matrix = make_array(
        &mut ctx,
        &runtime,
        &[1, 2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(3),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let specialized = ncl_object::make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(4), Word::fixnum(5)],
    )
    .unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, vector, PrintOptions::new()),
        Ok("#(2 2)".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, matrix, PrintOptions::new()),
        Ok("#2A(3 3)".to_string())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            specialized,
            PrintOptions::new().with_vector_length(Some(1)),
        ),
        Ok("#(4 ...)".to_string())
    );
    assert!(
        render(
            &runtime,
            &mut ctx,
            matrix,
            PrintOptions::new().with_array(false)
        )
        .unwrap()
        .starts_with("#<ARRAY ")
    );
}

#[test]
fn print_matrix_covers_nil_true_level_and_pretty_separator() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        render(&runtime, &mut ctx, Word::NIL, PrintOptions::new()),
        Ok("NIL".into())
    );
    assert_eq!(
        render(&runtime, &mut ctx, Word::TRUE, PrintOptions::new()),
        Ok("T".into())
    );
    let inner = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, inner, Word::NIL).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new().with_level(Some(1))
        ),
        Ok("(#)".to_string())
    );
    let long = make_string(
        &mut ctx,
        &runtime,
        &"q".repeat(90).chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let values = make_simple_vector(&mut ctx, &runtime, &[long, Word::fixnum(2)]).unwrap();
    assert!(
        render(
            &runtime,
            &mut ctx,
            values,
            PrintOptions::new().with_pretty(true)
        )
        .unwrap()
        .contains('\n')
    );
}

#[test]
fn dispatch_and_builtin_error_matrix_preserves_object_errors() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        pprint_dispatch(&mut ctx, Word::fixnum(1), Word::fixnum(2)),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        copy_pprint_dispatch(&mut ctx, &runtime, Word::NIL).unwrap(),
        Word::NIL
    );
    let print = builtin(&runtime, &mut ctx, "PRINT");
    let text = make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[text, Word::fixnum(2)]),
        Err(ObjectError::TypeError)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[]),
        Err(ObjectError::TypeError)
    );
}
