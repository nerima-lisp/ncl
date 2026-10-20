#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]

//! Tenth-wave coverage for condition routing, escaped strings, and cycles.

use ncl_object::{
    FunctionObject, Package, Runtime, ThreadContext, Word, make_complex, make_cons, make_ratio,
    make_string,
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
) -> Result<String, PrintError> {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, &options)?;
    Ok(sink.into_string())
}

#[test]
fn princ_and_prin1_route_a_condition_without_report_through_streams() {
    let (runtime, mut ctx) = context();
    ncl_conditions::register(&runtime).unwrap();
    let class = ncl_conditions::condition_class(&mut ctx, &runtime, "ERROR").unwrap();
    let condition = ncl_conditions::make_condition(&mut ctx, &runtime, class, &[]).unwrap();
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let princ_stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let prin1_stream = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    let prin1 = builtin(&runtime, &mut ctx, "PRIN1");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[condition, princ_stream]),
        Ok(condition)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, prin1, &[condition, prin1_stream]),
        Ok(condition)
    );
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");
    let princ_text = runtime
        .call_builtin(&mut ctx, get_output, &[princ_stream])
        .unwrap();
    let prin1_text = runtime
        .call_builtin(&mut ctx, get_output, &[prin1_stream])
        .unwrap();
    let princ_output = string_value(&ctx, princ_text);
    let prin1_output = string_value(&ctx, prin1_text);
    assert_eq!(princ_output, "ERROR condition");
    assert!(prin1_output.starts_with("#<"), "{prin1_output}");
}

#[test]
fn escaped_string_printing_handles_quotes_and_backslashes() {
    let (runtime, mut ctx) = context();
    let string = make_string(&mut ctx, &runtime, &['a', '"', '\\', 'b']).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, string, PrintOptions::new()),
        Ok("\"a\\\"\\\\b\"".to_string())
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            string,
            PrintOptions::new().with_escape(false),
        ),
        Ok("a\"\\b".to_string())
    );
}

#[test]
fn nested_numeric_objects_preserve_radix_and_signs() {
    let (runtime, mut ctx) = context();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(-5), Word::fixnum(8)).unwrap();
    let complex = make_complex(&mut ctx, &runtime, ratio.as_word(), Word::fixnum(-2)).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            complex.as_word(),
            PrintOptions::new().with_base(16).with_radix(true),
        ),
        Ok("#C(#x-5/#x8 #x-2)".to_string())
    );
}

#[test]
fn dotted_cycle_errors_without_circle_and_labels_with_circle() {
    let (runtime, mut ctx) = context();
    let tail = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL).unwrap();
    let root = make_cons(&mut ctx, &runtime, Word::fixnum(1), tail).unwrap();
    ncl_object::rplacd(&mut ctx, tail, root).unwrap();
    assert_eq!(
        render(&runtime, &mut ctx, root, PrintOptions::new()),
        Err(PrintError::Circularity)
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            root,
            PrintOptions::new().with_circle(true),
        ),
        Ok("#1=(1 2 . #1#)".to_string())
    );
}

#[test]
fn print_object_builtin_rejects_missing_object_before_stream_lookup() {
    let (runtime, mut ctx) = context();
    let princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, princ, &[]),
        Err(ncl_object::ObjectError::TypeError)
    );
    let _ = intern(&runtime, &mut ctx, "COMMON-LISP", "*STANDARD-OUTPUT*");
}
