use ncl_object::package::Package;
use ncl_object::{
    ArrayElementType, ObjectError, ObjectRef, Runtime, ThreadContext, Word, array_dimensions, car,
    cdr, simple_vector_length, simple_vector_ref, string_length, string_ref,
};

pub(super) fn array_element_type_symbol(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    element_type: ArrayElementType,
) -> Result<Word, ObjectError> {
    let name = match element_type {
        ArrayElementType::T => "T",
        ArrayElementType::Bit => "BIT",
        ArrayElementType::Character => "CHARACTER",
        ArrayElementType::BaseChar => "BASE-CHAR",
        ArrayElementType::Fixnum => "FIXNUM",
        ArrayElementType::Signed => "SIGNED-BYTE",
        ArrayElementType::Unsigned => "UNSIGNED-BYTE",
        ArrayElementType::SingleFloat => "SINGLE-FLOAT",
        ArrayElementType::DoubleFloat => "DOUBLE-FLOAT",
    };
    let package = runtime
        .find_package(ctx, "COMMON-LISP")
        .ok_or(ObjectError::PackageConflict)?;
    Ok(Package::from_word(package).intern(ctx, runtime, name)?.0)
}

pub(super) fn list_values(ctx: &ThreadContext, mut list: Word) -> Result<Vec<Word>, ObjectError> {
    let mut values = Vec::new();
    while list != Word::NIL {
        values.push(car(ctx, list)?);
        list = cdr(ctx, list)?;
    }
    Ok(values)
}

pub(super) fn dimension_values(
    ctx: &ThreadContext,
    value: Word,
) -> Result<Vec<usize>, ObjectError> {
    let values = match value.as_fixnum() {
        Some(_) => vec![value],
        None => list_values(ctx, value)?,
    };
    values
        .into_iter()
        .map(|value| {
            usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                .map_err(|_| ObjectError::TypeError)
        })
        .collect()
}

pub(super) fn array_shape(ctx: &ThreadContext, value: Word) -> Result<Vec<usize>, ObjectError> {
    match ncl_object::classify_object(ctx, value) {
        ObjectRef::SimpleVector(vector) => Ok(vec![simple_vector_length(ctx, vector)?]),
        ObjectRef::String(string) => Ok(vec![string_length(ctx, string)?]),
        ObjectRef::Array(_) | ObjectRef::SpecializedArray(_) => array_dimensions(ctx, value),
        _ => Err(ObjectError::TypeError),
    }
}

/// Read one level of a `:initial-contents` nested structure as a flat
/// sequence of elements. Lists, simple vectors, and strings are accepted,
/// matching the sequence types CLHS permits at each level of nesting.
pub(super) fn sequence_elements(
    ctx: &ThreadContext,
    value: Word,
) -> Result<Vec<Word>, ObjectError> {
    match ncl_object::classify_object(ctx, value) {
        ObjectRef::SimpleVector(vector) => (0..simple_vector_length(ctx, vector)?)
            .map(|index| simple_vector_ref(ctx, vector, index))
            .collect(),
        ObjectRef::String(string) => (0..string_length(ctx, string)?)
            .map(|index| string_ref(ctx, string, index).map(|c| Word::character(u32::from(c))))
            .collect(),
        ObjectRef::Cons(_) => list_values(ctx, value),
        _ if value == Word::NIL => Ok(Vec::new()),
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) non-sequence contents are rejected.
    }
}

/// Flatten a `:initial-contents` structure into row-major element order,
/// validating the shape against `dimensions` at every level of nesting.
pub(super) fn flatten_initial_contents(
    ctx: &ThreadContext,
    contents: Word,
    dimensions: &[usize],
) -> Result<Vec<Word>, ObjectError> {
    fn walk(
        ctx: &ThreadContext,
        value: Word,
        dims: &[usize],
        out: &mut Vec<Word>,
    ) -> Result<(), ObjectError> {
        let Some((&expected, rest)) = dims.split_first() else {
            out.push(value);
            return Ok(());
        };
        let elements = sequence_elements(ctx, value)?;
        if elements.len() != expected {
            return Err(ObjectError::TypeError);
        }
        if rest.is_empty() {
            out.extend(elements);
        } else {
            for element in elements {
                walk(ctx, element, rest, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(ctx, contents, dimensions, &mut out)?;
    Ok(out)
}
