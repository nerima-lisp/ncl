use std::collections::HashMap;

use super::super::numeric::NumericDomain;
use super::super::{ClTypeKind, TypeArena};
use super::member::{member_disjoint_from_named, member_disjoint_from_range};
use super::named::{are_disjoint, named_relation};
use super::{SubtypeRelation, combine_all, combine_any, subtype_inner};
use crate::ids::TypeId;
pub(super) fn disjoint_relation(
    arena: &TypeArena,
    left: TypeId,
    right: TypeId,
    cache: &mut HashMap<(TypeId, TypeId), SubtypeRelation>,
) -> SubtypeRelation {
    if left == right {
        return SubtypeRelation::No;
    }
    let (Some(left_node), Some(right_node)) = (arena.get(left), arena.get(right)) else {
        return SubtypeRelation::Unknown;
    };
    match (left_node.kind(), right_node.kind()) {
        (ClTypeKind::Named(left), ClTypeKind::Named(right)) => {
            if are_disjoint(*left, *right) {
                SubtypeRelation::Yes
            } else if named_relation(*left, *right) == SubtypeRelation::Yes
                || named_relation(*right, *left) == SubtypeRelation::Yes
            {
                SubtypeRelation::No
            } else {
                SubtypeRelation::Unknown
            }
        }
        (ClTypeKind::NumericRange(left), ClTypeKind::NumericRange(right)) => {
            if left.domain() != right.domain()
                && left.domain() != NumericDomain::Integer
                && right.domain() != NumericDomain::Integer
            {
                SubtypeRelation::Unknown
            } else if left.domain() != right.domain() || left.is_disjoint(*right) {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::No
            }
        }
        (_, ClTypeKind::Complement(excluded)) => subtype_inner(arena, left, *excluded, cache),
        (_, ClTypeKind::Union(members)) => combine_all(
            members
                .iter()
                .map(|member| disjoint_relation(arena, left, *member, cache)),
        ),
        (_, ClTypeKind::Intersection(members)) => match combine_any(
            members
                .iter()
                .map(|member| disjoint_relation(arena, left, *member, cache)),
        ) {
            SubtypeRelation::Yes => SubtypeRelation::Yes,
            SubtypeRelation::No | SubtypeRelation::Unknown => SubtypeRelation::Unknown,
        },
        (ClTypeKind::Named(name), ClTypeKind::Member(values)) => {
            member_disjoint_from_named(values, *name)
        }
        (ClTypeKind::NumericRange(range), ClTypeKind::Member(values)) => {
            member_disjoint_from_range(values, *range)
        }
        _ => SubtypeRelation::Unknown, // check-added-lines: allow(wildcard) unhandled type pairs remain unknown
    }
}
