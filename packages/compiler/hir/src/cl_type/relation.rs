//! Conservative subtype reasoning for the compiler type lattice.

use std::collections::HashMap;

use super::compound::{ArrayDimensions, ArrayType, BuiltinType, FunctionType, MemberValue};
use super::numeric::{NumericDomain, NumericRange, NumericValue};
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
    let (Some(source_type), Some(target_type)) = (arena.get(sub), arena.get(sup)) else {
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
        _ => SubtypeRelation::Unknown,
    };
    cache.insert((sub, sup), result);
    result
}

fn multiple_values_relation(
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

fn named_relation(sub: BuiltinType, sup: BuiltinType) -> SubtypeRelation {
    if sub == sup || is_supertype(sup, sub) {
        SubtypeRelation::Yes
    } else if is_supertype(sub, sup) || are_disjoint(sub, sup) {
        SubtypeRelation::No
    } else {
        SubtypeRelation::Unknown
    }
}

const fn is_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    is_numeric_supertype(sup, sub)
        || is_sequence_supertype(sup, sub)
        || is_other_supertype(sup, sub)
}

#[allow(clippy::unnested_or_patterns)]
const fn is_numeric_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    use BuiltinType::{
        Bignum, Complex, DoubleFloat, Fixnum, Float, Integer, LongFloat, Number, Ratio, Rational,
        Real, ShortFloat, SingleFloat,
    };
    matches!(
        (sup, sub),
        (
            Number,
            Real | Rational
                | Integer
                | Fixnum
                | Bignum
                | Ratio
                | Float
                | ShortFloat
                | SingleFloat
                | DoubleFloat
                | LongFloat
                | Complex
        ) | (
            Real,
            Rational
                | Integer
                | Fixnum
                | Bignum
                | Ratio
                | Float
                | ShortFloat
                | SingleFloat
                | DoubleFloat
                | LongFloat
        ) | (Rational, Integer | Fixnum | Bignum | Ratio)
            | (Integer, Fixnum | Bignum)
            | (Float, ShortFloat | SingleFloat | DoubleFloat | LongFloat)
    )
}

#[allow(clippy::unnested_or_patterns)]
const fn is_sequence_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    use BuiltinType::{
        Array, BaseString, BitVector, Cons, List, Null, Sequence, SimpleArray, SimpleBaseString,
        SimpleBitVector, SimpleString, SimpleVector, String, Vector,
    };
    matches!(
        (sup, sub),
        (
            Sequence,
            List | Cons
                | Null
                | Vector
                | String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | BitVector
                | SimpleBitVector
                | SimpleVector
        ) | (
            Array,
            Vector
                | String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | BitVector
                | SimpleBitVector
                | SimpleVector
                | SimpleArray
        ) | (
            Vector,
            String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | BitVector
                | SimpleBitVector
                | SimpleVector
        ) | (List, Cons | Null)
            | (String, BaseString | SimpleString | SimpleBaseString)
            | (BaseString | SimpleString, SimpleBaseString)
            | (BitVector, SimpleBitVector)
            | (
                SimpleArray,
                SimpleBitVector | SimpleVector | SimpleString | SimpleBaseString
            )
    )
}

#[allow(clippy::unnested_or_patterns)]
const fn is_other_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    use BuiltinType::{
        Array, Atom, BaseChar, BaseString, Bignum, BitVector, Boolean, Character, CompiledFunction,
        Complex, DoubleFloat, ExtendedChar, Fixnum, Float, Function, HashTable, Integer, Keyword,
        LongFloat, Null, Number, Package, RandomState, Ratio, Rational, Real, Restart, ShortFloat,
        SimpleArray, SimpleBaseString, SimpleBitVector, SimpleString, SimpleVector, SingleFloat,
        StandardChar, Stream, String, Structure, Symbol, Vector,
    };
    matches!(
        (sup, sub),
        (Function, CompiledFunction)
            | (Symbol, Null | Keyword | Boolean)
            | (Boolean, Null)
            | (Character, BaseChar | StandardChar | ExtendedChar)
            | (BaseChar, StandardChar)
            | (
                Atom,
                Null | Boolean
                    | Symbol
                    | Keyword
                    | Package
                    | HashTable
                    | RandomState
                    | Restart
                    | Function
                    | CompiledFunction
                    | Number
                    | Real
                    | Rational
                    | Integer
                    | Fixnum
                    | Bignum
                    | Ratio
                    | Float
                    | ShortFloat
                    | SingleFloat
                    | DoubleFloat
                    | LongFloat
                    | Complex
                    | Character
                    | BaseChar
                    | StandardChar
                    | ExtendedChar
                    | String
                    | BaseString
                    | SimpleString
                    | SimpleBaseString
                    | BitVector
                    | SimpleBitVector
                    | Vector
                    | SimpleVector
                    | Array
                    | SimpleArray
                    | Stream
                    | Structure
            )
    )
}

fn are_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    known_disjoint(left, right) || known_disjoint(right, left)
}

#[allow(clippy::unnested_or_patterns)]
fn known_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    use BuiltinType::{
        Array, Atom, BaseChar, BaseString, BitVector, Boolean, Character, CompiledFunction, Cons,
        ExtendedChar, Function, HashTable, Keyword, List, Null, Package, RandomState, Restart,
        Sequence, SimpleArray, SimpleBaseString, SimpleBitVector, SimpleString, SimpleVector,
        StandardChar, Stream, String, Structure, Symbol, Vector,
    };
    if is_numeric_type(left) && is_numeric_type(right) {
        return numeric_types_disjoint(left, right);
    }
    if is_numeric_type(left) {
        return matches!(
            right,
            Boolean
                | Character
                | BaseChar
                | StandardChar
                | ExtendedChar
                | Symbol
                | Keyword
                | Null
                | String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | Cons
                | List
                | Sequence
                | Function
                | CompiledFunction
                | Array
                | SimpleArray
                | Vector
                | BitVector
                | SimpleBitVector
                | SimpleVector
                | Package
                | Stream
                | Restart
                | HashTable
                | RandomState
                | Structure
        );
    }
    if is_numeric_type(right) {
        return known_disjoint(right, left);
    }
    matches!(
        (left, right),
        (Atom, Cons)
            | (
                Cons,
                Atom | Symbol
                    | Null
                    | String
                    | Character
                    | Function
                    | Array
                    | Package
                    | Stream
                    | Restart
            )
            | (
                Symbol,
                Cons | String | Character | Function | Array | Package | Stream | Restart
            )
            | (
                String,
                Symbol | Character | Function | Package | Stream | Restart
            )
            | (
                Character,
                Symbol | String | Function | Array | Sequence | Package | Stream | Restart
            )
            | (
                Function,
                Symbol | String | Character | Array | Sequence | Package | Stream | Restart | List
            )
            | (
                List,
                String | Character | Function | Array | Package | Stream | Restart
            )
            | (Sequence, Character | Function | Package | Stream | Restart)
            | (
                Array,
                Character | Function | Package | Stream | Restart | List
            )
            | (
                Package,
                String | Character | Function | Array | Sequence | Cons
            )
            | (
                Stream,
                String | Character | Function | Array | Sequence | Cons
            )
            | (
                Restart,
                String | Character | Function | Array | Sequence | Cons
            )
    )
}

const fn numeric_types_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    known_numeric_disjoint(left, right) || known_numeric_disjoint(right, left)
}

const fn known_numeric_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    use BuiltinType::{
        Bignum, Complex, DoubleFloat, Fixnum, Float, Integer, LongFloat, Ratio, Rational, Real,
        ShortFloat, SingleFloat,
    };
    matches!(
        (left, right),
        (
            Integer | Rational,
            Float | ShortFloat | SingleFloat | DoubleFloat | LongFloat
        ) | (
            Float | ShortFloat | SingleFloat | DoubleFloat | LongFloat | Complex,
            Integer | Rational
        ) | (Integer | Rational | Real, Complex)
            | (Complex, Real)
            | (Fixnum | Bignum, Ratio)
            | (Ratio, Fixnum | Bignum)
    )
}

const fn is_numeric_type(name: BuiltinType) -> bool {
    matches!(
        name,
        BuiltinType::Number
            | BuiltinType::Real
            | BuiltinType::Rational
            | BuiltinType::Integer
            | BuiltinType::Fixnum
            | BuiltinType::Bignum
            | BuiltinType::Ratio
            | BuiltinType::Float
            | BuiltinType::ShortFloat
            | BuiltinType::SingleFloat
            | BuiltinType::DoubleFloat
            | BuiltinType::LongFloat
            | BuiltinType::Complex
    )
}

const fn range_to_named(range: NumericRange, name: BuiltinType) -> SubtypeRelation {
    use BuiltinType::{
        Atom, Complex, DoubleFloat, Float, Integer, Number, Rational, Real, SingleFloat,
    };
    let relation = match range.domain() {
        NumericDomain::Integer => match name {
            Integer | Rational | Real | Number | Atom => Some(true),
            Float | SingleFloat | DoubleFloat | Complex => Some(false),
            _ => None,
        },
        NumericDomain::SingleFloat => match name {
            SingleFloat | Float | Real | Number | Atom => Some(true),
            Integer | Rational | Complex => Some(false),
            _ => None,
        },
        NumericDomain::DoubleFloat => match name {
            DoubleFloat | Float | Real | Number | Atom => Some(true),
            Integer | Rational | Complex => Some(false),
            _ => None,
        },
    };
    match relation {
        Some(true) => SubtypeRelation::Yes,
        Some(false) => SubtypeRelation::No,
        None => SubtypeRelation::Unknown,
    }
}

fn named_to_range(name: BuiltinType, range: NumericRange) -> SubtypeRelation {
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
        _ => SubtypeRelation::Unknown,
    }
}

fn numeric_relation(sub: NumericRange, sup: NumericRange) -> SubtypeRelation {
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

fn member_to_member(sub: &[MemberValue], sup: &[MemberValue]) -> SubtypeRelation {
    combine_all(sub.iter().map(|value| member_value_in_member(*value, sup)))
}

fn member_to_union(
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
            _ => SubtypeRelation::No,
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

fn member_value_in_member(value: MemberValue, candidates: &[MemberValue]) -> SubtypeRelation {
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

fn member_to_named(values: &[MemberValue], name: BuiltinType) -> SubtypeRelation {
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
        _ => SubtypeRelation::No,
    }
}

fn member_to_range(values: &[MemberValue], range: NumericRange) -> SubtypeRelation {
    combine_all(values.iter().map(|value| match value {
        MemberValue::Integer(value) => contains_value(range, NumericValue::Integer(*value)),
        MemberValue::SingleFloat(value) => contains_value(range, NumericValue::SingleFloat(*value)),
        MemberValue::DoubleFloat(value) => contains_value(range, NumericValue::DoubleFloat(*value)),
        MemberValue::OpaqueObject { .. } => SubtypeRelation::Unknown,
        _ => SubtypeRelation::No,
    }))
}

fn contains_value(range: NumericRange, value: NumericValue) -> SubtypeRelation {
    match range.contains_value(value) {
        Some(true) => SubtypeRelation::Yes,
        Some(false) => SubtypeRelation::No,
        None => SubtypeRelation::Unknown,
    }
}

fn named_to_member(name: BuiltinType, values: &[MemberValue]) -> SubtypeRelation {
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
        _ => SubtypeRelation::Unknown,
    }
}

fn range_to_member(range: NumericRange, values: &[MemberValue]) -> SubtypeRelation {
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

fn array_to_named(array: &ArrayType, name: BuiltinType) -> SubtypeRelation {
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
        _ => SubtypeRelation::Unknown,
    }
}

fn array_relation(
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
                _ => SubtypeRelation::No,
            }))
        }
    }
}

fn function_relation(
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

fn disjoint_relation(
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
        (ClTypeKind::Empty, _) | (_, ClTypeKind::Empty) => SubtypeRelation::Yes,
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
        (ClTypeKind::Complement(excluded), _) => subtype_inner(arena, right, *excluded, cache),
        (_, ClTypeKind::Complement(excluded)) => subtype_inner(arena, left, *excluded, cache),
        (ClTypeKind::Union(members), _) => combine_all(
            members
                .iter()
                .map(|member| disjoint_relation(arena, *member, right, cache)),
        ),
        (_, ClTypeKind::Union(members)) => combine_all(
            members
                .iter()
                .map(|member| disjoint_relation(arena, left, *member, cache)),
        ),
        (ClTypeKind::Intersection(members), _) => match combine_any(
            members
                .iter()
                .map(|member| disjoint_relation(arena, *member, right, cache)),
        ) {
            SubtypeRelation::Yes => SubtypeRelation::Yes,
            SubtypeRelation::No | SubtypeRelation::Unknown => SubtypeRelation::Unknown,
        },
        (_, ClTypeKind::Intersection(members)) => match combine_any(
            members
                .iter()
                .map(|member| disjoint_relation(arena, left, *member, cache)),
        ) {
            SubtypeRelation::Yes => SubtypeRelation::Yes,
            SubtypeRelation::No | SubtypeRelation::Unknown => SubtypeRelation::Unknown,
        },
        (ClTypeKind::Member(left), ClTypeKind::Member(right)) => invert_overlap(combine_any(
            left.iter()
                .map(|value| member_value_in_member(*value, right)),
        )),
        (ClTypeKind::Member(values), ClTypeKind::Named(name))
        | (ClTypeKind::Named(name), ClTypeKind::Member(values)) => {
            member_disjoint_from_named(values, *name)
        }
        (ClTypeKind::Member(values), ClTypeKind::NumericRange(range))
        | (ClTypeKind::NumericRange(range), ClTypeKind::Member(values)) => {
            member_disjoint_from_range(values, *range)
        }
        _ => SubtypeRelation::Unknown,
    }
}

fn member_disjoint_from_named(values: &[MemberValue], name: BuiltinType) -> SubtypeRelation {
    invert_overlap(combine_any(
        values
            .iter()
            .map(|value| member_value_to_named(*value, name)),
    ))
}

fn member_disjoint_from_range(values: &[MemberValue], range: NumericRange) -> SubtypeRelation {
    invert_overlap(combine_any(values.iter().map(|value| match value {
        MemberValue::Integer(value) => contains_value(range, NumericValue::Integer(*value)),
        MemberValue::SingleFloat(value) => contains_value(range, NumericValue::SingleFloat(*value)),
        MemberValue::DoubleFloat(value) => contains_value(range, NumericValue::DoubleFloat(*value)),
        MemberValue::OpaqueObject { .. } => SubtypeRelation::Unknown,
        _ => SubtypeRelation::No,
    })))
}

const fn invert_overlap(overlap: SubtypeRelation) -> SubtypeRelation {
    match overlap {
        SubtypeRelation::Yes => SubtypeRelation::No,
        SubtypeRelation::No => SubtypeRelation::Yes,
        SubtypeRelation::Unknown => SubtypeRelation::Unknown,
    }
}
