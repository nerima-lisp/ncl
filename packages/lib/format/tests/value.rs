#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::{execute, parse};
use ncl_object::{make_string, Runtime, ThreadContext, Word};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn run(control: &str, value: Word, ctx: &mut ThreadContext, runtime: &Runtime) -> String {
    let mut sink = StringSink::new();
    execute(
        &parse(control).expect("control"),
        &[value],
        ctx,
        runtime,
        &mut sink,
    )
    .expect("execute");
    sink.into_string()
}

#[test]
fn formats_a_and_s_with_clhs_padding_parameters_and_alignment() {
    let (runtime, mut ctx) = context();
    let value = string(&runtime, &mut ctx, "hello");

    assert_eq!(run("~8A", value, &mut ctx, &runtime), "hello   ");
    assert_eq!(run("~8@A", value, &mut ctx, &runtime), "   hello");
    assert_eq!(run("~5,3,6,'_A", value, &mut ctx, &runtime), "hello______");
    assert_eq!(run("~8,1,0,'_@S", value, &mut ctx, &runtime), "_\"hello\"");
}

#[test]
fn formats_nil_with_colon_as_empty_list_notation() {
    let (runtime, mut ctx) = context();

    let mut sink = StringSink::new();
    execute(
        &parse("~A/~S").expect("control"),
        &[Word::NIL, Word::NIL],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "NIL/NIL");
    let mut sink = StringSink::new();
    execute(
        &parse("~:A/~:S").expect("control"),
        &[Word::NIL, Word::NIL],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    assert_eq!(sink.into_string(), "()/()");
}
