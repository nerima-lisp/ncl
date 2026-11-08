use super::super::compound::BuiltinType;
use super::super::numeric::{NumericDomain, NumericRange};
use super::SubtypeRelation;
pub(super) const fn range_to_named(range: NumericRange, name: BuiltinType) -> SubtypeRelation {
    use BuiltinType::{
        Atom, Complex, DoubleFloat, Float, Integer, Number, Rational, Real, SingleFloat,
    };
    let relation = match range.domain() {
        NumericDomain::Integer => match name {
            Integer | Rational | Real | Number | Atom => Some(true),
            Float | SingleFloat | DoubleFloat | Complex => Some(false),
            _ => None, // check-added-lines: allow(wildcard) unclassified numeric names remain unknown
        },
        NumericDomain::SingleFloat => match name {
            SingleFloat | Float | Real | Number | Atom => Some(true),
            Integer | Rational | Complex => Some(false),
            _ => None, // check-added-lines: allow(wildcard) unclassified numeric names remain unknown
        },
        NumericDomain::DoubleFloat => match name {
            DoubleFloat | Float | Real | Number | Atom => Some(true),
            Integer | Rational | Complex => Some(false),
            _ => None, // check-added-lines: allow(wildcard) unclassified numeric names remain unknown
        },
    };
    match relation {
        Some(true) => SubtypeRelation::Yes,
        Some(false) => SubtypeRelation::No,
        None => SubtypeRelation::Unknown,
    }
}
pub(super) fn named_to_range(name: BuiltinType, range: NumericRange) -> SubtypeRelation {
    use BuiltinType::{DoubleFloat, Float, Integer, Number, Real, SingleFloat};
    let full_domain = range.lower().is_none() && range.upper().is_none();
    match (name, range.domain()) {
        (Integer, NumericDomain::Integer) if full_domain => SubtypeRelation::Yes,
        (SingleFloat, NumericDomain::SingleFloat) | (DoubleFloat, NumericDomain::DoubleFloat)
            if range.covers_domain() =>
        {
            SubtypeRelation::Yes
        }
        (SingleFloat, NumericDomain::SingleFloat) | (DoubleFloat, NumericDomain::DoubleFloat) => {
            SubtypeRelation::No
        }
        (Integer | Float | Real | Number, _) => SubtypeRelation::No,
        _ => SubtypeRelation::Unknown, // check-added-lines: allow(wildcard) unhandled builtin and domain pairs remain unknown
    }
}
pub(super) fn numeric_relation(sub: NumericRange, sup: NumericRange) -> SubtypeRelation {
    if sub.domain() != sup.domain() {
        if sub.domain() == NumericDomain::Integer || sup.domain() == NumericDomain::Integer {
            SubtypeRelation::No
        } else {
            SubtypeRelation::Unknown
        }
    } else if sup.contains_range(sub) {
        SubtypeRelation::Yes
    } else {
        SubtypeRelation::No
    }
}
