use super::super::*;

#[test]
fn numeric_ranges_cover_named_formats_and_order_both_interval_directions()
-> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(123))?;
    let integer = named(&mut arena, BuiltinType::Integer)?;
    let single = named(&mut arena, BuiltinType::SingleFloat)?;
    let double = named(&mut arena, BuiltinType::DoubleFloat)?;
    let rational = named(&mut arena, BuiltinType::Rational)?;
    let complex = named(&mut arena, BuiltinType::Complex)?;
    let string = named(&mut arena, BuiltinType::String)?;
    let full_integer =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    let upper_bounded_integer = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        None,
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
    )?)?;
    let bounded_integer = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(0))),
        Some(NumericBound::inclusive(NumericValue::Integer(10))),
    )?)?;
    let full_single =
        arena.numeric_range(NumericRange::new(NumericDomain::SingleFloat, None, None)?)?;
    let bounded_single = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let full_double =
        arena.numeric_range(NumericRange::new(NumericDomain::DoubleFloat, None, None)?)?;
    let bounded_double = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            0.0_f64.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(
            1.0_f64.to_bits(),
        ))),
    )?)?;

    // CLHS defines the standard numeric type families and their interval bounds.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_intege.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_float.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_complex.htm
    // docs/src/design/hir.md: unresolved relations remain Unknown.
    assert_eq!(arena.subtype(integer, full_integer)?, SubtypeRelation::Yes);
    assert_eq!(
        arena.subtype(integer, bounded_integer)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(single, full_single)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(single, bounded_single)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(double, full_double)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(double, bounded_double)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(bounded_single, integer)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_single, rational)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(bounded_single, complex)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_single, string)?,
        SubtypeRelation::Unknown
    );
    assert_eq!(arena.subtype(bounded_double, integer)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_double, rational)?,
        SubtypeRelation::No
    );
    assert_eq!(arena.subtype(bounded_double, complex)?, SubtypeRelation::No);
    assert_eq!(
        arena.subtype(bounded_double, string)?,
        SubtypeRelation::Unknown
    );

    assert_float_interval_subtyping(&mut arena)?;

    assert_eq!(
        arena.subtype(full_integer, bounded_integer)?,
        SubtypeRelation::No
    );
    assert_eq!(
        arena.subtype(full_integer, upper_bounded_integer)?,
        SubtypeRelation::No
    );
    Ok(())
}

fn assert_float_interval_subtyping(arena: &mut TypeArena) -> Result<(), Box<dyn Error>> {
    let low = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            0.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
    )?)?;
    let high = arena.numeric_range(NumericRange::new(
        NumericDomain::SingleFloat,
        Some(NumericBound::exclusive(NumericValue::SingleFloat(
            1.0_f32.to_bits(),
        ))),
        Some(NumericBound::inclusive(NumericValue::SingleFloat(
            2.0_f32.to_bits(),
        ))),
    )?)?;
    let not_low = arena.complement(low)?;
    let not_high = arena.complement(high)?;
    assert_eq!(arena.subtype(low, not_high)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(high, not_low)?, SubtypeRelation::Yes);
    Ok(())
}

#[test]
fn double_float_singleton_ranges_match_only_their_eql_member() -> Result<(), Box<dyn Error>> {
    let mut arena = TypeArena::new(form(129))?;
    let value = 0.5_f64.to_bits();
    let range = arena.numeric_range(NumericRange::new(
        NumericDomain::DoubleFloat,
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(value))),
        Some(NumericBound::inclusive(NumericValue::DoubleFloat(value))),
    )?)?;
    let matching = arena.member(vec![MemberValue::DoubleFloat(value)])?;
    let other = arena.member(vec![MemberValue::DoubleFloat(1.0_f64.to_bits())])?;

    // A singleton float type contains only its EQL value.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_float.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(arena.subtype(range, matching)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(range, other)?, SubtypeRelation::No);
    Ok(())
}

#[test]
fn nonsingleton_ranges_and_finite_members_keep_exact_overlap_results() -> Result<(), Box<dyn Error>>
{
    let mut arena = TypeArena::new(form(124))?;
    let all_integers =
        arena.numeric_range(NumericRange::new(NumericDomain::Integer, None, None)?)?;
    let interval = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(2))),
    )?)?;
    let singleton = arena.numeric_range(NumericRange::new(
        NumericDomain::Integer,
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
        Some(NumericBound::inclusive(NumericValue::Integer(1))),
    )?)?;
    let one = arena.member(vec![MemberValue::Integer(1)])?;
    let two = arena.member(vec![MemberValue::Integer(2)])?;
    let not_one = arena.complement(one)?;
    let not_interval = arena.complement(interval)?;

    // CLHS MEMBER is the set of listed EQL values; numeric interval endpoints are inclusive here.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_member.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_eql.htm
    assert_eq!(arena.subtype(all_integers, one)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(interval, one)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(singleton, one)?, SubtypeRelation::Yes);
    assert_eq!(arena.subtype(singleton, two)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(interval, not_one)?, SubtypeRelation::No);
    assert_eq!(arena.subtype(all_integers, not_one)?, SubtypeRelation::No);
    // The unbounded INTEGER range overlaps every bounded INTEGER interval.
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_intege.htm
    // https://www.lispworks.com/documentation/HyperSpec/Body/t_not.htm
    assert_eq!(
        arena.subtype(all_integers, not_interval)?,
        SubtypeRelation::No
    );
    Ok(())
}
