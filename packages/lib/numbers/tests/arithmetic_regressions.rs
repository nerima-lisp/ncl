#![expect(
    clippy::unwrap_used,
    reason = "tests call registered numeric builtins with valid fixtures"
)]
//! Public-boundary regressions for arithmetic across integer and numeric representations.

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, make_bignum_from_limbs, make_double,
};

fn setup() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_conditions::register(&runtime).unwrap();
    ncl_lib_numbers::register(&runtime).unwrap();
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function =
        FunctionObject::try_from(runtime.function(ctx, "COMMON-LISP", name).unwrap()).unwrap();
    runtime.call_builtin(ctx, function, args).unwrap()
}

fn integer_from_limbs(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    negative: bool,
    limbs: &[u32],
) -> Word {
    make_bignum_from_limbs(ctx, runtime, negative, limbs)
        .unwrap()
        .into()
}

fn ratio(runtime: &Runtime, ctx: &mut ThreadContext, numerator: i64, denominator: i64) -> Word {
    call(
        runtime,
        ctx,
        "/",
        &[Word::fixnum(numerator), Word::fixnum(denominator)],
    )
}

fn assert_equal(runtime: &Runtime, ctx: &mut ThreadContext, left: Word, right: Word, label: &str) {
    assert_eq!(
        call(runtime, ctx, "=", &[left, right]),
        Word::TRUE,
        "{label}"
    );
}

fn assert_predicate(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    name: &str,
    args: &[Word],
    expected: bool,
) {
    assert_eq!(
        call(runtime, ctx, name, args),
        if expected { Word::TRUE } else { Word::NIL },
        "{name}"
    );
}

fn complex_floats(ctx: &mut ThreadContext, runtime: &Runtime, real: f64, imag: f64) -> Word {
    let mut real = make_double(ctx, runtime, real).unwrap().into();
    let real_root = ncl_object::push_root(ctx, &mut real);
    let mut imag = make_double(ctx, runtime, imag).unwrap().into();
    let imag_root = ncl_object::push_root(ctx, &mut imag);
    let complex = call(runtime, ctx, "COMPLEX", &[real, imag]);
    assert!(ncl_object::pop_root(ctx, imag_root));
    assert!(ncl_object::pop_root(ctx, real_root));
    complex
}

fn assert_complex_parts(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    value: Word,
    expected_real: i64,
    expected_imag: i64,
) {
    assert_predicate(runtime, ctx, "COMPLEXP", &[value], true);
    let mut real = call(runtime, ctx, "REALPART", &[value]);
    let real_root = ncl_object::push_root(ctx, &mut real);
    assert_predicate(runtime, ctx, "FLOATP", &[real], true);
    assert_equal(
        runtime,
        ctx,
        real,
        Word::fixnum(expected_real),
        "complex real part",
    );
    let mut imag = call(runtime, ctx, "IMAGPART", &[value]);
    let imag_root = ncl_object::push_root(ctx, &mut imag);
    assert_predicate(runtime, ctx, "FLOATP", &[imag], true);
    assert_equal(
        runtime,
        ctx,
        imag,
        Word::fixnum(expected_imag),
        "complex imaginary part",
    );
    assert!(ncl_object::pop_root(ctx, imag_root));
    assert!(ncl_object::pop_root(ctx, real_root));
}

#[test]
fn bignum_abs_negation_and_signum_preserve_sign_and_magnitude() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: a = 680564733841876926939094893764770990802.
    let mut positive = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let positive_root = ncl_object::push_root(&mut ctx, &mut positive);
    let mut negative = integer_from_limbs(
        &mut ctx,
        &runtime,
        true,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let negative_root = ncl_object::push_root(&mut ctx, &mut negative);

    let absolute = call(&runtime, &mut ctx, "ABS", &[negative]);
    let mut absolute = absolute;
    let absolute_root = ncl_object::push_root(&mut ctx, &mut absolute);
    assert_equal(&runtime, &mut ctx, absolute, positive, "ABS(-a) = a");
    let negated = call(&runtime, &mut ctx, "-", &[positive]);
    let mut negated = negated;
    let negated_root = ncl_object::push_root(&mut ctx, &mut negated);
    assert_equal(
        &runtime,
        &mut ctx,
        negated,
        negative,
        "-a preserves the full magnitude",
    );
    let positive_signum = call(&runtime, &mut ctx, "SIGNUM", &[positive]);
    assert_equal(
        &runtime,
        &mut ctx,
        positive_signum,
        Word::fixnum(1),
        "SIGNUM(a)",
    );
    let negative_signum = call(&runtime, &mut ctx, "SIGNUM", &[negative]);
    assert_equal(
        &runtime,
        &mut ctx,
        negative_signum,
        Word::fixnum(-1),
        "SIGNUM(-a)",
    );

    assert!(ncl_object::pop_root(&mut ctx, negated_root));
    assert!(ncl_object::pop_root(&mut ctx, absolute_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_root));
    assert!(ncl_object::pop_root(&mut ctx, positive_root));
}

#[test]
fn max_min_and_eql_compare_fixnums_with_multi_limb_integers() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: MAX(17,a)=a, MIN(17,a)=17, EQL(17,a)=NIL, EQL(a,a+0)=T.
    let mut big = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let big_root = ncl_object::push_root(&mut ctx, &mut big);
    let small = Word::fixnum(17);
    let maximum = call(&runtime, &mut ctx, "MAX", &[small, big]);
    assert_equal(&runtime, &mut ctx, maximum, big, "MAX(fixnum, bignum)");
    let minimum = call(&runtime, &mut ctx, "MIN", &[small, big]);
    assert_equal(&runtime, &mut ctx, minimum, small, "MIN(fixnum, bignum)");
    assert_predicate(&runtime, &mut ctx, "EQL", &[small, big], false);
    let same_value = call(&runtime, &mut ctx, "+", &[big, Word::fixnum(0)]);
    let mut same_value = same_value;
    let same_value_root = ncl_object::push_root(&mut ctx, &mut same_value);
    assert_predicate(&runtime, &mut ctx, "EQL", &[big, same_value], true);
    assert!(ncl_object::pop_root(&mut ctx, same_value_root));
    assert!(ncl_object::pop_root(&mut ctx, big_root));
}

#[test]
fn ratio_add_sub_and_complex_arithmetic_keep_expected_values_and_types() {
    let (runtime, mut ctx) = setup();
    let mut one_third = ratio(&runtime, &mut ctx, 1, 3);
    let one_third_root = ncl_object::push_root(&mut ctx, &mut one_third);
    let mut one_sixth = ratio(&runtime, &mut ctx, 1, 6);
    let one_sixth_root = ncl_object::push_root(&mut ctx, &mut one_sixth);
    let mut half = ratio(&runtime, &mut ctx, 1, 2);
    let half_root = ncl_object::push_root(&mut ctx, &mut half);

    let sum = call(&runtime, &mut ctx, "+", &[one_third, one_sixth]);
    let mut sum = sum;
    let sum_root = ncl_object::push_root(&mut ctx, &mut sum);
    assert_predicate(&runtime, &mut ctx, "RATIONALP", &[sum], true);
    assert_predicate(&runtime, &mut ctx, "INTEGERP", &[sum], false);
    assert_equal(&runtime, &mut ctx, sum, half, "1/3 + 1/6 = 1/2");
    let difference = call(&runtime, &mut ctx, "-", &[one_third, one_sixth]);
    let mut difference = difference;
    let difference_root = ncl_object::push_root(&mut ctx, &mut difference);
    assert_predicate(&runtime, &mut ctx, "RATIONALP", &[difference], true);
    assert_equal(&runtime, &mut ctx, difference, one_sixth, "1/3 - 1/6 = 1/6");

    assert!(ncl_object::pop_root(&mut ctx, difference_root));
    assert!(ncl_object::pop_root(&mut ctx, sum_root));
    assert!(ncl_object::pop_root(&mut ctx, half_root));
    assert!(ncl_object::pop_root(&mut ctx, one_sixth_root));
    assert!(ncl_object::pop_root(&mut ctx, one_third_root));

    // SBCL 2.6.10: #C(1.0d0 2.0d0) +/-/* #C(3.0d0 -4.0d0)
    // => #C(4.0d0 -2.0d0), #C(-2.0d0 6.0d0), #C(11.0d0 2.0d0).
    let mut left = complex_floats(&mut ctx, &runtime, 1.0, 2.0);
    let left_root = ncl_object::push_root(&mut ctx, &mut left);
    let mut right = complex_floats(&mut ctx, &runtime, 3.0, -4.0);
    let right_root = ncl_object::push_root(&mut ctx, &mut right);
    let mut sum = call(&runtime, &mut ctx, "+", &[left, right]);
    let sum_root = ncl_object::push_root(&mut ctx, &mut sum);
    assert_complex_parts(&runtime, &mut ctx, sum, 4, -2);
    let mut difference = call(&runtime, &mut ctx, "-", &[left, right]);
    let difference_root = ncl_object::push_root(&mut ctx, &mut difference);
    assert_complex_parts(&runtime, &mut ctx, difference, -2, 6);
    let mut product = call(&runtime, &mut ctx, "*", &[left, right]);
    let product_root = ncl_object::push_root(&mut ctx, &mut product);
    assert_complex_parts(&runtime, &mut ctx, product, 11, 2);

    assert!(ncl_object::pop_root(&mut ctx, product_root));
    assert!(ncl_object::pop_root(&mut ctx, difference_root));
    assert!(ncl_object::pop_root(&mut ctx, sum_root));
    assert!(ncl_object::pop_root(&mut ctx, right_root));
    assert!(ncl_object::pop_root(&mut ctx, left_root));
}

#[test]
fn mod_and_rem_preserve_ratio_and_float_value_types() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: MOD(-7/3,2/3)=1/3 and REM(-7/3,2/3)=-1/3.
    let mut dividend = ratio(&runtime, &mut ctx, -7, 3);
    let dividend_root = ncl_object::push_root(&mut ctx, &mut dividend);
    let mut divisor = ratio(&runtime, &mut ctx, 2, 3);
    let divisor_root = ncl_object::push_root(&mut ctx, &mut divisor);
    let mut positive_third = ratio(&runtime, &mut ctx, 1, 3);
    let positive_third_root = ncl_object::push_root(&mut ctx, &mut positive_third);
    let mut negative_third = ratio(&runtime, &mut ctx, -1, 3);
    let negative_third_root = ncl_object::push_root(&mut ctx, &mut negative_third);
    let mut mod_ratio = call(&runtime, &mut ctx, "MOD", &[dividend, divisor]);
    let mod_ratio_root = ncl_object::push_root(&mut ctx, &mut mod_ratio);
    assert_predicate(&runtime, &mut ctx, "RATIONALP", &[mod_ratio], true);
    assert_equal(&runtime, &mut ctx, mod_ratio, positive_third, "ratio MOD");
    let mut rem_ratio = call(&runtime, &mut ctx, "REM", &[dividend, divisor]);
    let rem_ratio_root = ncl_object::push_root(&mut ctx, &mut rem_ratio);
    assert_predicate(&runtime, &mut ctx, "RATIONALP", &[rem_ratio], true);
    assert_equal(&runtime, &mut ctx, rem_ratio, negative_third, "ratio REM");

    assert!(ncl_object::pop_root(&mut ctx, rem_ratio_root));
    assert!(ncl_object::pop_root(&mut ctx, mod_ratio_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_third_root));
    assert!(ncl_object::pop_root(&mut ctx, positive_third_root));
    assert!(ncl_object::pop_root(&mut ctx, divisor_root));
    assert!(ncl_object::pop_root(&mut ctx, dividend_root));

    // SBCL 2.6.10: MOD(-7.5d0,2.0d0)=0.5d0 and REM=-1.5d0.
    let mut float_dividend = make_double(&mut ctx, &runtime, -7.5).unwrap().into();
    let float_dividend_root = ncl_object::push_root(&mut ctx, &mut float_dividend);
    let mut float_divisor = make_double(&mut ctx, &runtime, 2.0).unwrap().into();
    let float_divisor_root = ncl_object::push_root(&mut ctx, &mut float_divisor);
    let mut half = ratio(&runtime, &mut ctx, 1, 2);
    let half_root = ncl_object::push_root(&mut ctx, &mut half);
    let mut negative_three_halves = ratio(&runtime, &mut ctx, -3, 2);
    let negative_three_halves_root = ncl_object::push_root(&mut ctx, &mut negative_three_halves);
    let mut mod_float = call(&runtime, &mut ctx, "MOD", &[float_dividend, float_divisor]);
    let mod_float_root = ncl_object::push_root(&mut ctx, &mut mod_float);
    assert_predicate(&runtime, &mut ctx, "FLOATP", &[mod_float], true);
    assert_equal(&runtime, &mut ctx, mod_float, half, "float MOD");
    let mut rem_float = call(&runtime, &mut ctx, "REM", &[float_dividend, float_divisor]);
    let rem_float_root = ncl_object::push_root(&mut ctx, &mut rem_float);
    assert_predicate(&runtime, &mut ctx, "FLOATP", &[rem_float], true);
    assert_equal(
        &runtime,
        &mut ctx,
        rem_float,
        negative_three_halves,
        "float REM",
    );

    assert!(ncl_object::pop_root(&mut ctx, rem_float_root));
    assert!(ncl_object::pop_root(&mut ctx, mod_float_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_three_halves_root));
    assert!(ncl_object::pop_root(&mut ctx, half_root));
    assert!(ncl_object::pop_root(&mut ctx, float_divisor_root));
    assert!(ncl_object::pop_root(&mut ctx, float_dividend_root));
}

#[test]
fn exact_ratio_floor_and_ceiling_return_expected_remainders() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: FLOOR(7/3,2/3)=(3,1/3), CEILING=(4,-1/3).
    let mut numerator = ratio(&runtime, &mut ctx, 7, 3);
    let numerator_root = ncl_object::push_root(&mut ctx, &mut numerator);
    let mut divisor = ratio(&runtime, &mut ctx, 2, 3);
    let divisor_root = ncl_object::push_root(&mut ctx, &mut divisor);
    let mut third = ratio(&runtime, &mut ctx, 1, 3);
    let third_root = ncl_object::push_root(&mut ctx, &mut third);
    let mut negative_third = ratio(&runtime, &mut ctx, -1, 3);
    let negative_third_root = ncl_object::push_root(&mut ctx, &mut negative_third);

    let floor = call(&runtime, &mut ctx, "FLOOR", &[numerator, divisor]);
    let mut floor_remainder = ctx.values()[1];
    let floor_remainder_root = ncl_object::push_root(&mut ctx, &mut floor_remainder);
    assert_equal(
        &runtime,
        &mut ctx,
        floor,
        Word::fixnum(3),
        "ratio FLOOR quotient",
    );
    assert_equal(
        &runtime,
        &mut ctx,
        floor_remainder,
        third,
        "ratio FLOOR remainder",
    );
    let ceiling = call(&runtime, &mut ctx, "CEILING", &[numerator, divisor]);
    let mut ceiling_remainder = ctx.values()[1];
    let ceiling_remainder_root = ncl_object::push_root(&mut ctx, &mut ceiling_remainder);
    assert_equal(
        &runtime,
        &mut ctx,
        ceiling,
        Word::fixnum(4),
        "ratio CEILING quotient",
    );
    assert_equal(
        &runtime,
        &mut ctx,
        ceiling_remainder,
        negative_third,
        "ratio CEILING remainder",
    );

    assert!(ncl_object::pop_root(&mut ctx, ceiling_remainder_root));
    assert!(ncl_object::pop_root(&mut ctx, floor_remainder_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_third_root));
    assert!(ncl_object::pop_root(&mut ctx, third_root));
    assert!(ncl_object::pop_root(&mut ctx, divisor_root));
    assert!(ncl_object::pop_root(&mut ctx, numerator_root));
}

#[test]
fn multi_limb_floor_and_truncate_reconstruct_the_dividend() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: FLOOR(a,b)=TRUNCATE(a,b)=(2,12345678920987654310).
    let mut a = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let a_root = ncl_object::push_root(&mut ctx, &mut a);
    let mut b = integer_from_limbs(
        &mut ctx,
        &runtime,
        false,
        &[0xb34f_e916, 0xffff_fffd, 0xffff_ffff, 0xffff_ffff],
    );
    let b_root = ncl_object::push_root(&mut ctx, &mut b);
    let mut expected_remainder =
        integer_from_limbs(&mut ctx, &runtime, false, &[0x847f_38a6, 0xab54_a991]);
    let expected_remainder_root = ncl_object::push_root(&mut ctx, &mut expected_remainder);

    let floor = call(&runtime, &mut ctx, "FLOOR", &[a, b]);
    let mut floor_remainder = ctx.values()[1];
    let floor_remainder_root = ncl_object::push_root(&mut ctx, &mut floor_remainder);
    assert_equal(
        &runtime,
        &mut ctx,
        floor,
        Word::fixnum(2),
        "multi-limb FLOOR quotient",
    );
    assert_equal(
        &runtime,
        &mut ctx,
        floor_remainder,
        expected_remainder,
        "multi-limb FLOOR remainder",
    );
    let mut floor_product = call(&runtime, &mut ctx, "*", &[floor, b]);
    let floor_product_root = ncl_object::push_root(&mut ctx, &mut floor_product);
    let mut floor_reconstruction = call(&runtime, &mut ctx, "+", &[floor_product, floor_remainder]);
    let floor_reconstruction_root = ncl_object::push_root(&mut ctx, &mut floor_reconstruction);
    assert_equal(
        &runtime,
        &mut ctx,
        floor_reconstruction,
        a,
        "a = floor(a / b) * b + mod(a, b)",
    );

    let truncate = call(&runtime, &mut ctx, "TRUNCATE", &[a, b]);
    let mut truncate_remainder = ctx.values()[1];
    let truncate_remainder_root = ncl_object::push_root(&mut ctx, &mut truncate_remainder);
    assert_equal(
        &runtime,
        &mut ctx,
        truncate,
        Word::fixnum(2),
        "multi-limb TRUNCATE quotient",
    );
    assert_equal(
        &runtime,
        &mut ctx,
        truncate_remainder,
        expected_remainder,
        "multi-limb TRUNCATE remainder",
    );
    let mut truncate_product = call(&runtime, &mut ctx, "*", &[truncate, b]);
    let truncate_product_root = ncl_object::push_root(&mut ctx, &mut truncate_product);
    let mut truncate_reconstruction = call(
        &runtime,
        &mut ctx,
        "+",
        &[truncate_product, truncate_remainder],
    );
    let truncate_reconstruction_root =
        ncl_object::push_root(&mut ctx, &mut truncate_reconstruction);
    assert_equal(
        &runtime,
        &mut ctx,
        truncate_reconstruction,
        a,
        "a = truncate(a / b) * b + rem(a, b)",
    );

    assert!(ncl_object::pop_root(&mut ctx, truncate_reconstruction_root));
    assert!(ncl_object::pop_root(&mut ctx, truncate_product_root));
    assert!(ncl_object::pop_root(&mut ctx, truncate_remainder_root));
    assert!(ncl_object::pop_root(&mut ctx, floor_reconstruction_root));
    assert!(ncl_object::pop_root(&mut ctx, floor_product_root));
    assert!(ncl_object::pop_root(&mut ctx, floor_remainder_root));
    assert!(ncl_object::pop_root(&mut ctx, expected_remainder_root));
    assert!(ncl_object::pop_root(&mut ctx, b_root));
    assert!(ncl_object::pop_root(&mut ctx, a_root));
}
