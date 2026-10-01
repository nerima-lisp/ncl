#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Sixth-wave coverage for ambient options, pretty separators, arrays, and streams.

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_cons, make_simple_vector, make_string, set_symbol_special,
    set_symbol_value,
};
use ncl_printer::{PrintOptions, StringSink, write};

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
) -> String {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options).unwrap();
    sink.into_string()
}

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
}

fn string_value(ctx: &ThreadContext, value: Word) -> String {
    let length = ncl_object::string_length(ctx, value).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
}

#[test]
fn ambient_length_special_and_extension_fallback_are_asserted() {
    let (runtime, mut ctx) = context();
    let length = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LENGTH*");
    set_symbol_special(&mut ctx, length, true).unwrap();
    set_symbol_value(&mut ctx, length, Word::fixnum(2)).unwrap();
    assert_eq!(
        ncl_object::symbol_value(&ctx, length).unwrap(),
        Word::fixnum(2)
    );

    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert_eq!(options.length().map(ncl_printer::NonNegative::get), Some(2));
    assert_eq!(options.vector_length(), None);
}

#[test]
fn pretty_vectors_break_lines_and_vector_limit_precedes_print_length() {
    let (runtime, mut ctx) = context();
    let long = make_string(
        &mut ctx,
        &runtime,
        &"x".repeat(90).chars().collect::<Vec<_>>(),
    )
    .unwrap();
    let vector = make_simple_vector(
        &mut ctx,
        &runtime,
        &[long, Word::fixnum(2), Word::fixnum(3)],
    )
    .unwrap();
    let pretty = render(
        &runtime,
        &mut ctx,
        vector,
        PrintOptions::new().with_pretty(true),
    );
    assert!(pretty.starts_with("#(\""), "{pretty}");
    assert!(
        pretty.contains('\n'),
        "pretty vector did not break: {pretty}"
    );

    let limited = render(
        &runtime,
        &mut ctx,
        vector,
        PrintOptions::new()
            .with_length(Some(1))
            .with_vector_length(Some(2)),
    );
    assert!(limited.starts_with("#(\""), "{limited}");
    assert!(limited.contains(" 2 ...)"), "{limited}");
}

#[test]
fn rank_two_arrays_apply_limits_and_opaque_mode() {
    let (runtime, mut ctx) = context();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[2, 2],
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
            PrintOptions::new().with_vector_length(Some(2)),
        ),
        "#2A(4 4 ...)",
    );
    let opaque = render(
        &runtime,
        &mut ctx,
        array,
        PrintOptions::new().with_array(false),
    );
    assert!(opaque.starts_with("#<ARRAY "), "{opaque}");
}

#[test]
fn print_builtins_reject_bad_streams_and_preserve_print_newlines() {
    let (runtime, mut ctx) = context();
    let object = make_string(&mut ctx, &runtime, &['o', 'k']).unwrap();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[object, Word::fixnum(1)]),
        Err(ObjectError::TypeError)
    );

    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[object, stream]),
        Ok(object)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let output = runtime
        .call_builtin(&mut ctx, get_output, &[stream])
        .unwrap();
    assert_eq!(string_value(&ctx, output), "\n\"ok\"\n");
}

#[test]
fn circle_scan_keeps_dotted_cycle_labelable_through_the_tail() {
    let (runtime, mut ctx) = context();
    let one = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, Word::fixnum(0), one).unwrap();
    ncl_object::rplacd(&mut ctx, one, outer).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new().with_circle(true),
        ),
        "#1=(0 1 . #1#)"
    );
}
