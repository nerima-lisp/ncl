#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on printer output"
)]
#![allow(
    clippy::similar_names,
    reason = "the test compares adjacent PRINC, PRIN1, and PRINT calls"
)]

//! Coverage for readable output flowing through the public print builtins.

use ncl_object::{FunctionObject, Package, Runtime, ThreadContext, Word, make_cons, make_string};
use ncl_printer::{PrintOptions, StringSink, write};

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_printer::register(&mut ctx, &runtime).unwrap();
    ncl_lib_streams::register(&runtime).unwrap();
    (runtime, ctx)
}

fn intern(runtime: &Runtime, ctx: &mut ThreadContext, name: &str) -> Word {
    let package = runtime.ensure_package(ctx, "COMMON-LISP").unwrap();
    Package::from_word(package)
        .intern(ctx, runtime, name)
        .unwrap()
        .0
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
fn readable_print_builtins_force_reader_syntax_and_preserve_print_newlines() {
    let (runtime, mut ctx) = context();
    let escape = intern(&runtime, &mut ctx, "*PRINT-ESCAPE*");
    let readably = intern(&runtime, &mut ctx, "*PRINT-READABLY*");
    ncl_object::set_symbol_special(&mut ctx, escape, true).unwrap();
    ncl_object::set_symbol_special(&mut ctx, readably, true).unwrap();
    ncl_object::set_symbol_value(&mut ctx, escape, Word::NIL).unwrap();
    ncl_object::set_symbol_value(&mut ctx, readably, Word::TRUE).unwrap();

    let object = make_string(&mut ctx, &runtime, &['r', 'e', 'a', 'd', '"', 'a', 'b']).unwrap();
    let make_stream = builtin(&runtime, &mut ctx, "MAKE-STRING-OUTPUT-STREAM");
    let get_output = builtin(&runtime, &mut ctx, "GET-OUTPUT-STREAM-STRING");

    let stream_princ = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let builtin_princ = builtin(&runtime, &mut ctx, "PRINC");
    assert_eq!(
        runtime.call_builtin(&mut ctx, builtin_princ, &[object, stream_princ]),
        Ok(object)
    );
    let princ_output = runtime
        .call_builtin(&mut ctx, get_output, &[stream_princ])
        .unwrap();
    assert_eq!(string_value(&ctx, princ_output), "\"read\\\"ab\"");

    let stream_prin1 = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let builtin_prin1 = builtin(&runtime, &mut ctx, "PRIN1");
    assert_eq!(
        runtime.call_builtin(&mut ctx, builtin_prin1, &[object, stream_prin1]),
        Ok(object)
    );
    let prin1_output = runtime
        .call_builtin(&mut ctx, get_output, &[stream_prin1])
        .unwrap();
    assert_eq!(string_value(&ctx, prin1_output), "\"read\\\"ab\"");

    let stream_print = runtime.call_builtin(&mut ctx, make_stream, &[]).unwrap();
    let builtin_print = builtin(&runtime, &mut ctx, "PRINT");
    assert_eq!(
        runtime.call_builtin(&mut ctx, builtin_print, &[object, stream_print]),
        Ok(object)
    );
    let print_output = runtime
        .call_builtin(&mut ctx, get_output, &[stream_print])
        .unwrap();
    assert_eq!(string_value(&ctx, print_output), "\n\"read\\\"ab\"\n");
}

#[test]
fn readable_options_cover_shared_circle_scanning_and_decimal_radix_suffix() {
    let (runtime, mut ctx) = context();
    let shared = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).unwrap();
    let tail = make_cons(&mut ctx, &runtime, shared, Word::NIL).unwrap();
    let outer = make_cons(&mut ctx, &runtime, shared, tail).unwrap();
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            outer,
            PrintOptions::new()
                .with_circle(true)
                .with_circle_not_shared(true),
        ),
        "((1) (1))"
    );
    assert_eq!(
        render(
            &runtime,
            &mut ctx,
            Word::fixnum(42),
            PrintOptions::new().with_base(10).with_radix(true),
        ),
        "42."
    );
}
