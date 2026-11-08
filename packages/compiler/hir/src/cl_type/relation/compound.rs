use std::collections::HashMap;

use super::super::TypeArena;
use super::super::compound::{ArrayDimensions, ArrayType, BuiltinType, FunctionType};
use super::{SubtypeRelation, combine_all, subtype_inner};
use crate::ids::TypeId;
pub(super) fn multiple_values_relation(
    arena: &TypeArena,
    sub: &[TypeId],
    sup: &[TypeId],
    cache: &mut HashMap<(TypeId, TypeId), SubtypeRelation>,
) -> SubtypeRelation {
    if sub.len() == sup.len() {
        combine_all(
            sub.iter()
                .zip(sup)
                .map(|(sub, sup)| subtype_inner(arena, *sub, *sup, cache)),
        )
    } else {
        SubtypeRelation::No
    }
}
pub(super) fn array_to_named(array: &ArrayType, name: BuiltinType) -> SubtypeRelation {
    use BuiltinType::{Array, Sequence, SimpleArray, Vector};
    let rank = match &array.dimensions {
        ArrayDimensions::Any => None,
        ArrayDimensions::Rank(rank) => Some(*rank),
        ArrayDimensions::Axes(axes) => Some(axes.len()),
    };
    match name {
        Array => SubtypeRelation::Yes,
        SimpleArray if array.simple => SubtypeRelation::Yes,
        Vector | Sequence if rank == Some(1) => SubtypeRelation::Yes,
        _ => SubtypeRelation::Unknown, // check-added-lines: allow(wildcard) unmodeled array shapes remain unknown
    }
}
pub(super) fn array_relation(
    arena: &TypeArena,
    sub: &ArrayType,
    sup: &ArrayType,
    cache: &mut HashMap<(TypeId, TypeId), SubtypeRelation>,
) -> SubtypeRelation {
    let element = match (sub.element_type, sup.element_type) {
        (_, None) => SubtypeRelation::Yes,
        (Some(sub), Some(sup)) => subtype_inner(arena, sub, sup, cache),
        (None, Some(_)) => SubtypeRelation::No,
    };
    let dimensions = dimensions_relation(&sub.dimensions, &sup.dimensions);
    let simple = if sup.simple && !sub.simple {
        SubtypeRelation::No
    } else {
        SubtypeRelation::Yes
    };
    combine_all([element, dimensions, simple])
}
fn dimensions_relation(sub: &ArrayDimensions, sup: &ArrayDimensions) -> SubtypeRelation {
    match (sub, sup) {
        (_, ArrayDimensions::Any) => SubtypeRelation::Yes,
        (ArrayDimensions::Any, _) => SubtypeRelation::No,
        (ArrayDimensions::Rank(sub), ArrayDimensions::Rank(sup)) => {
            if sub == sup {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::No
            }
        }
        (ArrayDimensions::Axes(sub), ArrayDimensions::Rank(sup)) => {
            if sub.len() == *sup {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::No
            }
        }
        (ArrayDimensions::Rank(sub), ArrayDimensions::Axes(sup)) => {
            if *sub == sup.len() && sup.iter().all(Option::is_none) {
                SubtypeRelation::Yes
            } else {
                SubtypeRelation::No
            }
        }
        (ArrayDimensions::Axes(sub), ArrayDimensions::Axes(sup)) => {
            if sub.len() != sup.len() {
                return SubtypeRelation::No;
            }
            combine_all(sub.iter().zip(sup).map(|(sub, sup)| match (sub, sup) {
                (_, None) => SubtypeRelation::Yes,
                (Some(sub), Some(sup)) if sub == sup => SubtypeRelation::Yes,
                (Some(_) | None, Some(_)) => SubtypeRelation::No,
            }))
        }
    }
}
pub(super) fn function_relation(
    arena: &TypeArena,
    sub: &FunctionType,
    sup: &FunctionType,
    cache: &mut HashMap<(TypeId, TypeId), SubtypeRelation>,
) -> SubtypeRelation {
    if sub.parameters != sup.parameters {
        return SubtypeRelation::Unknown;
    }
    subtype_inner(arena, sub.returns, sup.returns, cache)
}
