//! Public-boundary checks for mixed fixnum and multi-limb integer arithmetic.

use ncl_object::{
    FunctionObject, ObjectError, Runtime, ThreadContext, Word, make_bignum_from_limbs, make_double,
};

fn test_value<T>(result: Result<T, ObjectError>) -> T {
    result.unwrap_or_else(|error| panic!("valid numeric fixture failed: {error:?}"))
}

fn setup() -> (Runtime, ThreadContext) {
    let runtime = test_value(Runtime::new());
    let mut ctx = ThreadContext::new();
    test_value(ctx.register(&runtime));
    test_value(ncl_conditions::register(&runtime));
    test_value(ncl_lib_numbers::register(&runtime));
    (runtime, ctx)
}

fn call(runtime: &Runtime, ctx: &mut ThreadContext, name: &str, args: &[Word]) -> Word {
    let function = runtime
        .function(ctx, "COMMON-LISP", name)
        .unwrap_or_else(|| panic!("missing COMMON-LISP function {name}"));
    let function = test_value(FunctionObject::try_from(function));
    test_value(runtime.call_builtin(ctx, function, args))
}

fn integer(ctx: &mut ThreadContext, runtime: &Runtime, negative: bool, limbs: &[u32]) -> Word {
    test_value(make_bignum_from_limbs(ctx, runtime, negative, limbs)).into()
}

fn float(ctx: &mut ThreadContext, runtime: &Runtime, value: f64) -> Word {
    test_value(make_double(ctx, runtime, value)).into()
}

fn complex_float(runtime: &Runtime, ctx: &mut ThreadContext, real: f64, imaginary: f64) -> Word {
    let mut real = float(ctx, runtime, real);
    let real_root = ncl_object::push_root(ctx, &mut real);
    let mut imaginary = float(ctx, runtime, imaginary);
    let imaginary_root = ncl_object::push_root(ctx, &mut imaginary);
    let mut complex = call(runtime, ctx, "COMPLEX", &[real, imaginary]);
    let complex_root = ncl_object::push_root(ctx, &mut complex);
    assert!(ncl_object::pop_root(ctx, complex_root));
    assert!(ncl_object::pop_root(ctx, imaginary_root));
    assert!(ncl_object::pop_root(ctx, real_root));
    complex
}

fn assert_equal(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    actual: Word,
    expected: Word,
    label: &str,
) {
    assert_eq!(
        call(runtime, ctx, "=", &[actual, expected]),
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

fn assert_float_value(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    actual: Word,
    expected: f64,
    label: &str,
) {
    let mut expected = float(ctx, runtime, expected);
    let expected_root = ncl_object::push_root(ctx, &mut expected);
    assert_predicate(runtime, ctx, "FLOATP", &[actual], true);
    assert_equal(runtime, ctx, actual, expected, label);
    assert!(ncl_object::pop_root(ctx, expected_root));
}

fn assert_complex_value(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    actual: Word,
    expected_value: (f64, f64),
) {
    let mut expected = complex_float(runtime, ctx, expected_value.0, expected_value.1);
    let expected_root = ncl_object::push_root(ctx, &mut expected);
    assert_predicate(runtime, ctx, "COMPLEXP", &[actual], true);
    assert_equal(runtime, ctx, actual, expected, "complex result");
    assert!(ncl_object::pop_root(ctx, expected_root));
}

#[test]
fn mixed_bignum_addition_is_commutative_and_matches_sbcl() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: a=680564733841876926939094893764770990802; a+17=...0819.
    let mut a = integer(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let a_root = ncl_object::push_root(&mut ctx, &mut a);
    let small = Word::fixnum(17);
    let mut expected = integer(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ae3, 0xab54_a98c, 0, 0, 2],
    );
    let expected_root = ncl_object::push_root(&mut ctx, &mut expected);

    let mut a_plus_small = call(&runtime, &mut ctx, "+", &[a, small]);
    let a_plus_small_root = ncl_object::push_root(&mut ctx, &mut a_plus_small);
    assert_equal(&runtime, &mut ctx, a_plus_small, expected, "a + 17");
    let mut small_plus_a = call(&runtime, &mut ctx, "+", &[small, a]);
    let small_plus_a_root = ncl_object::push_root(&mut ctx, &mut small_plus_a);
    assert_equal(&runtime, &mut ctx, small_plus_a, expected, "17 + a");
    assert_equal(
        &runtime,
        &mut ctx,
        a_plus_small,
        small_plus_a,
        "addition is commutative",
    );

    assert!(ncl_object::pop_root(&mut ctx, small_plus_a_root));
    assert!(ncl_object::pop_root(&mut ctx, a_plus_small_root));
    assert!(ncl_object::pop_root(&mut ctx, expected_root));
    assert!(ncl_object::pop_root(&mut ctx, a_root));
}

#[test]
fn mixed_bignum_subtraction_and_multiplication_match_sbcl_and_reconstruct() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: a-17=680564733841876926939094893764770990785,
    // 17-a is its negative, and a*17=11569600475311907757964613194001106843634.
    let mut a = integer(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let a_root = ncl_object::push_root(&mut ctx, &mut a);
    let small = Word::fixnum(17);
    let mut difference = integer(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ac1, 0xab54_a98c, 0, 0, 2],
    );
    let difference_root = ncl_object::push_root(&mut ctx, &mut difference);
    let mut negative_difference = integer(
        &mut ctx,
        &runtime,
        true,
        &[0xeb1f_0ac1, 0xab54_a98c, 0, 0, 2],
    );
    let negative_difference_root = ncl_object::push_root(&mut ctx, &mut negative_difference);
    let mut product = integer(
        &mut ctx,
        &runtime,
        false,
        &[0x9d0f_b7f2, 0x609f_425b, 0xb, 0, 0x22],
    );
    let product_root = ncl_object::push_root(&mut ctx, &mut product);

    let mut a_minus_small = call(&runtime, &mut ctx, "-", &[a, small]);
    let a_minus_small_root = ncl_object::push_root(&mut ctx, &mut a_minus_small);
    assert_equal(&runtime, &mut ctx, a_minus_small, difference, "a - 17");
    let mut small_minus_a = call(&runtime, &mut ctx, "-", &[small, a]);
    let small_minus_a_root = ncl_object::push_root(&mut ctx, &mut small_minus_a);
    assert_equal(
        &runtime,
        &mut ctx,
        small_minus_a,
        negative_difference,
        "17 - a",
    );
    let mut recovered_a = call(&runtime, &mut ctx, "+", &[a_minus_small, small]);
    let recovered_a_root = ncl_object::push_root(&mut ctx, &mut recovered_a);
    assert_equal(&runtime, &mut ctx, recovered_a, a, "(a - 17) + 17 = a");
    let mut recovered_small = call(&runtime, &mut ctx, "+", &[small_minus_a, a]);
    let recovered_small_root = ncl_object::push_root(&mut ctx, &mut recovered_small);
    assert_equal(
        &runtime,
        &mut ctx,
        recovered_small,
        small,
        "(17 - a) + a = 17",
    );

    let mut a_times_small = call(&runtime, &mut ctx, "*", &[a, small]);
    let a_times_small_root = ncl_object::push_root(&mut ctx, &mut a_times_small);
    assert_equal(&runtime, &mut ctx, a_times_small, product, "a * 17");
    let mut small_times_a = call(&runtime, &mut ctx, "*", &[small, a]);
    let small_times_a_root = ncl_object::push_root(&mut ctx, &mut small_times_a);
    assert_equal(&runtime, &mut ctx, small_times_a, product, "17 * a");

    assert!(ncl_object::pop_root(&mut ctx, small_times_a_root));
    assert!(ncl_object::pop_root(&mut ctx, a_times_small_root));
    assert!(ncl_object::pop_root(&mut ctx, recovered_small_root));
    assert!(ncl_object::pop_root(&mut ctx, recovered_a_root));
    assert!(ncl_object::pop_root(&mut ctx, small_minus_a_root));
    assert!(ncl_object::pop_root(&mut ctx, a_minus_small_root));
    assert!(ncl_object::pop_root(&mut ctx, product_root));
    assert!(ncl_object::pop_root(&mut ctx, negative_difference_root));
    assert!(ncl_object::pop_root(&mut ctx, difference_root));
    assert!(ncl_object::pop_root(&mut ctx, a_root));
}

#[test]
fn mixed_bignum_exact_division_reconstructs_integer_results() {
    let (runtime, mut ctx) = setup();
    // SBCL 2.6.10: a/2=340282366920938463469547446882385495401, 0/a=0, a/a=1.
    let mut a = integer(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let a_root = ncl_object::push_root(&mut ctx, &mut a);
    let mut expected_half = integer(
        &mut ctx,
        &runtime,
        false,
        &[0x758f_8569, 0x55aa_54c6, 0, 0, 1],
    );
    let expected_half_root = ncl_object::push_root(&mut ctx, &mut expected_half);
    let two = Word::fixnum(2);
    let zero = Word::fixnum(0);

    let mut a_over_two = call(&runtime, &mut ctx, "/", &[a, two]);
    let a_over_two_root = ncl_object::push_root(&mut ctx, &mut a_over_two);
    assert_predicate(&runtime, &mut ctx, "INTEGERP", &[a_over_two], true);
    assert_equal(&runtime, &mut ctx, a_over_two, expected_half, "a / 2");
    let mut recovered_a = call(&runtime, &mut ctx, "*", &[a_over_two, two]);
    let recovered_a_root = ncl_object::push_root(&mut ctx, &mut recovered_a);
    assert_equal(&runtime, &mut ctx, recovered_a, a, "(a / 2) * 2 = a");

    let mut zero_over_a = call(&runtime, &mut ctx, "/", &[zero, a]);
    let zero_over_a_root = ncl_object::push_root(&mut ctx, &mut zero_over_a);
    assert_equal(&runtime, &mut ctx, zero_over_a, zero, "0 / a");
    let mut recovered_zero = call(&runtime, &mut ctx, "*", &[zero_over_a, a]);
    let recovered_zero_root = ncl_object::push_root(&mut ctx, &mut recovered_zero);
    assert_equal(&runtime, &mut ctx, recovered_zero, zero, "(0 / a) * a = 0");

    let mut a_over_a = call(&runtime, &mut ctx, "/", &[a, a]);
    let a_over_a_root = ncl_object::push_root(&mut ctx, &mut a_over_a);
    assert_equal(&runtime, &mut ctx, a_over_a, Word::fixnum(1), "a / a");
    let mut recovered_a_again = call(&runtime, &mut ctx, "*", &[a_over_a, a]);
    let recovered_a_again_root = ncl_object::push_root(&mut ctx, &mut recovered_a_again);
    assert_equal(&runtime, &mut ctx, recovered_a_again, a, "(a / a) * a = a");

    assert!(ncl_object::pop_root(&mut ctx, recovered_a_again_root));
    assert!(ncl_object::pop_root(&mut ctx, a_over_a_root));
    assert!(ncl_object::pop_root(&mut ctx, recovered_zero_root));
    assert!(ncl_object::pop_root(&mut ctx, zero_over_a_root));
    assert!(ncl_object::pop_root(&mut ctx, recovered_a_root));
    assert!(ncl_object::pop_root(&mut ctx, a_over_two_root));
    assert!(ncl_object::pop_root(&mut ctx, expected_half_root));
    assert!(ncl_object::pop_root(&mut ctx, a_root));
}

#[test]
fn mixed_bignum_ordering_and_eql_cover_both_operand_orders() {
    let (runtime, mut ctx) = setup();
    let mut a = integer(
        &mut ctx,
        &runtime,
        false,
        &[0xeb1f_0ad2, 0xab54_a98c, 0, 0, 2],
    );
    let a_root = ncl_object::push_root(&mut ctx, &mut a);
    let small = Word::fixnum(17);
    // SBCL 2.6.10: < 17 a, > a 17, and EQL(17,a) are T, T, and NIL.
    for (name, args, expected) in [
        ("<", [small, a], true),
        ("<", [a, small], false),
        (">", [a, small], true),
        (">", [small, a], false),
        ("<=", [small, a], true),
        ("<=", [a, small], false),
        (">=", [a, small], true),
        (">=", [small, a], false),
        ("=", [a, small], false),
        ("EQL", [a, small], false),
    ] {
        assert_predicate(&runtime, &mut ctx, name, &args, expected);
    }
    let mut same_value = call(&runtime, &mut ctx, "+", &[a, Word::fixnum(0)]);
    let same_value_root = ncl_object::push_root(&mut ctx, &mut same_value);
    assert_predicate(&runtime, &mut ctx, "EQL", &[a, same_value], true);
    let maximum_forward = call(&runtime, &mut ctx, "MAX", &[small, a]);
    assert_equal(&runtime, &mut ctx, maximum_forward, a, "MAX(17,a)");
    let maximum_reverse = call(&runtime, &mut ctx, "MAX", &[a, small]);
    assert_equal(&runtime, &mut ctx, maximum_reverse, a, "MAX(a,17)");
    let minimum_forward = call(&runtime, &mut ctx, "MIN", &[small, a]);
    assert_equal(&runtime, &mut ctx, minimum_forward, small, "MIN(17,a)");
    let minimum_reverse = call(&runtime, &mut ctx, "MIN", &[a, small]);
    assert_equal(&runtime, &mut ctx, minimum_reverse, small, "MIN(a,17)");
    assert!(ncl_object::pop_root(&mut ctx, same_value_root));
    assert!(ncl_object::pop_root(&mut ctx, a_root));
}

#[test]
fn float_integer_addition_and_multiplication_dispatch_both_orders() {
    let (runtime, mut ctx) = setup();
    let mut one_point_five = float(&mut ctx, &runtime, 1.5);
    let float_root = ncl_object::push_root(&mut ctx, &mut one_point_five);
    let two = Word::fixnum(2);
    let mut float_plus_integer = call(&runtime, &mut ctx, "+", &[one_point_five, two]);
    let result_root = ncl_object::push_root(&mut ctx, &mut float_plus_integer);
    assert_float_value(&runtime, &mut ctx, float_plus_integer, 3.5, "1.5d0 + 2");
    let mut integer_plus_float = call(&runtime, &mut ctx, "+", &[two, one_point_five]);
    let reverse_root = ncl_object::push_root(&mut ctx, &mut integer_plus_float);
    assert_float_value(&runtime, &mut ctx, integer_plus_float, 3.5, "2 + 1.5d0");
    let mut float_times_integer = call(&runtime, &mut ctx, "*", &[one_point_five, two]);
    let product_root = ncl_object::push_root(&mut ctx, &mut float_times_integer);
    assert_float_value(&runtime, &mut ctx, float_times_integer, 3.0, "1.5d0 * 2");
    let mut integer_times_float = call(&runtime, &mut ctx, "*", &[two, one_point_five]);
    let reverse_product_root = ncl_object::push_root(&mut ctx, &mut integer_times_float);
    assert_float_value(&runtime, &mut ctx, integer_times_float, 3.0, "2 * 1.5d0");
    assert!(ncl_object::pop_root(&mut ctx, reverse_product_root));
    assert!(ncl_object::pop_root(&mut ctx, product_root));
    assert!(ncl_object::pop_root(&mut ctx, reverse_root));
    assert!(ncl_object::pop_root(&mut ctx, result_root));
    assert!(ncl_object::pop_root(&mut ctx, float_root));
}

#[test]
fn complex_scalar_subtraction_and_multiplication_match_sbcl() {
    let (runtime, mut ctx) = setup();
    let mut three_four = complex_float(&runtime, &mut ctx, 3.0, 4.0);
    let three_four_root = ncl_object::push_root(&mut ctx, &mut three_four);
    let mut one = float(&mut ctx, &runtime, 1.0);
    let one_root = ncl_object::push_root(&mut ctx, &mut one);
    let mut complex_minus_float = call(&runtime, &mut ctx, "-", &[three_four, one]);
    let minus_root = ncl_object::push_root(&mut ctx, &mut complex_minus_float);
    assert_complex_value(&runtime, &mut ctx, complex_minus_float, (2.0, 4.0));

    let mut one_two = complex_float(&runtime, &mut ctx, 1.0, 2.0);
    let one_two_root = ncl_object::push_root(&mut ctx, &mut one_two);
    let mut five = float(&mut ctx, &runtime, 5.0);
    let five_root = ncl_object::push_root(&mut ctx, &mut five);
    let mut float_minus_complex = call(&runtime, &mut ctx, "-", &[five, one_two]);
    let reverse_root = ncl_object::push_root(&mut ctx, &mut float_minus_complex);
    assert_complex_value(&runtime, &mut ctx, float_minus_complex, (4.0, -2.0));

    let mut three = float(&mut ctx, &runtime, 3.0);
    let three_root = ncl_object::push_root(&mut ctx, &mut three);
    let mut complex_times_float = call(&runtime, &mut ctx, "*", &[one_two, three]);
    let product_root = ncl_object::push_root(&mut ctx, &mut complex_times_float);
    assert_complex_value(&runtime, &mut ctx, complex_times_float, (3.0, 6.0));

    assert!(ncl_object::pop_root(&mut ctx, product_root));
    assert!(ncl_object::pop_root(&mut ctx, three_root));
    assert!(ncl_object::pop_root(&mut ctx, reverse_root));
    assert!(ncl_object::pop_root(&mut ctx, five_root));
    assert!(ncl_object::pop_root(&mut ctx, one_two_root));
    assert!(ncl_object::pop_root(&mut ctx, minus_root));
    assert!(ncl_object::pop_root(&mut ctx, one_root));
    assert!(ncl_object::pop_root(&mut ctx, three_four_root));
}

#[test]
fn complex_division_dispatches_complex_complex_and_float_pairs() {
    let (runtime, mut ctx) = setup();
    let mut one_two = complex_float(&runtime, &mut ctx, 1.0, 2.0);
    let numerator_root = ncl_object::push_root(&mut ctx, &mut one_two);
    let mut three_four = complex_float(&runtime, &mut ctx, 3.0, 4.0);
    let denominator_root = ncl_object::push_root(&mut ctx, &mut three_four);

    let mut complex_over_complex = call(&runtime, &mut ctx, "/", &[one_two, three_four]);
    let complex_pair_root = ncl_object::push_root(&mut ctx, &mut complex_over_complex);
    assert_complex_value(&runtime, &mut ctx, complex_over_complex, (0.44, 0.08));

    let mut two = float(&mut ctx, &runtime, 2.0);
    let two_root = ncl_object::push_root(&mut ctx, &mut two);
    let mut complex_over_float = call(&runtime, &mut ctx, "/", &[one_two, two]);
    let complex_float_root = ncl_object::push_root(&mut ctx, &mut complex_over_float);
    assert_complex_value(&runtime, &mut ctx, complex_over_float, (0.5, 1.0));
    let mut one = float(&mut ctx, &runtime, 1.0);
    let one_root = ncl_object::push_root(&mut ctx, &mut one);
    let mut float_over_complex = call(&runtime, &mut ctx, "/", &[one, three_four]);
    let float_complex_root = ncl_object::push_root(&mut ctx, &mut float_over_complex);
    assert_complex_value(&runtime, &mut ctx, float_over_complex, (0.12, -0.16));

    assert!(ncl_object::pop_root(&mut ctx, float_complex_root));
    assert!(ncl_object::pop_root(&mut ctx, one_root));
    assert!(ncl_object::pop_root(&mut ctx, complex_float_root));
    assert!(ncl_object::pop_root(&mut ctx, two_root));
    assert!(ncl_object::pop_root(&mut ctx, complex_pair_root));
    assert!(ncl_object::pop_root(&mut ctx, denominator_root));
    assert!(ncl_object::pop_root(&mut ctx, numerator_root));
}

#[test]
fn exact_ratio_and_small_integer_division_and_zero_exponent_match_sbcl() {
    let (runtime, mut ctx) = setup();
    let mut one_half = call(&runtime, &mut ctx, "/", &[Word::fixnum(1), Word::fixnum(2)]);
    let half_root = ncl_object::push_root(&mut ctx, &mut one_half);
    let mut ratio_quotient = call(&runtime, &mut ctx, "/", &[one_half, Word::fixnum(3)]);
    let quotient_root = ncl_object::push_root(&mut ctx, &mut ratio_quotient);
    let mut one_sixth = call(&runtime, &mut ctx, "/", &[Word::fixnum(1), Word::fixnum(6)]);
    let sixth_root = ncl_object::push_root(&mut ctx, &mut one_sixth);
    assert_predicate(&runtime, &mut ctx, "RATIONALP", &[ratio_quotient], true);
    assert_equal(&runtime, &mut ctx, ratio_quotient, one_sixth, "(1/2) / 3");

    let mut small_quotient = call(&runtime, &mut ctx, "/", &[Word::fixnum(8), Word::fixnum(2)]);
    let small_root = ncl_object::push_root(&mut ctx, &mut small_quotient);
    assert_predicate(&runtime, &mut ctx, "INTEGERP", &[small_quotient], true);
    assert_equal(&runtime, &mut ctx, small_quotient, Word::fixnum(4), "8 / 2");
    let exponent_zero = call(
        &runtime,
        &mut ctx,
        "EXPT",
        &[Word::fixnum(2), Word::fixnum(0)],
    );
    assert_equal(&runtime, &mut ctx, exponent_zero, Word::fixnum(1), "2 ** 0");
    assert!(ncl_object::pop_root(&mut ctx, small_root));
    assert!(ncl_object::pop_root(&mut ctx, sixth_root));
    assert!(ncl_object::pop_root(&mut ctx, quotient_root));
    assert!(ncl_object::pop_root(&mut ctx, half_root));
}

#[test]
fn eql_dispatches_equal_ratio_float_complex_and_eq_words() {
    let (runtime, mut ctx) = setup();
    let mut ratio_left = call(&runtime, &mut ctx, "/", &[Word::fixnum(1), Word::fixnum(3)]);
    let left_ratio_root = ncl_object::push_root(&mut ctx, &mut ratio_left);
    let mut ratio_right = call(&runtime, &mut ctx, "/", &[Word::fixnum(2), Word::fixnum(6)]);
    let right_ratio_root = ncl_object::push_root(&mut ctx, &mut ratio_right);
    assert_predicate(&runtime, &mut ctx, "EQL", &[ratio_left, ratio_right], true);

    let mut float_left = float(&mut ctx, &runtime, 1.5);
    let left_float_root = ncl_object::push_root(&mut ctx, &mut float_left);
    let mut float_right = float(&mut ctx, &runtime, 1.5);
    let right_float_root = ncl_object::push_root(&mut ctx, &mut float_right);
    assert_predicate(&runtime, &mut ctx, "EQL", &[float_left, float_right], true);

    let mut complex_left = complex_float(&runtime, &mut ctx, 1.0, 2.0);
    let left_complex_root = ncl_object::push_root(&mut ctx, &mut complex_left);
    let mut complex_right = complex_float(&runtime, &mut ctx, 1.0, 2.0);
    let right_complex_root = ncl_object::push_root(&mut ctx, &mut complex_right);
    assert_predicate(
        &runtime,
        &mut ctx,
        "EQL",
        &[complex_left, complex_right],
        true,
    );
    assert_predicate(
        &runtime,
        &mut ctx,
        "EQ",
        &[Word::fixnum(17), Word::fixnum(17)],
        true,
    );
    assert_predicate(
        &runtime,
        &mut ctx,
        "/=",
        &[Word::fixnum(17), Word::fixnum(17)],
        false,
    );

    assert!(ncl_object::pop_root(&mut ctx, right_complex_root));
    assert!(ncl_object::pop_root(&mut ctx, left_complex_root));
    assert!(ncl_object::pop_root(&mut ctx, right_float_root));
    assert!(ncl_object::pop_root(&mut ctx, left_float_root));
    assert!(ncl_object::pop_root(&mut ctx, right_ratio_root));
    assert!(ncl_object::pop_root(&mut ctx, left_ratio_root));
}
