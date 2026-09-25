//! The `typep` predicate.

use ncl_object::{
    ArrayElementType, ObjectRef, Package, ThreadContext, Word, array_dimensions, car, cdr,
    classify_object, simple_vector_length, specialized_array_element_type, string_length,
    symbol_package,
};

use crate::adapter::from_word;
use crate::text::string_to_upper;
use crate::{
    ArrayDimension, ArrayDimensions, IntegerBound, NamedType, TypeError, TypeSpecifier, Value,
};

/// Test whether `object` satisfies the type specifier.
///
/// # Errors
///
/// Returns [`TypeError::CannotInvoke`] for a `satisfies` specifier and
/// [`TypeError::UnexpandedDeftype`] for an unexpanded `deftype`; propagates
/// object-layer errors as [`TypeError::Object`].
pub fn typep(
    ctx: &mut ThreadContext,
    object: Word,
    spec: &TypeSpecifier,
) -> Result<bool, TypeError> {
    match spec {
        TypeSpecifier::Named(named) => typep_named(ctx, object, *named),
        TypeSpecifier::IntegerRange { low, high } => Ok(typep_integer_range(object, *low, *high)),
        TypeSpecifier::Or(specs) => any_match(ctx, object, specs),
        TypeSpecifier::And(specs) => all_match(ctx, object, specs),
        TypeSpecifier::Not(inner) => Ok(!typep(ctx, object, inner)?),
        TypeSpecifier::Member(items) => member_match(ctx, object, items),
        TypeSpecifier::Eql(item) => value_matches(ctx, object, item),
        TypeSpecifier::Satisfies(predicate) => Err(TypeError::CannotInvoke(predicate.clone())),
        TypeSpecifier::Array {
            element_type,
            dimensions,
            simple,
        } => typep_array(
            ctx,
            object,
            element_type.as_deref(),
            dimensions.as_ref(),
            *simple,
        ),
        TypeSpecifier::Vector { element_type, size } => {
            typep_vector(ctx, object, element_type.as_deref(), *size)
        }
        TypeSpecifier::Cons { car, cdr } => typep_cons(ctx, object, car, cdr),
        TypeSpecifier::Function { .. } => Ok(is_function(ctx, object)),
        TypeSpecifier::Values(_) => Ok(true),
        TypeSpecifier::Deftype { name, .. } => Err(TypeError::UnexpandedDeftype(name.clone())),
    }
}

fn typep_named(ctx: &ThreadContext, object: Word, named: NamedType) -> Result<bool, TypeError> {
    match named {
        NamedType::T | NamedType::ValuesType => Ok(true),
        NamedType::Nil
        | NamedType::ShortFloat
        | NamedType::SingleFloat
        | NamedType::LongFloat
        | NamedType::RandomState
        | NamedType::Restart
        | NamedType::Character
        | NamedType::BaseChar
        | NamedType::StandardChar
        | NamedType::ExtendedChar => Ok(false),
        NamedType::Boolean => Ok(object == Word::NIL || object == Word::TRUE),
        NamedType::Symbol => {
            Ok(object == Word::NIL || matches!(classify_object(ctx, object), ObjectRef::Symbol(_)))
        }
        NamedType::Keyword => is_keyword(ctx, object),
        NamedType::Package => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::Package(_)
        )),
        NamedType::Cons => Ok(object.is_cons()),
        NamedType::List => Ok(object.is_list()),
        NamedType::Null => Ok(object == Word::NIL),
        NamedType::Atom => Ok(!object.is_cons()),
        NamedType::Function => Ok(is_function(ctx, object)),
        NamedType::CompiledFunction => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::Function(_)
        )),
        NamedType::Number => Ok(is_number(ctx, object)),
        NamedType::Real => Ok(is_real(ctx, object)),
        NamedType::Rational => Ok(is_rational(ctx, object)),
        NamedType::Integer => {
            Ok(object.is_fixnum() || matches!(classify_object(ctx, object), ObjectRef::Bignum(_)))
        }
        NamedType::Fixnum => Ok(object.is_fixnum()),
        NamedType::Bignum => Ok(matches!(classify_object(ctx, object), ObjectRef::Bignum(_))),
        NamedType::Ratio => Ok(matches!(classify_object(ctx, object), ObjectRef::Ratio(_))),
        NamedType::Float | NamedType::DoubleFloat => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::DoubleFloat(_)
        )),
        NamedType::Complex => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::Complex(_)
        )),
        NamedType::String
        | NamedType::BaseString
        | NamedType::SimpleString
        | NamedType::SimpleBaseString => {
            Ok(matches!(classify_object(ctx, object), ObjectRef::String(_)))
        }
        NamedType::Vector => is_vector(ctx, object),
        NamedType::SimpleVector => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::SimpleVector(_)
        )),
        NamedType::Array => Ok(is_array(ctx, object)),
        NamedType::SimpleArray => Ok(is_simple_array(ctx, object)),
        NamedType::BitVector | NamedType::SimpleBitVector => is_bit_vector(ctx, object),
        NamedType::Sequence => Ok(object.is_list() || is_vector(ctx, object)?),
        NamedType::HashTable => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::HashTable(_)
        )),
        NamedType::Stream => Ok(matches!(classify_object(ctx, object), ObjectRef::Stream(_))),
        NamedType::Structure => Ok(matches!(
            classify_object(ctx, object),
            ObjectRef::Structure(_)
        )),
    }
}

const fn typep_integer_range(object: Word, low: IntegerBound, high: IntegerBound) -> bool {
    let Some(value) = object.as_fixnum() else {
        // Bignum objects are outside fixnum range and are not compared here.
        return false;
    };
    bound_contains(low, value, true) && bound_contains(high, value, false)
}

const fn bound_contains(bound: IntegerBound, value: i64, lower: bool) -> bool {
    match bound {
        IntegerBound::Unbounded => true,
        IntegerBound::Inclusive(bound) => {
            if lower {
                value >= bound
            } else {
                value <= bound
            }
        }
        IntegerBound::Exclusive(bound) => {
            if lower {
                value > bound
            } else {
                value < bound
            }
        }
    }
}

fn any_match(
    ctx: &mut ThreadContext,
    object: Word,
    specs: &[TypeSpecifier],
) -> Result<bool, TypeError> {
    for spec in specs {
        if typep(ctx, object, spec)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn all_match(
    ctx: &mut ThreadContext,
    object: Word,
    specs: &[TypeSpecifier],
) -> Result<bool, TypeError> {
    for spec in specs {
        if !typep(ctx, object, spec)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn member_match(ctx: &ThreadContext, object: Word, items: &[Value]) -> Result<bool, TypeError> {
    for item in items {
        if value_matches(ctx, object, item)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::similar_names)]
fn typep_cons(
    ctx: &mut ThreadContext,
    object: Word,
    car_spec: &TypeSpecifier,
    cdr_spec: &TypeSpecifier,
) -> Result<bool, TypeError> {
    if !object.is_cons() {
        return Ok(false);
    }
    let car_object = car(ctx, object)?;
    let cdr_object = cdr(ctx, object)?;
    Ok(typep(ctx, car_object, car_spec)? && typep(ctx, cdr_object, cdr_spec)?)
}

fn typep_array(
    ctx: &ThreadContext,
    object: Word,
    element_type: Option<&TypeSpecifier>,
    dimensions: Option<&ArrayDimensions>,
    simple: bool,
) -> Result<bool, TypeError> {
    let reference = classify_object(ctx, object);
    #[allow(clippy::wildcard_enum_match_arm)]
    let (is_array, is_simple) = match reference {
        ObjectRef::SimpleVector(_) | ObjectRef::SpecializedArray(_) | ObjectRef::String(_) => {
            (true, true)
        }
        ObjectRef::Array(_) => (true, false),
        _ => (false, false),
    };
    if !is_array || (simple && !is_simple) {
        return Ok(false);
    }
    // Element type is parsed but not enforced here: the array's own kind
    // already fixes its element representation.
    let _ = element_type;
    if let Some(dimensions) = dimensions
        && !dimensions_match(ctx, reference, dimensions)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn typep_vector(
    ctx: &ThreadContext,
    object: Word,
    element_type: Option<&TypeSpecifier>,
    size: Option<ArrayDimension>,
) -> Result<bool, TypeError> {
    if !is_vector(ctx, object)? {
        return Ok(false);
    }
    let _ = element_type;
    if let Some(size) = size
        && let Some(length) = vector_length(ctx, object)?
        && !dimension_matches(size, length)
    {
        return Ok(false);
    }
    Ok(true)
}

fn dimensions_match(
    ctx: &ThreadContext,
    reference: ObjectRef,
    dimensions: &ArrayDimensions,
) -> Result<bool, TypeError> {
    match dimensions {
        ArrayDimensions::Wild => Ok(true),
        ArrayDimensions::Rank(expected) => Ok(rank_of(ctx, reference)? == Some(*expected)),
        ArrayDimensions::Ranks(expected) => {
            let Some(actual) = actual_dimensions(ctx, reference)? else {
                return Ok(true);
            };
            if actual.len() != expected.len() {
                return Ok(false);
            }
            for (index, bound) in expected.iter().enumerate() {
                let Some(dimension) = actual.get(index) else {
                    return Ok(false);
                };
                if !dimension_matches(*bound, *dimension) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn dimension_matches(bound: ArrayDimension, actual: usize) -> bool {
    match bound {
        ArrayDimension::Any => true,
        ArrayDimension::Exact(expected) => actual == expected,
        ArrayDimension::Exclusive(expected) => actual < expected,
    }
}

fn rank_of(ctx: &ThreadContext, reference: ObjectRef) -> Result<Option<usize>, TypeError> {
    #[allow(clippy::wildcard_enum_match_arm)]
    match reference {
        ObjectRef::SimpleVector(_) | ObjectRef::String(_) | ObjectRef::SpecializedArray(_) => {
            Ok(Some(1))
        }
        ObjectRef::Array(word) => Ok(Some(array_dimensions(ctx, word)?.len())),
        _ => Ok(None),
    }
}

fn actual_dimensions(
    ctx: &ThreadContext,
    reference: ObjectRef,
) -> Result<Option<Vec<usize>>, TypeError> {
    #[allow(clippy::wildcard_enum_match_arm)]
    match reference {
        ObjectRef::SimpleVector(word) => Ok(Some(vec![simple_vector_length(ctx, word)?])),
        ObjectRef::String(word) => Ok(Some(vec![string_length(ctx, word)?])),
        ObjectRef::Array(word) => Ok(Some(array_dimensions(ctx, word)?)),
        _ => Ok(None),
    }
}

#[must_use]
fn is_function(ctx: &ThreadContext, object: Word) -> bool {
    matches!(
        classify_object(ctx, object),
        ObjectRef::Function(_) | ObjectRef::Closure(_)
    )
}

#[must_use]
fn is_number(ctx: &ThreadContext, object: Word) -> bool {
    object.is_fixnum()
        || matches!(
            classify_object(ctx, object),
            ObjectRef::Bignum(_)
                | ObjectRef::Ratio(_)
                | ObjectRef::DoubleFloat(_)
                | ObjectRef::Complex(_)
        )
}

#[must_use]
fn is_real(ctx: &ThreadContext, object: Word) -> bool {
    object.is_fixnum()
        || matches!(
            classify_object(ctx, object),
            ObjectRef::Bignum(_) | ObjectRef::Ratio(_) | ObjectRef::DoubleFloat(_)
        )
}

#[must_use]
fn is_rational(ctx: &ThreadContext, object: Word) -> bool {
    object.is_fixnum()
        || matches!(
            classify_object(ctx, object),
            ObjectRef::Bignum(_) | ObjectRef::Ratio(_)
        )
}

fn is_vector(ctx: &ThreadContext, object: Word) -> Result<bool, TypeError> {
    let reference = classify_object(ctx, object);
    #[allow(clippy::wildcard_enum_match_arm)]
    Ok(match reference {
        ObjectRef::SimpleVector(_) | ObjectRef::String(_) | ObjectRef::SpecializedArray(_) => true,
        ObjectRef::Array(word) => array_dimensions(ctx, word)?.len() == 1,
        _ => false,
    })
}

#[must_use]
fn is_array(ctx: &ThreadContext, object: Word) -> bool {
    matches!(
        classify_object(ctx, object),
        ObjectRef::SimpleVector(_)
            | ObjectRef::SpecializedArray(_)
            | ObjectRef::Array(_)
            | ObjectRef::String(_)
    )
}

#[must_use]
fn is_simple_array(ctx: &ThreadContext, object: Word) -> bool {
    matches!(
        classify_object(ctx, object),
        ObjectRef::SimpleVector(_) | ObjectRef::SpecializedArray(_) | ObjectRef::String(_)
    )
}

fn is_bit_vector(ctx: &ThreadContext, object: Word) -> Result<bool, TypeError> {
    if let ObjectRef::SpecializedArray(word) = classify_object(ctx, object) {
        Ok(specialized_array_element_type(ctx, word)? == ArrayElementType::Bit)
    } else {
        Ok(false)
    }
}

fn is_keyword(ctx: &ThreadContext, object: Word) -> Result<bool, TypeError> {
    if object == Word::NIL || !matches!(classify_object(ctx, object), ObjectRef::Symbol(_)) {
        return Ok(false);
    }
    let package = symbol_package(ctx, object)?;
    if package == Word::NIL {
        return Ok(false);
    }
    let name = Package::from(package).name(ctx)?;
    Ok(string_to_upper(ctx, name)? == "KEYWORD")
}

fn vector_length(ctx: &ThreadContext, object: Word) -> Result<Option<usize>, TypeError> {
    #[allow(clippy::wildcard_enum_match_arm)]
    match classify_object(ctx, object) {
        ObjectRef::SimpleVector(word) => Ok(Some(simple_vector_length(ctx, word)?)),
        ObjectRef::String(word) => Ok(Some(string_length(ctx, word)?)),
        ObjectRef::Array(word) => {
            let dimensions = array_dimensions(ctx, word)?;
            Ok((dimensions.len() == 1)
                .then(|| dimensions.first().copied())
                .flatten())
        }
        _ => Ok(None),
    }
}

#[allow(clippy::float_cmp)]
fn value_matches(ctx: &ThreadContext, object: Word, value: &Value) -> Result<bool, TypeError> {
    if matches!(value, Value::Opaque(bits) if *bits == object.bits()) {
        return Ok(true);
    }
    Ok(from_word(ctx, object)? == *value)
}
