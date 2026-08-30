use super::exceeds_exact_bignum_digit_cap;

#[test]
fn exceeds_exact_bignum_digit_cap_fast_rejects_a_non_round_value_far_over_the_cap() {
    let value = ibig::IBig::from(9) * ibig::IBig::from(10).pow(150_000);
    assert_eq!(value.to_string().len(), 150_001);
    assert!(exceeds_exact_bignum_digit_cap(&value));
}

#[test]
fn exceeds_exact_bignum_digit_cap_fast_accepts_a_non_round_value_far_under_the_cap() {
    let value = ibig::IBig::from(9) * ibig::IBig::from(10).pow(50_000);
    assert_eq!(value.to_string().len(), 50_001);
    assert!(!exceeds_exact_bignum_digit_cap(&value));
}

#[test]
fn exceeds_exact_bignum_digit_cap_accepts_zero() {
    assert!(!exceeds_exact_bignum_digit_cap(&ibig::IBig::from(0)));
}
