#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::register;
use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, make_string, make_symbol, string_ref,
};

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<_>>()).expect("string")
}

fn read_string(ctx: &ThreadContext, value: Word) -> String {
    let length = ncl_object::string_length(ctx, value).expect("length");
    (0..length)
        .map(|index| string_ref(ctx, value, index).expect("character"))
        .collect()
}

#[test]
fn format_is_callable_and_executes_value_and_line_directives() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    ncl_lib_streams::register(&runtime).expect("streams");
    register(&runtime).expect("format");

    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT");
    let function = FunctionObject::try_from(function).expect("function");
    let control = string(&runtime, &mut ctx, "~A/~S/~D~%~&");
    let first = string(&runtime, &mut ctx, "x");
    let second = string(&runtime, &mut ctx, "y");
    let result = runtime
        .call_builtin(
            &mut ctx,
            function,
            &[Word::NIL, control, first, second, Word::fixnum(12)],
        )
        .expect("format result");

    assert_eq!(read_string(&ctx, result), "x/\"y\"/12\n");
}

#[test]
fn format_accepts_symbol_and_character_controls() {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    ncl_lib_streams::register(&runtime).expect("streams");
    register(&runtime).expect("format");

    let function = runtime
        .function(&mut ctx, "COMMON-LISP", "FORMAT")
        .expect("FORMAT");
    let function = FunctionObject::try_from(function).expect("function");
    let symbol_name = string(&runtime, &mut ctx, "~A");
    let symbol = make_symbol(&mut ctx, &runtime, symbol_name).expect("symbol");
    let argument = string(&runtime, &mut ctx, "value");

    let symbol_result = runtime
        .call_builtin(&mut ctx, function, &[Word::NIL, symbol, argument])
        .expect("symbol control");
    assert_eq!(read_string(&ctx, symbol_result), "value");

    let character_result = runtime
        .call_builtin(
            &mut ctx,
            function,
            &[Word::NIL, Word::character(u32::from('x'))],
        )
        .expect("character control");
    assert_eq!(read_string(&ctx, character_result), "x");
}
