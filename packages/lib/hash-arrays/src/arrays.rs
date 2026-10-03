use ncl_object::array::{
    adjustable_array_p, array_displacement, array_element_type, array_has_fill_pointer_p,
    fill_pointer, vector_pop, vector_push, vector_push_extend,
};
use ncl_object::package::{nil, truth};
use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinArgs, BuiltinName, LambdaList, MultipleValues,
    ObjectError, ObjectRef, Parameter, ParameterType, Runtime, ThreadContext, Word,
    array_row_major_ref, array_row_major_set, classify_object, make_array, make_cons, pop_root,
    push_root, simple_vector_ref, simple_vector_set,
};

use super::{register_one, register_one_ncl, symbol_text};
use helpers::{array_element_type_symbol, array_shape};

mod adjust;
mod helpers;
mod make_array_builtin;

use adjust::adjust_array_builtin;
use make_array_builtin::make_array_builtin;

const ARRAY: Parameter = Parameter {
    name: BuiltinName::new("ARRAY"),
    ty: ParameterType::Any,
};
const INDEX: Parameter = Parameter {
    name: BuiltinName::new("INDEX"),
    ty: ParameterType::Fixnum,
};
const DIMENSIONS: Parameter = Parameter {
    name: BuiltinName::new("DIMENSIONS"),
    ty: ParameterType::Any,
};
const OPTIONS: Parameter = Parameter {
    name: BuiltinName::new("OPTIONS"),
    ty: ParameterType::Any,
};
const ELEMENT: Parameter = Parameter {
    name: BuiltinName::new("ELEMENT"),
    ty: ParameterType::Any,
};
const EXTENSION: Parameter = Parameter {
    name: BuiltinName::new("EXTENSION"),
    ty: ParameterType::Fixnum,
};
const RESULT: Parameter = Parameter {
    name: BuiltinName::new("RESULT"),
    ty: ParameterType::Any,
};
const VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};

fn arrayp_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            classify_object(ctx, args.required(0)?),
            ObjectRef::Array(_)
                | ObjectRef::SpecializedArray(_)
                | ObjectRef::SimpleVector(_)
                | ObjectRef::String(_)
        ) {
            truth()
        } else {
            nil()
        },
    )
}
fn vectorp_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if array_shape(ctx, args.required(0)?).is_ok_and(|shape| shape.len() == 1) {
            truth()
        } else {
            nil()
        },
    )
}
fn simple_vector_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if matches!(
            classify_object(ctx, args.required(0)?),
            ObjectRef::SimpleVector(_)
        ) {
            truth()
        } else {
            nil()
        },
    )
}
fn adjustable_array_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = match classify_object(ctx, args.required(0)?) {
        ObjectRef::Array(array) => adjustable_array_p(ctx, array)?,
        _ => false,
    };
    Ok(if result { truth() } else { nil() })
}
fn array_has_fill_pointer_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let result = match classify_object(ctx, args.required(0)?) {
        ObjectRef::Array(array) => array_has_fill_pointer_p(ctx, array)?,
        _ => false,
    };
    Ok(if result { truth() } else { nil() })
}
fn fill_pointer_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    fill_pointer(ctx, args.required(0)?).and_then(|value| {
        Ok(Word::fixnum(
            i64::try_from(value).map_err(|_| ObjectError::Layout)?,
        ))
    })
}
fn vector_push_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(vector_push(ctx, args.required(1)?, args.required(0)?)?
        .and_then(|index| i64::try_from(index).ok().map(Word::fixnum))
        .unwrap_or(Word::NIL))
}

fn vector_push_extend_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let extension = args.as_slice().get(2).copied().unwrap_or(Word::fixnum(1));
    let extension = usize::try_from(extension.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    let (index, adjusted) = vector_push_extend(
        ctx,
        runtime,
        args.required(1)?,
        args.required(0)?,
        extension,
    )?;
    values.set(&[
        Word::fixnum(i64::try_from(index).map_err(|_| ObjectError::Layout)?),
        adjusted,
    ]);
    Ok(Word::fixnum(
        i64::try_from(index).map_err(|_| ObjectError::Layout)?,
    ))
}

fn vector_pop_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    vector_pop(ctx, args.required(0)?)
}
fn array_rank_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::fixnum(
        i64::try_from(array_shape(ctx, args.required(0)?)?.len())
            .map_err(|_| ObjectError::Layout)?,
    ))
}
fn array_dimension_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let shape = array_shape(ctx, args.required(0)?)?;
    let index = usize::try_from(
        args.required(1)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    Ok(Word::fixnum(
        i64::try_from(*shape.get(index).ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::Layout)?,
    ))
}
fn array_total_size_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let total = array_shape(ctx, args.required(0)?)?
        .into_iter()
        .try_fold(1usize, usize::checked_mul)
        .ok_or(ObjectError::Layout)?;
    Ok(Word::fixnum(
        i64::try_from(total).map_err(|_| ObjectError::Layout)?,
    ))
}
fn array_dimensions_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut result = Word::NIL;
    for dimension in array_shape(ctx, args.required(0)?)?.into_iter().rev() {
        let token = push_root(ctx, &mut result);
        let next = make_cons(
            ctx,
            runtime,
            Word::fixnum(i64::try_from(dimension).map_err(|_| ObjectError::Layout)?),
            result,
        );
        if !pop_root(ctx, token) {
            return Err(ObjectError::Layout);
        }
        result = next?;
    }
    Ok(result)
}
fn array_in_bounds_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let shape = array_shape(ctx, args.required(0)?)?;
    // check-added-lines: allow(index)
    let indices = &args.as_slice()[1..];
    Ok(
        if shape.len() == indices.len()
            && shape.iter().zip(indices).all(|(dimension, index)| {
                index
                    .as_fixnum()
                    .and_then(|value| usize::try_from(value).ok())
                    .is_some_and(|index| index < *dimension)
            })
        {
            truth()
        } else {
            nil()
        },
    )
}
fn row_major_aref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let index = usize::try_from(
        args.required(1)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    array_row_major_ref(ctx, args.required(0)?, index)
}
fn svref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let index = usize::try_from(
        args.required(1)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    simple_vector_ref(ctx, args.required(0)?, index)
}

fn aref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    // check-added-lines: allow(index)
    let offset = row_major_index(&array_shape(ctx, array)?, &args.as_slice()[1..])?;
    array_row_major_ref(ctx, array, offset)
}

/// `setf`-support: `(NCL-EXT::AREF-SET array subscript... value)`.
///
/// Mirrors [`aref_builtin`] but writes `value` at the computed row-major
/// offset and returns `value`, so `(setf (aref ...) v)` can expand to a
/// plain function call that already returns the stored value.
fn aref_set_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let slice = args.as_slice();
    let (array, rest) = slice.split_first().ok_or(ObjectError::TypeError)?;
    let (value, subscripts) = rest.split_last().ok_or(ObjectError::TypeError)?;
    let offset = row_major_index(&array_shape(ctx, *array)?, subscripts)?;
    array_row_major_set(ctx, *array, offset, *value)?;
    Ok(*value)
}

/// `setf`-support: `(NCL-EXT::SVREF-SET simple-vector index value)`.
fn svref_set_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let index = usize::try_from(
        args.required(1)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?,
    )
    .map_err(|_| ObjectError::TypeError)?;
    let value = args.required(2)?;
    simple_vector_set(ctx, args.required(0)?, index, value)?;
    Ok(value)
}

fn array_element_type_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    array_element_type_symbol(ctx, runtime, array_element_type(ctx, args.required(0)?)?)
}

fn array_displacement_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (target, offset) = array_displacement(ctx, args.required(0)?)?;
    values.set(&[
        target,
        Word::fixnum(i64::try_from(offset).map_err(|_| ObjectError::Layout)?),
    ]);
    Ok(target)
}

fn array_row_major_index_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let shape = array_shape(ctx, args.required(0)?)?;
    // check-added-lines: allow(index)
    let indices = &args.as_slice()[1..];
    let offset = row_major_index(&shape, indices)?;
    Ok(Word::fixnum(
        i64::try_from(offset).map_err(|_| ObjectError::Layout)?,
    ))
}

fn row_major_index(shape: &[usize], indices: &[Word]) -> Result<usize, ObjectError> {
    if shape.len() != indices.len() {
        return Err(ObjectError::TypeError);
    }
    let mut offset = 0usize;
    let mut stride = 1usize;
    for (dimension, index) in shape
        .iter()
        .copied()
        .rev()
        .zip(indices.iter().copied().rev())
    {
        let index = usize::try_from(index.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        if index >= dimension {
            return Err(ObjectError::TypeError);
        }
        offset = offset
            .checked_add(index.checked_mul(stride).ok_or(ObjectError::Layout)?)
            .ok_or(ObjectError::Layout)?;
        stride = stride.checked_mul(dimension).ok_or(ObjectError::Layout)?;
    }
    Ok(offset)
}

mod bit;
use bit::{
    bit_and_builtin, bit_andc1_builtin, bit_andc2_builtin, bit_builtin, bit_eqv_builtin,
    bit_ior_builtin, bit_nand_builtin, bit_nor_builtin, bit_not_builtin, bit_orc1_builtin,
    bit_orc2_builtin, bit_xor_builtin, sbit_builtin,
};

mod register;
mod vector;
pub use register::register;

#[cfg(test)]
#[path = "arrays/tests.rs"]
mod tests;
