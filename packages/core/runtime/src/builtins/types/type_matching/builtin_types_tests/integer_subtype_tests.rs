use crate::Value;
use crate::builtins::types::integer_subtype::{integer_spec_is_subtype, named_integer_is_subtype};

#[test]
fn integer_spec_is_subtype_reports_empty_ranges_and_missing_bounds() {
    let empty_subtype = integer_spec_is_subtype(
        &[Value::Integer(10), Value::Integer(0)],
        &[Value::Integer(0), Value::Integer(20)],
    )
    .unwrap_or_else(|error| panic!("valid integer bounds: {error}"));
    assert!(
        empty_subtype,
        "an empty integer range is a subtype of anything"
    );

    let empty_supertype = integer_spec_is_subtype(
        &[Value::Integer(0), Value::Integer(5)],
        &[Value::Integer(10), Value::Integer(0)],
    )
    .unwrap_or_else(|error| panic!("valid integer bounds: {error}"));
    assert!(
        !empty_supertype,
        "a non-empty range cannot be a subtype of an empty range"
    );

    let unbounded_subtype_lower =
        integer_spec_is_subtype(&[], &[Value::Integer(0), Value::Integer(20)])
            .unwrap_or_else(|error| panic!("valid integer bounds: {error}"));
    assert!(
        !unbounded_subtype_lower,
        "an unbounded-below subtype cannot fit inside a lower-bounded supertype"
    );

    let unbounded_subtype_upper = integer_spec_is_subtype(
        &[Value::Integer(0)],
        &[Value::Integer(0), Value::Integer(20)],
    )
    .unwrap_or_else(|error| panic!("valid integer bounds: {error}"));
    assert!(
        !unbounded_subtype_upper,
        "an unbounded-above subtype cannot fit inside an upper-bounded supertype"
    );

    let fully_unbounded_supertype =
        integer_spec_is_subtype(&[Value::Integer(5), Value::Integer(10)], &[])
            .unwrap_or_else(|error| panic!("valid integer bounds: {error}"));
    assert!(
        fully_unbounded_supertype,
        "any bounded range fits inside a supertype with no bounds at all"
    );
}

#[test]
fn named_integer_is_subtype_recognizes_bit_and_unbounded_integer_names() {
    let bit_within_byte =
        named_integer_is_subtype("BIT", &[Value::Integer(0), Value::Integer(255)])
            .unwrap_or_else(|error| panic!("BIT is a valid subtype name: {error}"));
    assert!(bit_within_byte);

    for name in ["INTEGER", "FIXNUM", "BIGNUM"] {
        let unbounded = named_integer_is_subtype(name, &[])
            .unwrap_or_else(|error| panic!("valid named integer type: {error}"));
        assert!(
            unbounded,
            "{name} with no supertype bounds must be a subtype"
        );

        let bounded = named_integer_is_subtype(name, &[Value::Integer(0), Value::Integer(10)])
            .unwrap_or_else(|error| panic!("valid named integer type: {error}"));
        assert!(
            !bounded,
            "{name} cannot be a subtype of a bounded integer range"
        );
    }

    let unrelated = named_integer_is_subtype("STRING", &[])
        .unwrap_or_else(|error| panic!("unrelated names are decidable: {error}"));
    assert!(
        !unrelated,
        "STRING is never treated as a named integer subtype"
    );
}
