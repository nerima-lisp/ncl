use std::error::Error;

use super::{
    BuiltinType, MemberValue, NumericBound, NumericDomain, NumericRange, NumericRangeError,
    NumericValue, SubtypeRelation, TypeArena, form, named,
};

const fn integer(value: i64) -> NumericValue {
    NumericValue::Integer(value)
}

const fn bound(value: NumericValue, inclusive: bool) -> NumericBound {
    if inclusive {
        NumericBound::inclusive(value)
    } else {
        NumericBound::exclusive(value)
    }
}

#[test]
fn numeric_ranges_reject_mixed_domains_nan_and_empty_endpoints() {
    // CLHS 4.2.3 defines INTEGER bounds; empty open intervals contain no values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert!(matches!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(NumericBound::inclusive(NumericValue::SingleFloat(
                1.0_f32.to_bits()
            ))),
            None,
        ),
        Err(NumericRangeError::DomainMismatch {
            expected: NumericDomain::Integer,
            actual: NumericDomain::SingleFloat,
        })
    ));
    assert_eq!(
        NumericRange::new(
            NumericDomain::DoubleFloat,
            Some(NumericBound::inclusive(NumericValue::DoubleFloat(
                f64::NAN.to_bits()
            ))),
            None,
        ),
        Err(NumericRangeError::NaNBoundary)
    );
    assert_eq!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(bound(integer(0), false)),
            Some(bound(integer(1), false)),
        ),
        Err(NumericRangeError::EmptyInterval)
    );
    assert_eq!(
        NumericRange::new(
            NumericDomain::Integer,
            Some(bound(integer(3), false)),
            Some(bound(integer(3), true)),
        ),
        Err(NumericRangeError::EmptyInterval)
    );
    assert_eq!(
        NumericRange::new(
            NumericDomain::DoubleFloat,
            Some(bound(NumericValue::DoubleFloat(0.0_f64.to_bits()), false)),
            Some(bound(
                NumericValue::DoubleFloat(f64::from_bits(1).to_bits()),
                false,
            )),
        ),
        Err(NumericRangeError::EmptyInterval)
    );
}

#[test]
fn signed_zero_and_inclusive_bounds_keep_numeric_membership_results() -> Result<(), Box<dyn Error>>
{
    let zero_range = NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            (-0.0_f32).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            (-0.0_f32).to_bits(),
        ))),
    )?;
    assert_eq!(
        zero_range.lower().map(NumericBound::value),
        Some(NumericValue::SingleFloat(0))
    );
    let upper = NumericRange::new(
        NumericDomain::Integer,
        Some(bound(integer(1), true)),
        Some(bound(integer(3), true)),
    )?;
    let mut arena = TypeArena::new(form(101))?;
    let upper_id = arena.numeric_range(upper)?;
    let one = arena.member(vec![MemberValue::Integer(1)])?;
    let four = arena.member(vec![MemberValue::Integer(4)])?;
    let character = arena.member(vec![MemberValue::Character('1')])?;
    let zero = arena.numeric_range(zero_range)?;
    let zero_member = arena.member(vec![MemberValue::SingleFloat(0.0_f32.to_bits())])?;

    // CLHS numeric type specifiers distinguish integer and float values and preserve interval endpoints.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(one, upper_id)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(four, upper_id)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(character, upper_id)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(zero_member, zero)?, SubtypeRelation::Yes);
    let integer = named(&mut arena, BuiltinType::Integer)?;
    assert_eq!(arena.subtype(upper_id, integer)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn numeric_range_relations_cover_unbounded_and_cross_format_intervals() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(102))?;
    let all_integers =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    let positive_integers = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(bound(integer(0), false)),
        None,
    )?)?;
    let all_single =
        arena.numeric_range(NumericRange::new(NumericDomain::SingleFloat, None, None)?)?;
    let all_double =
        arena.numeric_range(NumericRange::new(NumericDomain::DoubleFloat, None, None)?)?;
    let single = named(&mut arena, BuiltinType::SingleFloat)?;
    let double = named(&mut arena, BuiltinType::DoubleFloat)?;

    // CLHS 4.2.3 distinguishes single-float and double-float type specifiers.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(
        arena.subtype(positive_integers, all_integers)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(all_integers, positive_integers)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(all_single, single)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(single, all_single)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(all_double, double)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(all_single, all_double)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(
        arena.subtype(all_integers, all_single)?,
        SubtypeRelation::No
    );
    Ok(())
}

#[test]
fn range_subtyping_respects_open_endpoints_and_canonical_signed_zero() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(107))?;
    let closed = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(bound(integer(1), true)),
        Some(bound(integer(3), true)),
    )?)?;
    let open = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(bound(integer(1), false)),
        Some(bound(integer(3), false)),
    )?)?;
    let positive = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(bound(integer(0), false)),
        None,
    )?)?;
    let all_integers =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    let negative_zero = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            (-0.0_f32).to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            (-0.0_f32).to_bits(),
        ))),
    )?)?;
    let positive_zero = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
    )?)?;

    // CLHS numeric ranges include or exclude each stated boundary according to its endpoint.
    // https://www.lispworks.com/documentation/HyperSpec/Body/04_bc.htm
    assert_eq!(arena.subtype(open, closed)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(closed, open)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(positive, all_integers)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(all_integers, positive)?, SubtypeRelation::No);
    assert_eq!(negative_zero, positive_zero);
    assert_eq!(
        arena.subtype(negative_zero, positive_zero)?,
        SubtypeRelation::Yes
    );
    Ok(())
}

#[test]
fn integer_range_membership_respects_each_open_or_closed_endpoint() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(111))?;
    let interval = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::exclusive(integer(0))),
        Some(NumericBound::inclusive(integer(2))),
    )?)?;
    let zero = arena.member(vec![MemberValue::Integer(0)])?;
    let one = arena.member(vec![MemberValue::Integer(1)])?;
    let two = arena.member(vec![MemberValue::Integer(2)])?;
    let three = arena.member(vec![MemberValue::Integer(3)])?;

    // CLHS interval designators make the lower bound exclusive and upper bound inclusive.
    // https://www.lispworks.com/documentation/HyperSpec/Body/12_af.htm
    assert_eq!(arena.subtype(zero, interval)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(one, interval)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(two, interval)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(three, interval)?, SubtypeRelation::No);
    Ok(())
}

#[test]
fn adjacent_float_bounds_select_the_smallest_positive_representable_value()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(112))?;
    let single_zero_bits = 0.0_f32.to_bits();
    let smallest_single_bits = f32::from_bits(1).to_bits();
    let single_range = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::exclusive(NumericValue::SingleFloat(
            single_zero_bits,
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            smallest_single_bits,
        ))),
    )?)?;
    let single_zero = arena.member(vec![MemberValue::SingleFloat(single_zero_bits)])?;
    let smallest_single = arena.member(vec![MemberValue::SingleFloat(smallest_single_bits)])?;
    let double_zero_bits = 0.0_f64.to_bits();
    let smallest_double_bits = f64::from_bits(1).to_bits();
    let double_range = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::exclusive(NumericValue::DoubleFloat(
            double_zero_bits,
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            smallest_double_bits,
        ))),
    )?)?;
    let double_zero = arena.member(vec![MemberValue::DoubleFloat(double_zero_bits)])?;
    let smallest_double = arena.member(vec![MemberValue::DoubleFloat(smallest_double_bits)])?;

    // CLHS interval designators exclude the lower endpoint and include the upper endpoint.
    // https://www.lispworks.com/documentation/HyperSpec/Body/12_af.htm
    assert_eq!(
        arena.subtype(single_zero, single_range)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(smallest_single, single_range)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(single_range, smallest_single)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(double_zero, double_range)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(smallest_double, double_range)?,
        SubtypeRelation::Yes
    );
    assert_eq!(
        arena.subtype(double_range, smallest_double)?,
        SubtypeRelation::Yes
    );
    Ok(())
}
