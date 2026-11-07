#![expect(
    clippy::expect_used,
    reason = "tests assert on valid FORMAT controls and the expected printer output"
)]
//! Tests for FORMAT directive execution and printer output.

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_string};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<char>>()).expect("string")
}

#[test]
fn executes_literals_and_basic_value_directives() {
    let (runtime, mut ctx) = context();
    let control = parse("x=~A s=~S").expect("control");
    let value = string(&runtime, &mut ctx, "hello");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(&control, &[value, value], &mut ctx, &runtime, &mut sink),
        Ok(2)
    );
    assert_eq!(sink.into_string(), "x=hello s=\"hello\"");
}

#[test]
fn executes_integer_radices_and_line_controls() {
    let (runtime, mut ctx) = context();
    let control = parse("~D ~B ~O ~X~2%tail~&done~~").expect("control");
    let mut sink = StringSink::new();
    assert_eq!(
        execute(
            &control,
            &[Word::fixnum(255); 4],
            &mut ctx,
            &runtime,
            &mut sink,
        ),
        Ok(4)
    );
    assert_eq!(sink.into_string(), "255 11111111 377 FF\n\ntail\ndone~");
}

#[test]
fn rejects_missing_arguments_and_formats_non_integer_radix_arguments_as_aesthetic() {
    let (runtime, mut ctx) = context();
    let mut sink = StringSink::new();
    let missing = parse("~A").expect("control");
    let error =
        execute(&missing, &[], &mut ctx, &runtime, &mut sink).expect_err("missing argument");
    assert_eq!(error.position, 0);
    assert_eq!(error.directive, Some('A'));
    assert_eq!(error.kind, FormatErrorKind::MissingArgument);
    let non_integer = parse("~D").expect("control");
    let value = string(&runtime, &mut ctx, "not an integer");
    let consumed = execute(&non_integer, &[value], &mut ctx, &runtime, &mut sink)
        .expect("non-integer radix arguments use aesthetic output");
    assert_eq!(consumed, 1);
    assert_eq!(sink.into_string(), "not an integer");
}

#[test]
fn ampersand_does_not_add_a_second_newline_at_line_start() {
    let (runtime, mut ctx) = context();
    let control = parse("a~%~&b").expect("control");
    let mut sink = StringSink::new();
    execute(&control, &[], &mut ctx, &runtime, &mut sink).expect("execute");
    assert_eq!(sink.into_string(), "a\nb");
}
