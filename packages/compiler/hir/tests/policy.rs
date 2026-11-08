//! Optimization policy decisions at the supported quality and safety levels.

use std::error::Error;

use ncl_hir::{OptimizationPolicy, OptimizationQuality, OptimizationQualityError, SubtypeRelation};

#[test]
fn optimization_qualities_accept_zero_through_three_and_reject_four() {
    // CLHS 3.8 defines the optimization qualities and their standard values.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_h.htm
    assert_eq!(
        OptimizationQuality::new(0).map(OptimizationQuality::value),
        Ok(0)
    );
    assert_eq!(
        OptimizationQuality::try_from(3).map(OptimizationQuality::value),
        Ok(3)
    );
    assert_eq!(
        OptimizationQuality::new(4),
        Err(OptimizationQualityError::OutOfRange(4))
    );
    assert_eq!(
        OptimizationQualityError::OutOfRange(4).to_string(),
        "optimization quality 4 is outside 0 through 3"
    );
}

#[test]
fn safety_controls_check_retention_without_changing_other_qualities() -> Result<(), Box<dyn Error>>
{
    // CLHS 3.8 defines SAFETY as an independent optimization quality.
    // https://www.lispworks.com/documentation/HyperSpec/Body/03_h.htm
    let default = OptimizationPolicy::default();
    assert_eq!(default.speed().value(), 3);
    assert_eq!(default.safety().value(), 3);
    assert_eq!(default.debug().value(), 3);
    assert_eq!(default.space().value(), 3);

    let speed = OptimizationQuality::new(1)?;
    let safety = OptimizationQuality::new(0)?;
    let debug = OptimizationQuality::new(2)?;
    let space = OptimizationQuality::new(1)?;
    let policy = OptimizationPolicy::new(speed, safety, debug, space);
    assert_eq!(policy.speed().value(), 1);
    assert_eq!(policy.safety().value(), 0);
    assert_eq!(policy.debug().value(), 2);
    assert_eq!(policy.space().value(), 1);

    let checked = policy.with_safety(OptimizationQuality::new(1)?);
    assert_eq!(checked.speed(), speed);
    assert_eq!(checked.safety().value(), 1);
    assert_eq!(checked.debug(), debug);
    assert_eq!(checked.space(), space);
    assert_eq!(
        policy
            .with_speed(OptimizationQuality::new(2)?)
            .speed()
            .value(),
        2
    );
    assert_eq!(
        policy
            .with_debug(OptimizationQuality::new(0)?)
            .debug()
            .value(),
        0
    );
    assert_eq!(
        policy
            .with_space(OptimizationQuality::new(3)?)
            .space()
            .value(),
        3
    );

    for (subtype, expected) in [
        (SubtypeRelation::Yes, false),
        (SubtypeRelation::No, true),
        (SubtypeRelation::Unknown, true),
    ] {
        assert!(!policy.requires_type_check(subtype, false));
        assert_eq!(checked.requires_type_check(subtype, false), expected);
        assert!(!checked.requires_type_check(subtype, true));
    }
    for (call_is_proven, equivalent_guard) in [(false, false), (false, true), (true, false)] {
        assert!(!policy.requires_arity_check(call_is_proven, equivalent_guard));
        assert_eq!(
            checked.requires_arity_check(call_is_proven, equivalent_guard),
            !call_is_proven && !equivalent_guard
        );
        assert_eq!(
            checked.requires_keyword_check(call_is_proven, equivalent_guard),
            !call_is_proven && !equivalent_guard
        );
        assert_eq!(
            checked.requires_array_bounds_check(call_is_proven, equivalent_guard),
            !call_is_proven && !equivalent_guard
        );
    }
    assert!(!policy.diagnoses_invalid_call(true));
    assert!(!checked.diagnoses_invalid_call(false));
    assert!(checked.diagnoses_invalid_call(true));
    Ok(())
}
