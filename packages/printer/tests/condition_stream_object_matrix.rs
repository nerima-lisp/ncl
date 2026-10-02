#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Sixteenth-wave coverage for condition, stream, circle, and object matrices.

use ncl_object::{
    ArrayElementType, ArrayOptions, FunctionObject, ObjectError, Package, Runtime, ThreadContext,
    Word, make_array, make_cons, make_double, make_simple_vector, make_string, set_symbol_special,
    set_symbol_value,
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

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> FunctionObject {
    FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap()
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
fn condition_matrix_distinguishes_princ_report_and_prin1_opaque() {
    let (runtime, mut ctx) = context();
    ncl_conditions::register(&runtime).unwrap();
    let class = ncl_conditions::condition_class(&mut ctx, &runtime, "SIMPLE-CONDITION").unwrap();
    let control = make_string(&mut ctx, &runtime, &['b', 'a', 'd']).unwrap();
    let condition =
        ncl_conditions::make_condition(&mut ctx, &runtime, class, &[control, Word::NIL]).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            condition,
            PrintOptions::new().with_escape(false)
        ),
        Ok("bad".to_string())
    );
    assert!(
        render(&runtime, &mut ctx, condition, PrintOptions::new())
            .unwrap()
            .starts_with("#<INSTANCE ")
    );
}

#[test]
fn stream_matrix_routes_standard_terminal_and_explicit_streams() {
    let (runtime, mut ctx) = context();
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let standard = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let terminal = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let explicit = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let standard_var = intern(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-OUTPUT*");
    let terminal_var = intern(&runtime, &mut ctx, "COMMON-LISP", "*TERMINAL-IO*");
    set_symbol_value(&mut ctx, standard_var, standard).unwrap();
    set_symbol_value(&mut ctx, terminal_var, terminal).unwrap();
    let text = make_string(&mut ctx, &runtime, &['w', '1', '6']).unwrap();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[text, Word::NIL]),
        Ok(text)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[text, Word::TRUE]),
        Ok(text)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[text, explicit]),
        Ok(text)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    for stream in [standard, terminal, explicit] {
        let output = runtime
            .call_builtin(&mut ctx, get_output, &[stream])
            .unwrap();
        assert_eq!(ncl_object::string_length(&ctx, output).unwrap(), 3);
    }
}

#[test]
fn circle_matrix_distinguishes_repeated_noncycle_from_self_cycle() {
    let (runtime, mut ctx) = context();
    let shared = make_simple_vector(&mut ctx, &runtime, &[Word::fixnum(1)]).unwrap();
    let tail = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    let repeated = make_cons(&mut ctx, &runtime, shared, tail).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            repeated,
            PrintOptions::new()
                .with_circle(true)
                .with_circle_not_shared(true),
        ),
        Ok("(#(1) #(1))".to_string())
    );
    ncl_object::rplacd(&mut ctx, tail, repeated).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            repeated,
            PrintOptions::new()
                .with_circle(true)
                .with_circle_not_shared(true),
        ),
        Ok("#1=(#(1) #(1) . #1#)".to_string())
    );
}

#[test]
fn array_matrix_covers_adjustable_fill_pointer_and_readable_opaque() {
    let (runtime, mut ctx) = context();
    let array = make_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(8),
            adjustable: true,
            fill_pointer: Some(1),
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, array, PrintOptions::new()),
        Ok("#(8 8 8)".to_string())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            array,
            PrintOptions::new().with_vector_length(Some(1))
        ),
        Ok("#(8 ...)".to_string())
    );
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
fn options_matrix_reads_non_nil_other_values_and_rejects_bad_limits() {
    let (runtime, mut ctx) = context();
    let escape = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ESCAPE*");
    let level = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LEVEL*");
    let radix = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-RADIX*");
    for symbol in [escape, level, radix] {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    let other = make_string(&mut ctx, &runtime, &['v']).unwrap();
    set_symbol_value(&mut ctx, escape, other).unwrap();
    set_symbol_value(&mut ctx, level, Word::fixnum(-5)).unwrap();
    set_symbol_value(&mut ctx, radix, Word::TRUE).unwrap();
    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(options.escape());
    assert_eq!(options.level(), None);
    assert!(options.radix());
}

#[test]
fn number_matrix_covers_positive_zero_and_nonfinite_values() {
    let (runtime, mut ctx) = context();
    let positive_zero = make_double(&mut ctx, &runtime, 0.0).unwrap();
    let infinity = make_double(&mut ctx, &runtime, f64::INFINITY).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            positive_zero.as_word(),
            PrintOptions::new()
        ),
        Ok("0.0".to_string())
    );
    assert_eq!(
        render(&runtime, &mut ctx, infinity.as_word(), PrintOptions::new()),
        Ok("#<DOUBLE-FLOAT Infinity>".to_string())
    );
}

#[test]
fn builtin_missing_object_and_bad_stream_keep_object_errors() {
    let (runtime, mut ctx) = context();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[]),
        Err(ObjectError::TypeError)
    );
    let text = make_string(&mut ctx, &runtime, &['x']).unwrap();
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[text, Word::fixnum(16)]),
        Err(ObjectError::TypeError)
    );
}
