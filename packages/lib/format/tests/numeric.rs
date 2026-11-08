//! Expected FORMAT output for numeric directives.

#![expect(
    clippy::expect_used,
    reason = "Test fixtures keep setup failures local to each test."
)]

use ncl_lib_format::{FormatErrorKind, execute, parse};
use ncl_object::{
    HeapRootedWord, Runtime, ThreadContext, Word, make_bignum_from_limbs, make_cons, make_double,
    make_ratio, make_string, rplacd,
};
use ncl_printer::{PrintError, StringSink};

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

fn format_error_kind(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    control: &str,
    arguments: &[Word],
) -> FormatErrorKind {
    let control = parse(control).expect("control");
    let mut sink = StringSink::new();
    execute(&control, arguments, ctx, runtime, &mut sink)
        .expect_err("format error")
        .kind
}

fn assert_error_kind(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    control: &str,
    arguments: &[Word],
    expected: &FormatErrorKind,
) {
    assert_eq!(
        &format_error_kind(runtime, ctx, control, arguments),
        expected,
        "{control}"
    );
}

fn string(runtime: &Runtime, ctx: &mut ThreadContext, text: &str) -> Word {
    make_string(ctx, runtime, &text.chars().collect::<Vec<_>>()).expect("string")
}

fn double(runtime: &Runtime, ctx: &mut ThreadContext, value: f64) -> Word {
    make_double(ctx, runtime, value)
        .map(Into::into)
        .expect("double")
}

fn test_bignum(runtime: &Runtime, ctx: &mut ThreadContext, negative: bool, limbs: &[u32]) -> Word {
    make_bignum_from_limbs(ctx, runtime, negative, limbs)
        .map(Into::into)
        .expect("bignum")
}

fn ratio(runtime: &Runtime, ctx: &mut ThreadContext, numerator: Word, denominator: Word) -> Word {
    make_ratio(ctx, runtime, numerator, denominator)
        .map(Into::into)
        .expect("ratio")
}

#[test] // CLHS 22.3.2.1–22.3.2.5
fn formats_radices_with_flags_grouping_and_padding() {
    let (runtime, mut ctx) = context();
    let output = run_format(
        &runtime,
        &mut ctx,
        "~D|~@D|~:D|~5,'0D|~5,'0@D|~,,'|,2:D|~:X|~O|~:D|~,R|~@R",
        &[
            Word::fixnum(-1234),
            Word::fixnum(1234),
            Word::fixnum(1_234_567),
            Word::fixnum(42),
            Word::fixnum(-42),
            Word::fixnum(123_456),
            Word::fixnum(0xABCD),
            Word::fixnum(255),
            Word::fixnum(-1_234_567),
            Word::fixnum(42),
            Word::fixnum(4_000),
        ],
    );
    assert_eq!(
        output,
        "-1234|+1234|1,234,567|00042|00-42|12|34|56|A,BCD|377|-1,234,567|42|MMMM"
    );
}

#[test] // CLHS 22.3.2.2, 22.3.2.3, 22.3.2.5
fn preserves_arbitrary_width_integers_and_resolves_relative_parameters() {
    let (runtime, mut ctx) = context();
    let bignum = test_bignum(&runtime, &mut ctx, false, &[0, 0, 0, 0, 1]);
    let negative = test_bignum(&runtime, &mut ctx, true, &[0, 0, 0, 0, 1]);
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

#[test] // CLHS 22.3.3.4
fn monetary_directive_rounds_rational_values() {
    let (runtime, mut ctx) = context();
    let ratio = ratio(&runtime, &mut ctx, Word::fixnum(1), Word::fixnum(3));
    let output = run_format(&runtime, &mut ctx, "~$", &[ratio]);
    assert_eq!(output, "0.33");
}

#[test] // CLHS 22.3.2.1
fn formats_english_ordinal_and_roman_boundary_values() {
    let (runtime, mut ctx) = context();
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~R|~:R|~R|~:R|~R|~:R|~R|~:R|~R|~:R|~R|~R|~:R|~@R|~:@R|~@R|~:@R",
            &[
                Word::fixnum(0),
                Word::fixnum(0),
                Word::fixnum(100),
                Word::fixnum(100),
                Word::fixnum(120),
                Word::fixnum(120),
                Word::fixnum(123),
                Word::fixnum(123),
                Word::fixnum(1_005_020),
                Word::fixnum(1_000_000),
                Word::fixnum(-123),
                Word::fixnum(4),
                Word::fixnum(4),
                Word::fixnum(1994),
                Word::fixnum(1994),
                Word::fixnum(-9),
                Word::fixnum(0),
            ]
        ),
        "zero|zeroth|one hundred|one hundredth|one hundred twenty|one hundred twentieth|one hundred twenty-three|one hundred twenty-third|one million five thousand twenty|one millionth|minus one hundred twenty-three|four|fourth|MCMXCIV|MDCCCCLXXXXIIII|-IX|N"
    );
}

#[test] // CLHS 22.3.2.1/.2, 22.3.4.1
fn rejects_invalid_radix_parameters_and_keeps_aesthetic_fallbacks() {
    let (runtime, mut ctx) = context();
    let word = string(&runtime, &mut ctx, "x");
    assert_eq!(
        run_format(&runtime, &mut ctx, "~5D|~R", &[word, Word::fixnum(3)],),
        "x    |three"
    );
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~R", &[word]),
        FormatErrorKind::WrongArgumentType
    );
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~R", &[]),
        FormatErrorKind::MissingArgument
    );

    for control in [
        "~1R",
        "~37R",
        "~'xR",
        "~,,,0:D",
        "~1,2,3,4,5D",
        "~'xD",
        "~-1D",
        "~1,2D",
    ] {
        assert_error_kind(
            &runtime,
            &mut ctx,
            control,
            &[Word::fixnum(5)],
            &FormatErrorKind::InvalidParameter,
        );
    }
    assert_error_kind(
        &runtime,
        &mut ctx,
        "~vD",
        &[Word::character(u32::from('x')), Word::fixnum(5)],
        &FormatErrorKind::InvalidParameter,
    );
    assert_error_kind(
        &runtime,
        &mut ctx,
        "~9223372036854775807,'🦕D",
        &[word],
        &FormatErrorKind::InvalidParameter,
    );
    assert_eq!(
        run_format(&runtime, &mut ctx, "~1,'x,,4:D", &[Word::fixnum(12_345)]),
        "1,2345"
    );
}

#[test] // CLHS 22.3.2.1
fn formats_radix_bignums_and_rejects_unrepresentable_english_and_roman_values() {
    let (runtime, mut ctx) = context();
    let large = test_bignum(&runtime, &mut ctx, false, &[0, 0, 1]);
    let too_large_for_english = test_bignum(
        &runtime,
        &mut ctx,
        false,
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    );
    assert_eq!(
        run_format(&runtime, &mut ctx, "~16R|~16:R", &[large, large]),
        "10000000000000000|10,000,000,000,000,000"
    );
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~R", &[too_large_for_english]),
        FormatErrorKind::InvalidParameter
    );
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~@R", &[Word::fixnum(100_001)]),
        FormatErrorKind::InvalidParameter
    );
}

#[test] // CLHS 22.3.3.1–22.3.3.3
fn formats_default_float_precision_shortest_values_and_non_finite_numbers() {
    let (runtime, mut ctx) = context();
    let general = double(&runtime, &mut ctx, 123.456);
    let tiny = double(&runtime, &mut ctx, 0.01);
    let shortest = double(&runtime, &mut ctx, 12.34);
    let rounded = double(&runtime, &mut ctx, 123.456);
    let trailing_zeroes = double(&runtime, &mut ctx, 100.0);
    let zero = double(&runtime, &mut ctx, 0.0);
    let positive_scale = double(&runtime, &mut ctx, 1.25);
    let negative_scale = double(&runtime, &mut ctx, 1.25);
    let output = run_format(
        &runtime,
        &mut ctx,
        "~E|~G|~G|~8F|~10,2,2E|~E|~E|~,,,2E|~,,,-1E|~G",
        &[
            general,
            general,
            tiny,
            shortest,
            rounded,
            trailing_zeroes,
            zero,
            positive_scale,
            negative_scale,
            zero,
        ],
    );
    assert_eq!(
        output,
        "1.23456E+2|123.456    |1.0E-2|   12.34|  1.23E+02|1.0E+2|0.0E+0|12.5E-1|0.0125E+2|0.    "
    );

    let infinity = double(&runtime, &mut ctx, f64::INFINITY);
    let nan = double(&runtime, &mut ctx, f64::NAN);
    let negative_infinity = double(&runtime, &mut ctx, f64::NEG_INFINITY);
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~0,2F|~0,2E|~0,2G|~,,1F|~,,1F",
            &[infinity, nan, negative_infinity, infinity, nan],
        ),
        "inf|NaN|-inf|inf|NaN"
    );
}

#[test] // CLHS 22.3.2.2, 22.3.3.1/.2/.4, 22.3.4.1
fn float_directives_resolve_parameters_signs_fallbacks_and_scaled_values() {
    let (runtime, mut ctx) = context();
    let text = string(&runtime, &mut ctx, "x");
    let suffix = string(&runtime, &mut ctx, "z");
    let value = double(&runtime, &mut ctx, 1.25);
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~F|~5,2F|~5,2,2E|~,1,5$|~4F",
            &[value, text, text, text, text],
        ),
        "1.25|x    |x    |x    |x   "
    );
    let rounded = double(&runtime, &mut ctx, 123.456);
    let exponential = double(&runtime, &mut ctx, 637.5);
    let currency = double(&runtime, &mut ctx, 3.5);
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~8,2F|~5,2,0,'#,'0F|~8,2,2E|~14,6G|~2,1,8,'0:@$|~F",
            &[
                rounded,
                rounded,
                exponential,
                rounded,
                currency,
                Word::fixnum(42)
            ],
        ),
        "  123.46|#####|6.38E+02|   123.456    |+0003.50|42.0"
    );
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~5,2F|~v,2F|~#,2F~A",
            &[text, Word::fixnum(6), value, value, suffix,],
        ),
        "x    |  1.25|1.25z"
    );
    assert_eq!(
        run_format(&runtime, &mut ctx, "~8,2@F", &[value]),
        "   +1.25"
    );
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~3,2,,'#,'0F|~3,2F|~4,2F|~2,2,2,,'#E|~10,2,3,1,'#,'0,'qE",
            &[value, value, value, value, value]
        ),
        "###|1.25|1.25|##|01.25q+000"
    );
    let whole = double(&runtime, &mut ctx, 100.0);
    assert_eq!(run_format(&runtime, &mut ctx, "~8F", &[whole]), "   100.0");
    let negative_float = double(&runtime, &mut ctx, -0.25);
    assert_eq!(
        run_format(&runtime, &mut ctx, "~F", &[negative_float]),
        "-0.25"
    );
    assert_eq!(
        run_format(&runtime, &mut ctx, "~,,,1000E", &[value]),
        "1.25"
    );
    let scaled = double(&runtime, &mut ctx, 12.6);
    assert_eq!(run_format(&runtime, &mut ctx, "~,2,-1F", &[scaled]), "1.26");

    let negative = double(&runtime, &mut ctx, -0.25);
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~2,3,8,'0$|~2,3,8,'0:$",
            &[negative, negative],
        ),
        "0-000.25|-0000.25"
    );

    let bignum = test_bignum(&runtime, &mut ctx, false, &[0, 0, 1]);
    let negative_bignum = test_bignum(&runtime, &mut ctx, true, &[0, 0, 1]);
    assert_eq!(
        run_format(
            &runtime,
            &mut ctx,
            "~10,2,2E|~10,2,2E",
            &[bignum, negative_bignum]
        ),
        "  1.84E+19| -1.84E+19"
    );
}

#[test] // CLHS 22.3.2.2, 22.3.3.1, 22.3.3.2, 22.3.3.4
fn rejects_float_parameter_and_precision_boundaries() {
    let (runtime, mut ctx) = context();
    let value = double(&runtime, &mut ctx, 1.0);
    let zero_denominator = ratio(&runtime, &mut ctx, Word::fixnum(1), Word::fixnum(0));
    let zero = double(&runtime, &mut ctx, 0.0);
    let text = string(&runtime, &mut ctx, "x");
    let string_numerator = ratio(&runtime, &mut ctx, text, Word::fixnum(1));
    for control in [
        "~vF",
        "~1,10001F",
        "~,,2147483648F",
        "~,1,,3E",
        "~,,309F",
        "~,1,,-1E",
    ] {
        let args = if control == "~vF" {
            &[Word::fixnum(-1), value][..]
        } else {
            &[value][..]
        };
        assert_error_kind(
            &runtime,
            &mut ctx,
            control,
            args,
            &FormatErrorKind::InvalidParameter,
        );
    }
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~2,1,8,'0,1$", &[value]),
        FormatErrorKind::InvalidParameter
    );
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~,,-400F", &[value]),
        FormatErrorKind::InvalidParameter
    );
    assert_error_kind(
        &runtime,
        &mut ctx,
        "~,10000,,10000E",
        &[zero],
        &FormatErrorKind::InvalidParameter,
    );
    assert_error_kind(
        &runtime,
        &mut ctx,
        "~,400,,-399E",
        &[zero],
        &FormatErrorKind::InvalidParameter,
    );
    for control in ["~1,2,3,4,5,6F", "~1,2,3,4,5,6,7,8E"] {
        assert_error_kind(
            &runtime,
            &mut ctx,
            control,
            &[value],
            &FormatErrorKind::InvalidParameter,
        );
    }
    for control in ["~F", "~vF"] {
        assert_error_kind(
            &runtime,
            &mut ctx,
            control,
            &[],
            &FormatErrorKind::MissingArgument,
        );
    }
    assert_eq!(
        format_error_kind(&runtime, &mut ctx, "~F", &[zero_denominator]),
        FormatErrorKind::WrongArgumentType
    );
    assert_error_kind(
        &runtime,
        &mut ctx,
        "~F",
        &[string_numerator],
        &FormatErrorKind::WrongArgumentType,
    );
    assert_error_kind(
        &runtime,
        &mut ctx,
        "~2,1,9223372036854775807,'🦕$",
        &[value],
        &FormatErrorKind::InvalidParameter,
    );

    let cycle = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::NIL).expect("cons");
    let cycle = HeapRootedWord::new(&ctx, cycle).expect("root cycle");
    rplacd(&mut ctx, cycle.get(), cycle.get()).expect("make cycle");
    for control in ["~5F", "~5D"] {
        assert_error_kind(
            &runtime,
            &mut ctx,
            control,
            &[cycle.get()],
            &FormatErrorKind::Print(PrintError::Circularity),
        );
    }
}
