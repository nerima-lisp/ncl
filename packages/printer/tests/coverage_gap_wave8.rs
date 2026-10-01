#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Eighth-wave coverage for stream routing, ambient option values, and errors.

use ncl_object::{
    ArrayElementType, ArrayOptions, ObjectError, Package, Runtime, ThreadContext, Word, make_array,
    make_complex, make_cons, make_ratio, set_symbol_special, set_symbol_value,
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

fn builtin(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> ncl_object::FunctionObject {
    ncl_object::FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap())
        .unwrap()
}

fn string_value(ctx: &ThreadContext, value: Word) -> String {
    let length = ncl_object::string_length(ctx, value).unwrap();
    (0..length)
        .map(|index| ncl_object::string_ref(ctx, value, index).unwrap())
        .collect()
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

#[test]
fn princ_and_prin1_route_nil_and_true_stream_designators() {
    let (runtime, mut ctx) = context();
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let standard = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let terminal = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let standard_var = intern(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-OUTPUT*");
    let terminal_var = intern(&runtime, &mut ctx, "COMMON-LISP", "*TERMINAL-IO*");
    set_symbol_value(&mut ctx, standard_var, standard).unwrap();
    set_symbol_value(&mut ctx, terminal_var, terminal).unwrap();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['o', 'k']).unwrap();

    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[text, Word::NIL]),
        Ok(text)
    );
    let prin1 = builtin(&runtime, &mut ctx, "PRIN1");
    assert_eq!(
        runtime.call_builtin(&mut ctx, prin1, &[text, Word::TRUE]),
        Ok(text)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let standard_text = runtime
        .call_builtin(&mut ctx, get_output, &[standard])
        .unwrap();
    let terminal_text = runtime
        .call_builtin(&mut ctx, get_output, &[terminal])
        .unwrap();
    assert_eq!(string_value(&ctx, standard_text), "ok");
    assert_eq!(string_value(&ctx, terminal_text), "\"ok\"");
}

#[test]
fn ambient_specials_cover_other_values_and_case_fallbacks() {
    let (runtime, mut ctx) = context();
    let escape = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-ESCAPE*");
    let base = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-BASE*");
    let length = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-LENGTH*");
    let case = intern(&runtime, &mut ctx, "COMMON-LISP", "*PRINT-CASE*");
    for symbol in [escape, base, length, case] {
        set_symbol_special(&mut ctx, symbol, true).unwrap();
    }
    set_symbol_value(&mut ctx, escape, Word::fixnum(1)).unwrap();
    let invalid_base = ncl_object::make_string(&mut ctx, &runtime, &['x']).unwrap();
    set_symbol_value(&mut ctx, base, invalid_base).unwrap();
    set_symbol_value(&mut ctx, length, Word::TRUE).unwrap();
    set_symbol_value(&mut ctx, case, Word::fixnum(3)).unwrap();

    let options = PrintOptions::from_specials(&mut ctx, &runtime);
    assert!(options.escape());
    assert_eq!(options.base().get(), 10);
    assert_eq!(options.length(), None);
    assert_eq!(options.case(), ncl_printer::PrintCase::Upcase);
}

#[test]
fn displaced_array_prints_row_major_values_and_readability_rejects_opaque() {
    let (runtime, mut ctx) = context();
    let source = make_array(
        &mut ctx,
        &runtime,
        &[3],
        ArrayOptions {
            element_type: ArrayElementType::T,
            initial_element: Word::fixnum(7),
            adjustable: false,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )
    .unwrap();
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
        "#(7 7)"
    );
    let error = {
        let mut sink = StringSink::new();
        write(
            &mut ctx,
            &runtime,
            displaced,
            &mut sink,
            &PrintOptions::new().with_readably(true).with_array(false),
        )
        .unwrap_err()
    };
    assert_eq!(error, PrintError::NotReadable);
}

#[test]
fn ratio_complex_and_cycle_render_with_nested_printer_calls() {
    let (runtime, mut ctx) = context();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(-3), Word::fixnum(4)).unwrap();
    let complex = make_complex(&mut ctx, &runtime, ratio.as_word(), Word::fixnum(2)).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, complex.as_word(), PrintOptions::new()),
        "#C(-3/4 2)"
    );

    let tail = make_cons(&mut ctx, &runtime, Word::fixnum(8), Word::NIL).unwrap();
    let root = make_cons(&mut ctx, &runtime, Word::fixnum(9), tail).unwrap();
    ncl_object::rplacd(&mut ctx, tail, root).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            root,
            PrintOptions::new().with_circle(true),
        ),
        "#1=(9 8 . #1#)"
    );
}

#[test]
fn builtin_print_reports_invalid_output_stream_type() {
    let (runtime, mut ctx) = context();
    let text = ncl_object::make_string(&mut ctx, &runtime, &['x']).unwrap();
    let print = builtin(&runtime, &mut ctx, "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, print, &[text, Word::fixnum(0)]),
        Err(ObjectError::TypeError)
    );
}
