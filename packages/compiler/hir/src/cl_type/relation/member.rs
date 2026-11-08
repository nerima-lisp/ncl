use super::super::compound::{BuiltinType, MemberValue};
use super::super::numeric::{NumericRange, NumericValue};
use super::super::{ClTypeKind, TypeArena};
use super::{SubtypeRelation, combine_all, combine_any};
use crate::ids::TypeId;
pub(super) fn member_to_member(sub: &[MemberValue], sup: &[MemberValue]) -> SubtypeRelation {
    combine_all(sub.iter().map(|value| member_value_in_member(*value, sup)))
}
pub(super) fn member_to_union(
    arena: &TypeArena,
    values: &[MemberValue],
    supers: &[TypeId],
) -> SubtypeRelation {
    combine_all(values.iter().map(|value| {
        combine_any(
            supers
                .iter()
                .map(|supertype| member_value_subtype(arena, *value, *supertype)),
        )
    }))
}
fn member_value_subtype(
    arena: &TypeArena,
    value: MemberValue,
    supertype: TypeId,
) -> SubtypeRelation {
    let Some(supertype) = arena.get(supertype) else {
        return SubtypeRelation::Unknown;
    };
    match supertype.kind() {
        ClTypeKind::Any => SubtypeRelation::Yes,
        ClTypeKind::Empty => SubtypeRelation::No,
        ClTypeKind::Named(name) => member_value_to_named(value, *name),
        ClTypeKind::NumericRange(range) => match value {
            MemberValue::Integer(value) => contains_value(*range, NumericValue::Integer(value)),
            MemberValue::SingleFloat(value) => {
                contains_value(*range, NumericValue::SingleFloat(value))
            }
            MemberValue::DoubleFloat(value) => {
                contains_value(*range, NumericValue::DoubleFloat(value))
            }
            MemberValue::OpaqueObject { .. } => SubtypeRelation::Unknown,
            MemberValue::Nil
            | MemberValue::True
            | MemberValue::Character(_)
            | MemberValue::Symbol(_) => SubtypeRelation::No,
        },
        ClTypeKind::Member(values) => member_value_in_member(value, values),
        ClTypeKind::Union(members) => combine_any(
            members
                .iter()
                .map(|member| member_value_subtype(arena, value, *member)),
        ),
        ClTypeKind::Intersection(members) => combine_all(
            members
                .iter()
                .map(|member| member_value_subtype(arena, value, *member)),
        ),
        ClTypeKind::Complement(excluded) => {
            invert_overlap(member_value_subtype(arena, value, *excluded))
        }
        ClTypeKind::Cons { .. }
        | ClTypeKind::Array(_)
        | ClTypeKind::Function(_)
        | ClTypeKind::MultipleValues(_)
        | ClTypeKind::Class(_)
        | ClTypeKind::Opaque { .. } => {
            if matches!(value, MemberValue::OpaqueObject { .. }) {
                SubtypeRelation::Unknown
            } else {
                SubtypeRelation::No
            }
        }
    }
}
pub(super) fn member_value_in_member(
    value: MemberValue,
    candidates: &[MemberValue],
) -> SubtypeRelation {
    if candidates.iter().any(|candidate| candidate.eql(value)) {
        return SubtypeRelation::Yes;
    }
    if matches!(value, MemberValue::Symbol(_))
        && candidates
            .iter()
            .any(|candidate| matches!(candidate, MemberValue::Nil | MemberValue::True))
    {
        return SubtypeRelation::Unknown;
    }
    if matches!(value, MemberValue::Nil | MemberValue::True)
        && candidates
            .iter()
            .any(|candidate| matches!(candidate, MemberValue::Symbol(_)))
    {
        return SubtypeRelation::Unknown;
    }
    SubtypeRelation::No
}
pub(super) fn member_to_named(values: &[MemberValue], name: BuiltinType) -> SubtypeRelation {
    combine_all(
        values
            .iter()
            .map(|value| member_value_to_named(*value, name)),
    )
}
fn member_value_to_named(value: MemberValue, name: BuiltinType) -> SubtypeRelation {
    use BuiltinType::{
        Atom, Boolean, Character, DoubleFloat, Float, Integer, Keyword, List, Null, Number,
        Rational, Real, Sequence, SingleFloat, Symbol,
    };
    match value {
        MemberValue::Nil if matches!(name, Null | Symbol | List | Sequence | Boolean | Atom) => {
            SubtypeRelation::Yes
        }
        MemberValue::True if matches!(name, Symbol | Boolean | Atom) => SubtypeRelation::Yes,
        MemberValue::Integer(_) if matches!(name, Integer | Rational | Real | Number | Atom) => {
            SubtypeRelation::Yes
        }
        MemberValue::Character(_) if name == Character => SubtypeRelation::Yes,
        MemberValue::SingleFloat(_)
            if matches!(name, SingleFloat | Float | Real | Number | Atom) =>
        {
            SubtypeRelation::Yes
        }
        MemberValue::DoubleFloat(_)
            if matches!(name, DoubleFloat | Float | Real | Number | Atom) =>
        {
            SubtypeRelation::Yes
        }
        MemberValue::Symbol(_) if matches!(name, Symbol | Atom) => SubtypeRelation::Yes,
        MemberValue::Symbol(_) if matches!(name, Boolean | Keyword | List | Null | Sequence) => {
            SubtypeRelation::Unknown
        }
        MemberValue::OpaqueObject { .. } => SubtypeRelation::Unknown,
        MemberValue::Nil
        | MemberValue::True
        | MemberValue::Integer(_)
        | MemberValue::Character(_)
        | MemberValue::SingleFloat(_)
        | MemberValue::DoubleFloat(_)
        | MemberValue::Symbol(_) => SubtypeRelation::No,
    }
}
pub(super) fn member_to_range(values: &[MemberValue], range: NumericRange) -> SubtypeRelation {
    combine_all(values.iter().map(|value| match value {
        MemberValue::Integer(value) => contains_value(range, NumericValue::Integer(*value)),
        MemberValue::SingleFloat(value) => contains_value(range, NumericValue::SingleFloat(*value)),
        MemberValue::DoubleFloat(value) => contains_value(range, NumericValue::DoubleFloat(*value)),
        MemberValue::OpaqueObject { .. } => SubtypeRelation::Unknown,
        MemberValue::Nil
        | MemberValue::True
        | MemberValue::Character(_)
        | MemberValue::Symbol(_) => SubtypeRelation::No,
    }))
}
fn contains_value(range: NumericRange, value: NumericValue) -> SubtypeRelation {
    match range.contains_value(value) {
        Some(true) => SubtypeRelation::Yes,
        Some(false) => SubtypeRelation::No,
        None => SubtypeRelation::Unknown,
    }
}
pub(super) fn named_to_member(name: BuiltinType, values: &[MemberValue]) -> SubtypeRelation {
    match name {
        BuiltinType::Null => {
            if values.iter().any(|value| value.eql(MemberValue::Nil)) {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::No
            }
        }
        BuiltinType::Boolean => {
            if values.iter().any(|value| value.eql(MemberValue::Nil))
                && values.iter().any(|value| value.eql(MemberValue::True))
            {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::No
            }
        }
        BuiltinType::Symbol => SubtypeRelation::No,
        _ => SubtypeRelation::Unknown, // check-added-lines: allow(wildcard) unclassified named types remain unknown
    }
}
pub(super) fn range_to_member(range: NumericRange, values: &[MemberValue]) -> SubtypeRelation {
    let Some(value) = range.singleton() else {
        return SubtypeRelation::No;
    };
    let member = match value {
        NumericValue::Integer(value) => MemberValue::Integer(value),
        NumericValue::SingleFloat(value) => MemberValue::SingleFloat(value),
        NumericValue::DoubleFloat(value) => MemberValue::DoubleFloat(value),
    };
    if values.iter().any(|value| value.eql(member)) {
        SubtypeRelation::Yes
    } else {
        SubtypeRelation::No
    }
}
pub(super) fn member_disjoint_from_named(
    values: &[MemberValue],
    name: BuiltinType,
) -> SubtypeRelation {
    invert_overlap(combine_any(
        values
            .iter()
            .map(|value| member_value_to_named(*value, name)),
    ))
}
pub(super) fn member_disjoint_from_range(
    values: &[MemberValue],
    range: NumericRange,
) -> SubtypeRelation {
    invert_overlap(combine_any(values.iter().map(|value| match value {
        MemberValue::Integer(value) => contains_value(range, NumericValue::Integer(*value)),
        MemberValue::SingleFloat(value) => contains_value(range, NumericValue::SingleFloat(*value)),
        MemberValue::DoubleFloat(value) => contains_value(range, NumericValue::DoubleFloat(*value)),
        MemberValue::OpaqueObject { .. } => SubtypeRelation::Unknown,
        MemberValue::Nil
        | MemberValue::True
        | MemberValue::Character(_)
        | MemberValue::Symbol(_) => SubtypeRelation::No,
    })))
}
pub(super) const fn invert_overlap(overlap: SubtypeRelation) -> SubtypeRelation {
    match overlap {
        SubtypeRelation::Yes => SubtypeRelation::No,
        SubtypeRelation::No => SubtypeRelation::Yes,
        SubtypeRelation::Unknown => SubtypeRelation::Unknown,
    }
}
