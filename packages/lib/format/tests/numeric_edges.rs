//! Boundary cases for numeric FORMAT directives.

#![expect(
    clippy::expect_used,
    reason = "Test fixtures keep setup failures local to each test."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    HeapRootedWord, Runtime, ThreadContext, Word, make_bignum_from_limbs, make_cons, make_double,
    make_ratio, make_string,
};
use ncl_printer::StringSink;

fn context() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().expect("runtime");
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).expect("register context");
    (runtime, ctx)
}

fn double(runtime: &Runtime, ctx: &mut ThreadContext, value: f64) -> Word {
    make_double(ctx, runtime, value)
        .map(Into::into)
        .expect("double")
}

fn rooted_double(runtime: &Runtime, ctx: &mut ThreadContext, value: f64) -> HeapRootedWord {
    let word = double(runtime, ctx, value);
    HeapRootedWord::new(ctx, word).expect("root double")
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> Word {
    make_string(ctx, runtime, &value.chars().collect::<Vec<_>>()).expect("string")
}

fn rooted_string(runtime: &Runtime, ctx: &mut ThreadContext, value: &str) -> HeapRootedWord {
    let word = string(runtime, ctx, value);
    HeapRootedWord::new(ctx, word).expect("root string")
}

fn run(runtime: &Runtime, ctx: &mut ThreadContext, control: &str, args: &[Word]) -> String {
    let control = parse(control).expect("control");
    let mut sink = StringSink::new();
    let consumed = execute(&control, args, ctx, runtime, &mut sink).expect("execute");
    assert_eq!(consumed, args.len());
    sink.into_string()
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

#[test] // CLHS 22.3.3.1–22.3.3.3
fn float_scale_precision_and_signed_zero_boundaries() {
    let (runtime, mut ctx) = context();
    let large = rooted_double(&runtime, &mut ctx, 123.456);
    let zero = rooted_double(&runtime, &mut ctx, 0.0);
    let negative_zero = rooted_double(&runtime, &mut ctx, -0.0);
    let positive_zero = rooted_double(&runtime, &mut ctx, 0.0);

    assert_eq!(
        run(
            &runtime,
            &mut ctx,
            "~1,1G|~,2,,2E|~,,,0E|~F|~@F|~E|~E",
            &[
                large.get(),
                zero.get(),
                large.get(),
                negative_zero.get(),
                positive_zero.get(),
                negative_zero.get(),
                positive_zero.get()
            ]
        ),
        "1.2E+2|00.0E-1|0.123456E+3|-0.0|+0.0|-0.0E+0|0.0E+0"
    );
    assert_eq!(run(&runtime, &mut ctx, "~,0E", &[large.get()]), "1.E+2");
}

#[test] // CLHS 22.3.3.1–22.3.3.3
fn bignum_to_float_overflow_retains_nonfinite_directive_behavior() {
    let (runtime, mut ctx) = context();
    let mut limbs = [0_u32; 33];
    limbs[32] = 1;
    let positive_word = make_bignum_from_limbs(&mut ctx, &runtime, false, &limbs)
        .map(Into::into)
        .expect("large positive bignum");
    let positive = HeapRootedWord::new(&ctx, positive_word).expect("root large positive bignum");
    let negative_word = make_bignum_from_limbs(&mut ctx, &runtime, true, &limbs)
        .map(Into::into)
        .expect("large negative bignum");
    let negative = HeapRootedWord::new(&ctx, negative_word).expect("root large negative bignum");

    assert_eq!(
        run(
            &runtime,
            &mut ctx,
            "~F|~E|~G|~F",
            &[
                positive.get(),
                positive.get(),
                negative.get(),
                negative.get()
            ]
        ),
        "inf.0|inf|-inf|-inf.0"
    );
}

#[test] // CLHS 22.3.2.1, 22.3.2.2
fn english_radix_skips_zero_groups_and_integer_grouping_uses_custom_interval() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run(
            &runtime,
            &mut ctx,
            "~R|~:R|~,,,1:D|~2R|~36R",
            &[
                Word::fixnum(1_000_001),
                Word::fixnum(4),
                Word::fixnum(1234),
                Word::fixnum(2),
                Word::fixnum(35),
            ]
        ),
        "one million one|fourth|1,2,3,4|10|Z"
    );
}

#[test] // CLHS 22.3.3.2
fn exponential_scale_boundaries_preserve_fallback_and_zero_padding() {
    let (runtime, mut ctx) = context();
    let zero = rooted_double(&runtime, &mut ctx, 0.0);
    let value = rooted_double(&runtime, &mut ctx, 1.25);
    let largest = rooted_double(&runtime, &mut ctx, f64::MAX);
    let rollover = rooted_double(&runtime, &mut ctx, 9.999_999_999_999_999e-307);

    assert_eq!(
        run(
            &runtime,
            &mut ctx,
            "~,2,,2E|~,,,-1000E",
            &[zero.get(), value.get()]
        ),
        "00.0E-1|1.25"
    );
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~,1,,2147483648E", &[value.get()]),
        FormatErrorKind::InvalidParameter
    );
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~,309,,309E", &[largest.get()]),
        FormatErrorKind::InvalidParameter
    );
    assert_eq!(
        run(&runtime, &mut ctx, "~,2,,2E", &[rollover.get()]),
        "10.0E-307"
    );
}

#[test] // CLHS 22.3.2.1
fn roman_rejects_integers_outside_u64_before_range_conversion() {
    let (runtime, mut ctx) = context();
    let beyond_u64_word = make_bignum_from_limbs(&mut ctx, &runtime, false, &[0, 0, 1])
        .map(Into::into)
        .expect("bignum beyond u64");
    let beyond_u64 = HeapRootedWord::new(&ctx, beyond_u64_word).expect("root bignum beyond u64");

    assert_eq!(
        error_kind(&runtime, &mut ctx, "~@R", &[beyond_u64.get()]),
        FormatErrorKind::InvalidParameter
    );
}

#[test] // CLHS 22.3.2.2, 22.3.3.1/.2/.4, 22.3.4.1
fn nonnumeric_floating_directives_use_aesthetic_right_padding() {
    let (runtime, mut ctx) = context();
    let text = rooted_string(&runtime, &mut ctx, "λ");

    assert_eq!(
        run(
            &runtime,
            &mut ctx,
            "~3F|~3E|~D|~,,3$",
            &[text.get(), text.get(), text.get(), text.get()]
        ),
        "λ  |λ  |λ|λ  "
    );
}

#[test] // CLHS 22.3.2.2–22.3.2.5, 22.3.3.1/.2/.4, 22.3.4.1
fn floating_and_radix_fallbacks_right_pad_composite_values() {
    let (runtime, mut ctx) = context();
    let tail_word = make_cons(&mut ctx, &runtime, Word::fixnum(2), Word::NIL).expect("list tail");
    let tail = HeapRootedWord::new(&ctx, tail_word).expect("root list tail");
    let list_word = make_cons(&mut ctx, &runtime, Word::fixnum(1), tail.get()).expect("list");
    let list = HeapRootedWord::new(&ctx, list_word).expect("root list");

    // CLHS F/E/$ send non-floats through ~wD; D/B/O/X use ~A for non-integers.
    // CLHS 22.3.4.1 right-pads ~A output unless @ is supplied.
    assert_eq!(
        run(
            &runtime,
            &mut ctx,
            "~8F|~,,8$|~8D|~8B|~8O|~8X",
            &[
                list.get(),
                list.get(),
                list.get(),
                list.get(),
                list.get(),
                list.get(),
            ]
        ),
        "(1 2)   |(1 2)   |(1 2)   |(1 2)   |(1 2)   |(1 2)   "
    );
}

#[test] // CLHS 22.3.3.1
fn fixed_float_falls_back_to_unquoted_aesthetic_text_without_a_width() {
    let (runtime, mut ctx) = context();
    let text = rooted_string(&runtime, &mut ctx, "not a float");

    assert_eq!(run(&runtime, &mut ctx, "~F", &[text.get()]), "not a float");
}

#[test] // CLHS 22.3.3.1
fn fixed_float_without_precision_omits_fractional_trailing_zeroes() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.2);

    assert_eq!(run(&runtime, &mut ctx, "~5F", &[value.get()]), "  1.2");
}

#[test] // CLHS 22.3.3.1
fn fixed_float_uses_width_to_choose_omitted_precision() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.2345);
    let small = rooted_double(&runtime, &mut ctx, 0.5);
    let carry = rooted_double(&runtime, &mut ctx, 9.99);

    assert_eq!(run(&runtime, &mut ctx, "~4F", &[value.get()]), "1.23");
    assert_eq!(run(&runtime, &mut ctx, "~4@F", &[value.get()]), "+1.2");
    assert_eq!(run(&runtime, &mut ctx, "~2F", &[small.get()]), ".5");
    assert_eq!(run(&runtime, &mut ctx, "~3F", &[carry.get()]), "10.");
}

#[test] // CLHS 22.3.3.1
fn fixed_float_removes_a_zero_created_by_rounding_at_omitted_precision() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, f64::from_bits(0x0e80_0000_0000_0000));

    let output = run(&runtime, &mut ctx, "~256F", &[value.get()]);
    let decimal = output.trim();
    let fraction = decimal.split_once('.').map_or("", |(_, fraction)| fraction);

    assert_eq!(output.chars().count(), 256);
    assert!(decimal.starts_with("0."));
    assert!(fraction.ends_with("63"));
    assert!(!fraction.ends_with('0'));
}

#[test] // CLHS 22.3.2.2, 22.3.3.1–22.3.3.4
fn float_ratio_and_parameter_type_errors_remain_typed() {
    let (runtime, mut ctx) = context();
    let value = rooted_double(&runtime, &mut ctx, 1.0);
    let text = rooted_string(&runtime, &mut ctx, "x");
    let malformed_ratio_word = make_ratio(&mut ctx, &runtime, Word::fixnum(1), text.get())
        .map(Into::into)
        .expect("ratio with non-integer denominator");
    let malformed_ratio = HeapRootedWord::new(&ctx, malformed_ratio_word)
        .expect("root ratio with non-integer denominator");

    assert_eq!(
        error_kind(&runtime, &mut ctx, "~F", &[malformed_ratio.get()]),
        FormatErrorKind::WrongArgumentType
    );
    // CLHS 22.3.3.3 specifies floating-point arguments for ~G.
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~G", &[text.get()]),
        FormatErrorKind::WrongArgumentType
    );
    // CLHS assigns integer and character roles to these directive parameters.
    for control in [
        "~'xF",
        // CLHS 22.3.3.1 specifies d as an integer parameter.
        "~,'xF",
        "~,,'xF",
        "~,1,0,7F",
        "~,1,0,,7F",
        "~'xE",
        // CLHS 22.3.3.2 specifies d as an integer parameter.
        "~,'xE",
        "~,,'xE",
        "~,,,'xE",
        "~,,,,1E",
        "~,,,,,1E",
        "~,,,,,,1E",
        "~'xG",
        // CLHS 22.3.3.3 specifies d as an integer parameter.
        "~,'xG",
        "~,,'xG",
        "~,,,'xG",
        "~,,,,1G",
        "~,,,,,1G",
        "~,,,,,,1G",
        "~'x$",
        "~,'x$",
        "~,,'x$",
        "~,,,1$",
        "~,,1D",
        "~,,,'xD",
    ] {
        assert_eq!(
            error_kind(&runtime, &mut ctx, control, &[value.get()]),
            FormatErrorKind::InvalidParameter,
            "{control}"
        );
    }
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~1,2,3,4,5,6,7,8G", &[value.get()]),
        FormatErrorKind::InvalidParameter
    );
}

#[test] // CLHS 22.3.2.2, 22.3.3.2–22.3.3.4
fn numeric_directives_report_missing_arguments() {
    let (runtime, mut ctx) = context();

    // CLHS 22.3.2.2 says ~D prints its argument as an integer.
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~D", &[]),
        FormatErrorKind::MissingArgument
    );
    // CLHS 22.3.3.2 says ~E prints the next argument in exponential notation.
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~E", &[]),
        FormatErrorKind::MissingArgument
    );
    // CLHS 22.3.3.3 says ~G prints the next argument in fixed or exponential notation.
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~G", &[]),
        FormatErrorKind::MissingArgument
    );
    // CLHS 22.3.3.4 says ~$ prints the next argument in fixed notation.
    assert_eq!(
        error_kind(&runtime, &mut ctx, "~$", &[]),
        FormatErrorKind::MissingArgument
    );
}

#[test] // CLHS 22.3.2.2, 22.3.3.4
fn oversized_padding_is_rejected_before_allocation() {
    let (runtime, mut ctx) = context();
    let text = rooted_string(&runtime, &mut ctx, "x");
    let value = rooted_double(&runtime, &mut ctx, 1.0);
    for (control, args) in [
        ("~9223372036854775807,'λD", &[text.get()][..]),
        ("~2,1,9223372036854775807,'λ$", &[value.get()][..]),
    ] {
        assert_eq!(
            error_kind(&runtime, &mut ctx, control, args),
            FormatErrorKind::InvalidParameter,
            "{control}"
        );
    }
}
