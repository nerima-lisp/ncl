//! Public numeric FORMAT boundary cases.

#![expect(
    clippy::expect_used,
    reason = "Test fixtures keep setup failures local to each test."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{HeapRootedWord, Runtime, ThreadContext, Word, make_double, make_ratio};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn rooted_double(runtime: &Runtime, ctx: &mut ThreadContext, value: f64) -> HeapRootedWord {
    let word: Word = make_double(ctx, runtime, value)
        .map(Into::into)
        .expect("double");
    HeapRootedWord::new(ctx, word).expect("root double")
}

fn rooted_ratio(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    numerator: i64,
    denominator: i64,
) -> HeapRootedWord {
    let word: Word = make_ratio(
        ctx,
        runtime,
        Word::fixnum(numerator),
        Word::fixnum(denominator),
    )
    .map(Into::into)
    .expect("ratio");
    HeapRootedWord::new(ctx, word).expect("root ratio")
}

fn error_kind(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    control: &str,
    args: &[Word],
) -> FormatErrorKind {
    let control = parse(control).expect("control");
    let mut sink = StringSink::new();
    execute(&control, args, ctx, runtime, &mut sink)
        .expect_err("format error")
        .kind
}

fn format_output(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    control: &str,
    args: &[Word],
) -> String {
    let control = parse(control).expect("control");
    let mut sink = StringSink::new();
    let consumed = execute(&control, args, ctx, runtime, &mut sink).expect("format output");
    assert_eq!(consumed, args.len());
    sink.into_string()
}

#[test]
fn exponential_rejects_scale_at_the_precision_limit() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.25);

    // CLHS 22.3.3.2 requires positive k to be strictly less than d+2.
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~,1,,3E", &[value.get()]),
        FormatErrorKind::InvalidParameter
    );
}

#[test]
fn fixed_rounding_uses_requested_fraction_digits() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.999);

    // CLHS 22.3.3.1 prints two fractional digits; this value is not a rounding tie.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,2F", &[value.get()]),
        "2.00"
    );
}

#[test]
fn fixed_format_applies_positive_scale_before_rounding() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.25);

    // CLHS 22.3.3.1 scales the argument by 10^k before printing d fraction digits.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,2,1F", &[value.get()]),
        "12.50"
    );
}

#[test]
fn exponential_and_general_modes_pad_negative_exponents() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 0.0125);

    // CLHS 22.3.3.2 uses k=1 and d=2; exponent width 3 pads -2 to -002.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,2,3,1E", &[value.get()]),
        "1.25E-002"
    );
    // CLHS 22.3.3.3 selects exponential form here; the supplied exponent marker is q.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,2,3,1,,,'qG", &[value.get()]),
        "1.25q-002"
    );
}

#[test]
fn general_uses_fixed_form_at_both_precision_boundaries() {
    let (runtime, mut ctx) = context();
    let upper = rooted_double(&runtime, &mut ctx, 123.45);
    let lower = rooted_double(&runtime, &mut ctx, 0.56);

    // CLHS 22.3.3.3 selects fixed form when d-n is zero; k is not passed to ~F.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,3,2,-2G", &[upper.get()]),
        "123.    "
    );
    // CLHS 22.3.3.3 also selects fixed form when d-n equals d.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,2,2,1G", &[lower.get()]),
        "0.56    "
    );
}

#[test]
fn fixed_format_accepts_exact_rational_input() {
    let (runtime, mut ctx) = context();
    let half = rooted_ratio(&mut ctx, &runtime, -1, 2);

    // CLHS 22.3.3.1 permits rational arguments to be converted to float; -1/2 is exact.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~F", &[half.get()]),
        "-0.5"
    );
}

#[test]
fn general_exponential_overflow_uses_the_supplied_overflow_character() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 123.456);

    // CLHS 22.3.3.3 selects E form for d-n<0; 22.3.3.2 emits overflowchar if it cannot fit.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~3,2,2,1,'#G", &[value.get()]),
        "###"
    );
}

#[test]
fn general_at_sign_reaches_fixed_and_exponential_signs() {
    let (runtime, mut ctx) = context();
    let fixed = rooted_double(&runtime, &mut ctx, 123.456);
    let exponential = rooted_double(&runtime, &mut ctx, 0.01);

    // CLHS 22.3.3.3 passes @ to ~F and appends ~ee@T; 22.3.3.1 forces the plus sign.
    // CLHS 22.3.6.1 defines the four spaces from that trailing tabulation.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~@G", &[fixed.get()]),
        "+123.456    "
    );
    // CLHS 22.3.3.3 passes @ to E form; CLHS 22.3.3.2 forces the positive exponent sign.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~@G", &[exponential.get()]),
        "+1.0E-2"
    );
}

#[test]
fn general_fixed_form_uses_overflow_character_for_its_field_width() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 123.456);

    // CLHS 22.3.3.3 maps fixed form to ~F followed by ~ee@T; 22.3.3.1 repeats overflowchar.
    // CLHS 22.3.6.1 defines the four trailing spaces from tabulation.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~8,6,2,1,'#G", &[value.get()]),
        "####    "
    );
}

#[test]
fn decimal_grouping_uses_custom_interval_and_separator() {
    let (runtime, mut ctx) = context();

    // CLHS 22.3.2.2 groups decimal digits by comma-interval with the supplied comma character.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~,,'.,2:D", &[Word::fixnum(-12_345)]),
        "-1.23.45"
    );
}

#[test]
fn negative_english_ordinal_keeps_minus_prefix() {
    let (runtime, mut ctx) = context();

    // CLHS 22.3.2.1 defines ~:R as English ordinal output when no radix is supplied.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~:R", &[Word::fixnum(-23)]),
        "minus twenty-third"
    );
}

#[test]
fn monetary_rounding_keeps_sign_and_padding_order() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, -9.999);

    // CLHS 22.3.3.4 rounds two decimals; : places the sign before padding.
    assert_eq!(
        format_output(
            &runtime,
            &mut ctx,
            "~2,1,8,'0$|~2,1,8,'0:$",
            &[value.get(), value.get()]
        ),
        "00-10.00|-0010.00"
    );
}

#[test]
fn monetary_format_zero_pads_to_minimum_integer_digits() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.25);

    // CLHS 22.3.3.4 requires n integer digits and supplies leading zeros when needed.
    assert_eq!(
        format_output(&runtime, &mut ctx, "~2,3$", &[value.get()]),
        "001.25"
    );
}
