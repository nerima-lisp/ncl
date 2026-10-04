#![allow(missing_docs, clippy::expect_used, clippy::unwrap_used)]

use ncl_lib_format::{execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_double};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("context");
    (runtime, ctx)
}

fn run(control: &str, value: f64) -> String {
    let (runtime, mut ctx) = context();
    let value = make_double(&mut ctx, &runtime, value).expect("float");
    let mut sink = StringSink::new();
    execute(
        &parse(control).expect("control"),
        &[Word::from(value)],
        &mut ctx,
        &runtime,
        &mut sink,
    )
    .expect("execute");
    sink.into_string()
}

#[test]
fn fixed_format_applies_scale_and_overflow_policy() {
    assert_eq!(run("~7,2,2F", 1.25), " 125.00");
    assert_eq!(run("~5,2,,'!F", 123.4), "!!!!!");
    assert_eq!(run("~5,2F", 123.4), "123.40");
}

#[test]
fn exponential_format_preserves_scale_and_exponent_width() {
    assert_eq!(run("~8,2,3,2E", 1.25), "1.25e+002");
    assert_eq!(run("~10,2,4,,,,'XG", 0.000_001_25), "1.25X-0006");
}

#[test]
fn currency_format_honors_minimum_integer_digits_and_colon_sign() {
    assert_eq!(run("~2,3,8,'0$", 1.25), "00001.25");
    assert_eq!(run("~2,1,8,'0:$", -1.25), "-0001.25");
}

#[test]
fn float_fallbacks_cover_zero_rounding_and_nonfinite_values() {
    assert_eq!(run("~E", 1.25), "1.25");
    assert_eq!(run("~8,2E", 0.0), " 0.00e+0");
    assert_eq!(run("~8,1E", 9.5), "  9.5e+1");
    assert_eq!(run("~F", f64::INFINITY), "#<DOUBLE-FLOAT Infinity>");
}
