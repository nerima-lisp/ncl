//! Conservative subtype reasoning for the compiler type lattice.

#[path = "relation/compound.rs"]
mod compound;
#[path = "relation/disjoint.rs"]
mod disjoint;
#[path = "relation/member.rs"]
mod member;
#[path = "relation/named.rs"]
mod named;
#[path = "relation/numeric.rs"]
mod numeric;

use std::collections::HashMap;

use self::compound::{array_relation, array_to_named, function_relation, multiple_values_relation};
use self::disjoint::disjoint_relation;
use self::member::{
    member_to_member, member_to_named, member_to_range, member_to_type, member_to_union,
    named_to_member, range_to_member,
};
use self::named::named_relation;
use self::numeric::{named_to_range, numeric_relation, range_to_named};
use super::compound::BuiltinType;
use super::{ClTypeError, ClTypeKind, TypeArena};
use crate::ids::TypeId;

/// The result of a subtype query.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SubtypeRelation {
    /// The relation is proven.
    Yes,
    /// The relation is disproven.
    No,
    /// The available type facts cannot decide the relation.
    Unknown,
}

/// Determines whether every value in sub is also in sup.
///
/// `Unknown` is returned whenever the model lacks enough information to prove
/// either answer. Callers may remove checks only for `Yes`.
///
/// # Errors
///
/// Returns `UnknownType` if either identity is not present in the arena.
pub fn subtype(
    arena: &TypeArena,
    sub: TypeId,
    sup: TypeId,
) -> Result<SubtypeRelation, ClTypeError> {
    arena.get(sub).ok_or(ClTypeError::UnknownType(sub))?;
    arena.get(sup).ok_or(ClTypeError::UnknownType(sup))?;
    Ok(subtype_inner(arena, sub, sup, &mut HashMap::new()))
}

fn subtype_inner(
    arena: &TypeArena,
    sub: TypeId,
    sup: TypeId,
    cache: &mut HashMap<(TypeId, TypeId), SubtypeRelation>,
) -> SubtypeRelation {
    if sub == sup {
        return SubtypeRelation::Yes;
    }
    if sub == arena.empty() || sup == arena.any() {
        return SubtypeRelation::Yes;
    }
    if sub == arena.any() || sup == arena.empty() {
        return SubtypeRelation::No;
    }
    if let Some(result) = cache.get(&(sub, sup)) {
        return *result;
    }
    let Some(source_type) = arena.get(sub) else {
        return SubtypeRelation::Unknown;
    };
    let Some(target_type) = arena.get(sup) else {
        return SubtypeRelation::Unknown;
    };
    let result = match (source_type.kind(), target_type.kind()) {
        (ClTypeKind::Named(sub), ClTypeKind::Named(sup)) => named_relation(*sub, *sup),
        (ClTypeKind::NumericRange(sub), ClTypeKind::NumericRange(sup)) => {
            numeric_relation(*sub, *sup)
        }
        (ClTypeKind::NumericRange(range), ClTypeKind::Named(name)) => range_to_named(*range, *name),
        (ClTypeKind::Named(name), ClTypeKind::NumericRange(range)) => named_to_range(*name, *range),
        (ClTypeKind::Member(values), ClTypeKind::Member(sup_values)) => {
            member_to_member(values, sup_values)
        }
        (ClTypeKind::Member(values), ClTypeKind::Named(name)) => member_to_named(values, *name),
        (ClTypeKind::Member(values), ClTypeKind::NumericRange(range)) => {
            member_to_range(values, *range)
        }
        (ClTypeKind::Member(values), ClTypeKind::Union(supers)) => {
            member_to_union(arena, values, supers)
        }
        (ClTypeKind::Named(name), ClTypeKind::Member(values)) => named_to_member(*name, values),
        (ClTypeKind::NumericRange(range), ClTypeKind::Member(values)) => {
            range_to_member(*range, values)
        }
        (ClTypeKind::Member(values), _) => member_to_type(arena, values, sup),
        (ClTypeKind::Union(subs), _) => {
            combine_all(subs.iter().map(|id| subtype_inner(arena, *id, sup, cache)))
        }
        (_, ClTypeKind::Union(sups)) => {
            let result = combine_any(sups.iter().map(|id| subtype_inner(arena, sub, *id, cache)));
            if result == SubtypeRelation::No {
                SubtypeRelation::Unknown
            } else {
                result
            }
        }
        (ClTypeKind::Intersection(subs), _) => {
            intersection_to_super(subs.iter().map(|id| subtype_inner(arena, *id, sup, cache)))
        }
        (_, ClTypeKind::Intersection(sups)) => {
            combine_all(sups.iter().map(|id| subtype_inner(arena, sub, *id, cache)))
        }
        (ClTypeKind::Complement(source_inner), ClTypeKind::Complement(target_inner)) => {
            subtype_inner(arena, *target_inner, *source_inner, cache)
        }
        (_, ClTypeKind::Complement(excluded)) => disjoint_relation(arena, sub, *excluded, cache),
        (
            ClTypeKind::Cons {
                car: source_head_type,
                cdr: source_tail_type,
            },
            ClTypeKind::Cons {
                car: target_head_type,
                cdr: target_tail_type,
            },
        ) => combine_all([
            subtype_inner(arena, *source_head_type, *target_head_type, cache),
            subtype_inner(arena, *source_tail_type, *target_tail_type, cache),
        ]),
        (ClTypeKind::Cons { .. }, ClTypeKind::Named(name)) => {
            named_relation(BuiltinType::Cons, *name)
        }
        (ClTypeKind::Array(sub), ClTypeKind::Array(sup)) => array_relation(arena, sub, sup, cache),
        (ClTypeKind::Array(array), ClTypeKind::Named(name)) => array_to_named(array, *name),
        (ClTypeKind::Function(sub), ClTypeKind::Function(sup)) => {
            function_relation(arena, sub, sup, cache)
        }
        (ClTypeKind::Function(_), ClTypeKind::Named(BuiltinType::Function)) => SubtypeRelation::Yes,
        (ClTypeKind::MultipleValues(sub), ClTypeKind::MultipleValues(sup)) => {
            multiple_values_relation(arena, sub, sup, cache)
        }
        (ClTypeKind::Class(sub), ClTypeKind::Class(sup)) => {
            if sub == sup {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::Unknown
            }
        }
        _ => SubtypeRelation::Unknown, // check-added-lines: allow(wildcard) unmodeled type pairs remain unknown
    };
    cache.insert((sub, sup), result);
    result
}

fn combine_all(results: impl IntoIterator<Item = SubtypeRelation>) -> SubtypeRelation {
    let mut unknown = false;
    for result in results {
        match result {
            SubtypeRelation::No => return SubtypeRelation::No,
            SubtypeRelation::Unknown => unknown = true,
            SubtypeRelation::Yes => {}
        }
    }
    if unknown {
        SubtypeRelation::Unknown
    } else {
        SubtypeRelation::Yes
    }
}

fn combine_any(results: impl IntoIterator<Item = SubtypeRelation>) -> SubtypeRelation {
    let mut unknown = false;
    for result in results {
        match result {
            SubtypeRelation::Yes => return SubtypeRelation::Yes,
            SubtypeRelation::Unknown => unknown = true,
            SubtypeRelation::No => {}
        }
    }
    if unknown {
        SubtypeRelation::Unknown
    } else {
        SubtypeRelation::No
    }
}

fn intersection_to_super(results: impl IntoIterator<Item = SubtypeRelation>) -> SubtypeRelation {
    if results
        .into_iter()
        .any(|result| result == SubtypeRelation::Yes)
    {
        SubtypeRelation::Yes
    } else {
        SubtypeRelation::Unknown
    }
}
