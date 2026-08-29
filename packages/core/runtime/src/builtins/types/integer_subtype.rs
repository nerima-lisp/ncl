#![allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn integer_spec_is_subtype(
    subtype_arguments: &[Value],
    supertype_arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let subtype_lower = subtype_arguments
        .first()
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let subtype_upper = subtype_arguments
        .get(1)
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let supertype_lower = supertype_arguments
        .first()
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let supertype_upper = supertype_arguments
        .get(1)
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();

    let subtype_empty = subtype_lower
        .zip(subtype_upper)
        .is_some_and(|(lower, upper)| lower > upper);
    let supertype_empty = supertype_lower
        .zip(supertype_upper)
        .is_some_and(|(lower, upper)| lower > upper);
    if subtype_empty {
        return Ok(true);
    }
    if supertype_empty {
        return Ok(false);
    }

    let lower_ok = match (subtype_lower, supertype_lower) {
        (_, None) => true,
        (Some(subtype), Some(supertype)) => subtype >= supertype,
        (None, Some(_)) => false,
    };
    let upper_ok = match (subtype_upper, supertype_upper) {
        (_, None) => true,
        (Some(subtype), Some(supertype)) => subtype <= supertype,
        (None, Some(_)) => false,
    };
    Ok(lower_ok && upper_ok)
}

pub(super) fn named_integer_is_subtype(
    subtype_name: &str,
    supertype_arguments: &[Value],
) -> Result<bool, RuntimeError> {
    if subtype_name == "BIT" {
        return integer_spec_is_subtype(
            &[Value::Integer(0), Value::Integer(1)],
            supertype_arguments,
        );
    }
    if matches!(subtype_name, "INTEGER" | "FIXNUM" | "BIGNUM") {
        let lower = supertype_arguments
            .first()
            .map(|bound| integer_type_bound("subtypep", bound))
            .transpose()?
            .flatten();
        let upper = supertype_arguments
            .get(1)
            .map(|bound| integer_type_bound("subtypep", bound))
            .transpose()?
            .flatten();
        return Ok(lower.is_none() && upper.is_none());
    }
    Ok(false)
}
