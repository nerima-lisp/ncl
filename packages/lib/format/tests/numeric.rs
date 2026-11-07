//! Expected FORMAT output for numeric directives.

#![expect(
    clippy::expect_used,
    reason = "Test fixtures keep setup failures local to each test."
)]

use ncl_lib_format::{execute, parse};
use ncl_object::{Runtime, ThreadContext, Word, make_bignum_from_limbs, make_double, make_ratio};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn run_format(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    control: &str,
    arguments: &[Word],
) -> String {
    let control = parse(control).expect("control");
    let mut sink = StringSink::new();
    let consumed = execute(&control, arguments, ctx, runtime, &mut sink).expect("execute");
    assert_eq!(consumed, arguments.len());
    sink.into_string()
}

#[test]
fn formats_radices_with_flags_grouping_and_padding() {
    let (runtime, mut ctx) = context();
    let output = run_format(
        &runtime,
        &mut ctx,
        "~D|~@D|~:D|~5,'0D|~5,'0@D|~,,'|,2:D|~:X|~O",
        &[
            Word::fixnum(-1234),
            Word::fixnum(1234),
            Word::fixnum(1_234_567),
            Word::fixnum(42),
            Word::fixnum(-42),
            Word::fixnum(123_456),
            Word::fixnum(0xABCD),
            Word::fixnum(255),
        ],
    );
    assert_eq!(
        output,
        "-1234|+1234|1,234,567|00042|00-42|12|34|56|A,BCD|377"
    );
}

#[test]
fn formats_radix_english_ordinal_and_roman_variants() {
    let (runtime, mut ctx) = context();
    let output = run_format(
        &runtime,
        &mut ctx,
        "~R|~:R|~@R|~:@R|~R|~16R|~R|~:R",
        &[
            Word::fixnum(4),
            Word::fixnum(21),
            Word::fixnum(4),
            Word::fixnum(4),
            Word::fixnum(-42),
            Word::fixnum(255),
            Word::fixnum(0),
            Word::fixnum(0),
        ],
    );
    assert_eq!(
        output,
        "four|twenty-first|IV|IIII|minus forty-two|FF|zero|zeroth"
    );
}

#[test]
fn formats_fixed_exponential_general_and_monetary_floats() {
    let (runtime, mut ctx) = context();
    let fixed = make_double(&mut ctx, &runtime, 123.456)
        .map(Into::into)
        .expect("fixed value");
    let exponential = make_double(&mut ctx, &runtime, 637.5)
        .map(Into::into)
        .expect("exponential value");
    let general = make_double(&mut ctx, &runtime, 123.456)
        .map(Into::into)
        .expect("general value");
    let currency = make_double(&mut ctx, &runtime, 3.5)
        .map(Into::into)
        .expect("currency value");
    let overflow = make_double(&mut ctx, &runtime, 123.456)
        .map(Into::into)
        .expect("overflow value");
    let output = run_format(
        &runtime,
        &mut ctx,
        "~8,2F|~5,2,0,'#,'0F|~8,2,2E|~14,6G|~2,1,8,'0:@$|~F",
        &[
            fixed,
            overflow,
            exponential,
            general,
            currency,
            Word::fixnum(42),
        ],
    );
    assert_eq!(
        output,
        "  123.46|#####|6.38E+02|   123.456    |+0003.50|42.0"
    );
}

#[test]
fn preserves_arbitrary_width_integers_and_resolves_relative_parameters() {
    let (runtime, mut ctx) = context();
    let bignum = make_bignum_from_limbs(&mut ctx, &runtime, false, &[0, 0, 0, 0, 1])
        .map(Into::into)
        .expect("bignum");
    let negative = make_bignum_from_limbs(&mut ctx, &runtime, true, &[0, 0, 0, 0, 1])
        .map(Into::into)
        .expect("negative bignum");
    let output = run_format(
        &runtime,
        &mut ctx,
        "~D|~D|~X|~B",
        &[bignum, negative, bignum, bignum],
    );
    assert_eq!(
        output,
        format!(
            "340282366920938463463374607431768211456|-340282366920938463463374607431768211456|1{}|1{}",
            "0".repeat(32),
            "0".repeat(128),
        )
    );

    let output = run_format(
        &runtime,
        &mut ctx,
        "~vD|~#D",
        &[Word::fixnum(5), Word::fixnum(42), Word::fixnum(42)],
    );
    assert_eq!(output, "   42|42");
}

#[test]
fn monetary_directive_rounds_rational_values() {
    let (runtime, mut ctx) = context();
    let ratio = make_ratio(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(3))
        .map(Into::into)
        .expect("ratio");
    let output = run_format(&runtime, &mut ctx, "~$", &[ratio]);
    assert_eq!(output, "0.33");
}
