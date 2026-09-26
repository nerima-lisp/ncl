use super::array_shape;
use ncl_object::array::array_element_type;
use ncl_object::package::{nil, truth};
use ncl_object::{
    ArrayElementType, BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime, ThreadContext,
    Word, classify_object, make_simple_vector,
};

pub(super) fn vector_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    make_simple_vector(ctx, runtime, args.as_slice())
}

pub(super) fn simple_bit_vector_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let simple_bit_vector = matches!(classify_object(ctx, value), ObjectRef::SpecializedArray(_))
        && array_element_type(ctx, value)? == ArrayElementType::Bit
        && array_shape(ctx, value)?.len() == 1;
    Ok(if simple_bit_vector { truth() } else { nil() })
}

pub(super) fn bit_vector_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let bit_vector = array_element_type(ctx, value) == Ok(ArrayElementType::Bit)
        && array_shape(ctx, value).is_ok_and(|shape| shape.len() == 1);
    Ok(if bit_vector { truth() } else { nil() })
}
