#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Fifth-wave coverage for numeric boundaries, array traversal, and builtin errors.

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_bignum_from_i128, make_cons, make_double, make_simple_vector,
    make_specialized_array, rplacd, set_symbol_special, set_symbol_value,
};
use ncl_printer::{
    ArrayMode, CircleMode, CircleSharingMode, EscapeMode, GensymMode, PrettyMode, PrintBase,
    PrintOptions, RadixMode, ReadabilityMode, StringSink, write,
};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn render(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    object: Word,
    options: PrintOptions,
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options).unwrap();
    sink.into_string()
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, package: &str, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, package).unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
}

fn builtin(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, package, name).unwrap()).unwrap()
}

#[test]
fn number_boundaries_cover_radices_bignum_zero_and_non_finite_floats() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(10),
            PrintOptions::new().with_base(3).with_radix(true)
        ),
        "#3r101"
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(-255),
            PrintOptions::new().with_base(36)
        ),
        "-73"
    );
    let zero = make_bignum_from_i128(&mut ctx, &runtime, 0).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, zero.as_word(), PrintOptions::new()),
        "0"
    );
    let nan = make_double(&mut ctx, &runtime, f64::NAN).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, nan.as_word(), PrintOptions::new()),
        "#<DOUBLE-FLOAT NaN>"
    );
    let infinity = make_double(&mut ctx, &runtime, f64::NEG_INFINITY).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, infinity.as_word(), PrintOptions::new()),
        "#<DOUBLE-FLOAT -Infinity>"
    );
}

#[test]
fn option_modes_expose_each_semantic_branch() {
    let options = PrintOptions::new()
        .with_escape_mode(EscapeMode::Raw)
        .with_readability_mode(ReadabilityMode::Readable)
        .with_print_base(PrintBase::new(16).unwrap())
        .with_radix(true)
        .with_circle(true)
        .with_pretty(true)
        .with_array(false)
        .with_gensym(false)
        .with_circle_not_shared(true);
    assert_eq!(options.escape_mode(), EscapeMode::Raw);
    assert_eq!(options.readability_mode(), ReadabilityMode::Readable);
    assert_eq!(options.base(), PrintBase::new(16).unwrap());
    assert_eq!(options.radix_mode(), RadixMode::WithRadix);
    assert_eq!(options.circle_mode(), CircleMode::Circle);
    assert_eq!(options.pretty_mode(), PrettyMode::Pretty);
    assert_eq!(options.array_mode(), ArrayMode::Opaque);
    assert_eq!(options.gensym_mode(), GensymMode::WithoutPrefix);
    assert_eq!(options.circle_sharing_mode(), CircleSharingMode::OnlyShared);
}

#[test]
fn circle_scan_traverses_general_and_specialized_arrays() {
    let (runtime, mut ctx) = context();
    let general = make_array(
        &mut ctx,
        &runtime,
        &[2],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(8),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    let specialized = make_specialized_array(
        &mut ctx,
        &runtime,
        ArrayElementType::Fixnum,
        &[Word::fixnum(9), Word::fixnum(10)],
    )
    .unwrap();
    let options = PrintOptions::new().with_circle(true);
    assert_eq!(render(&runtime, &mut ctx, general, options), "#(8 8)");
    assert_eq!(render(&runtime, &mut ctx, specialized, options), "#(9 10)");
}

#[test]
fn print_builtins_report_missing_arguments_and_circularity() {
    let (runtime, mut ctx) = context();
    let princ_fn = builtin(&runtime, &mut ctx, "COMMON-LISP", "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ_fn, &[]),
        Err(ObjectError::TypeError)
    );

    let make_stream = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "MAKE-STRING-OUTPUT-STREAM",
    );
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let cycle = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    rplacd(&mut ctx, cycle, cycle).unwrap();
    let output_fn = builtin(&runtime, &mut ctx, "COMMON-LISP", "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, output_fn, &[cycle, stream]),
        Err(ObjectError::Layout)
    );
}

#[test]
fn prin1_maps_unreadable_opaque_array_to_a_layout_error() {
    let (runtime, mut ctx) = context();
    let readable = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-READABLY*");
    let array = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ARRAY*");
    set_symbol_special(&mut ctx, readable, true).unwrap();
    set_symbol_special(&mut ctx, array, true).unwrap();
    set_symbol_value(&mut ctx, readable, Word::TRUE).unwrap();
    set_symbol_value(&mut ctx, array, Word::NIL).unwrap();
    let vector = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let make_stream = builtin(
        &runtime,
        &mut ctx,
        "COMMON-LISP",
        "MAKE-STRING-OUTPUT-STREAM",
    );
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let prin1 = builtin(&runtime, &mut ctx, "COMMON-LISP", "PRIN1");
    assert_eq!(
        runtime.call_builtin(&mut ctx, prin1, &[vector, stream]),
        Err(ObjectError::Layout)
    );
}
