use ncl_object::package::Package;
use ncl_object::{
    ArrayElementType, ObjectError, ObjectRef, Runtime, ThreadContext, Word, array_dimensions, car,
    cdr, simple_vector_length, string_length,
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

pub(super) fn list_values(
    ctx: &mut ThreadContext,
    mut list: Word,
) -> Result<Vec<Word>, ObjectError> {
    let mut values = Vec::new();
    while list != Word::NIL {
        values.push(car(ctx, list)?);
        list = cdr(ctx, list)?;
    }
    Ok(values)
}

pub(super) fn array_shape(ctx: &ThreadContext, value: Word) -> Result<Vec<usize>, ObjectError> {
    match ncl_object::classify_object(ctx, value) {
        ObjectRef::SimpleVector(vector) => Ok(vec![simple_vector_length(ctx, vector)?]),
        ObjectRef::String(string) => Ok(vec![string_length(ctx, string)?]),
        ObjectRef::Array(_) | ObjectRef::SpecializedArray(_) => array_dimensions(ctx, value),
        _ => Err(ObjectError::TypeError),
    }
}
