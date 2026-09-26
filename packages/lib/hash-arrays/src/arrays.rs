use ncl_object::package::{nil, truth};
use ncl_object::array::{adjust_array, adjustable_array_p, array_has_fill_pointer_p, fill_pointer,
    vector_pop, vector_push, vector_push_extend};
use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinArgs, BuiltinName, LambdaList, MultipleValues,
    ObjectError, ObjectRef, Parameter, ParameterType, Runtime, ThreadContext, Word,
    array_dimensions, array_row_major_ref, car, cdr, classify_object, make_array, make_cons,
    simple_vector_length, simple_vector_ref, string_length,
};

use super::{register_one, symbol_text};

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

fn list_values(ctx: &mut ThreadContext, mut list: Word) -> Result<Vec<Word>, ObjectError> {
    let mut values = Vec::new();
    while list != Word::NIL {
        values.push(car(ctx, list)?);
        list = cdr(ctx, list)?;
    }
    Ok(values)
}

fn array_shape(ctx: &ThreadContext, value: Word) -> Result<Vec<usize>, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::SimpleVector(vector) => Ok(vec![simple_vector_length(ctx, vector)?]),
        ObjectRef::String(string) => Ok(vec![string_length(ctx, string)?]),
        ObjectRef::Array(_) => array_dimensions(ctx, value),
        _ => Err(ObjectError::TypeError),
    }
}

fn make_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = list_values(ctx, args.required(0)?)?
        .into_iter()
        .map(|value| {
            usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                .map_err(|_| ObjectError::TypeError)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut element_type = ArrayElementType::T;
    let mut initial_element = Word::NIL;
    let mut adjustable = false;
    let mut fill_pointer = None;
    let mut displaced_to = None;
    let mut displaced_index_offset = 0;
    for pair in args.as_slice()[1..].as_chunks::<2>().0 {
        match symbol_text(ctx, pair[0])?
            .to_ascii_uppercase()
            .trim_start_matches(':')
        {
            "ELEMENT-TYPE" => {
                element_type = match symbol_text(ctx, pair[1])?.to_ascii_uppercase().as_str() {
                    "T" => ArrayElementType::T,
                    "BIT" => ArrayElementType::Bit,
                    "CHARACTER" => ArrayElementType::Character,
                    "BASE-CHAR" => ArrayElementType::BaseChar,
                    "FIXNUM" => ArrayElementType::Fixnum,
                    _ => return Err(ObjectError::TypeError),
                }
            }
            "INITIAL-ELEMENT" => initial_element = pair[1],
            "ADJUSTABLE" => adjustable = pair[1] != Word::NIL,
            "FILL-POINTER" => {
                fill_pointer = Some(
                    usize::try_from(pair[1].as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?,
                );
            }
            "DISPLACED-TO" => displaced_to = (pair[1] != Word::NIL).then_some(pair[1]),
            "DISPLACED-INDEX-OFFSET" => {
                displaced_index_offset =
                    usize::try_from(pair[1].as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?;
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    make_array(
        ctx,
        runtime,
        &dimensions,
        ArrayOptions {
            element_type,
            initial_element,
            adjustable,
            fill_pointer,
            displaced_to,
            displaced_index_offset,
        },
    )
}

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
        Ok(Word::fixnum(i64::try_from(value).map_err(|_| ObjectError::Layout)?))
    })
}

fn adjust_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = list_values(ctx, args.required(1)?)?
        .into_iter()
        .map(|value| usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?).map_err(|_| ObjectError::TypeError))
        .collect::<Result<Vec<_>, _>>()?;
    let initial = args.as_slice().get(2).copied().unwrap_or(Word::NIL);
    adjust_array(ctx, runtime, args.required(0)?, &dimensions, initial)
}

fn vector_push_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(vector_push(ctx, args.required(0)?, args.required(1)?)?
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
    let (index, adjusted) = vector_push_extend(ctx, runtime, args.required(0)?, args.required(1)?, extension)?;
    values.set(&[Word::fixnum(i64::try_from(index).map_err(|_| ObjectError::Layout)?), adjusted]);
    Ok(Word::fixnum(i64::try_from(index).map_err(|_| ObjectError::Layout)?))
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
        result = make_cons(
            ctx,
            runtime,
            Word::fixnum(i64::try_from(dimension).map_err(|_| ObjectError::Layout)?),
            result,
        )?;
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
    let shape = array_shape(ctx, array)?;
    let indices = &args.as_slice()[1..];
    if shape.len() != indices.len() {
        return Err(ObjectError::TypeError);
    }
    let mut offset = 0usize;
    for (dimension, index) in shape.into_iter().zip(indices.iter().copied()) {
        let index = usize::try_from(index.as_fixnum().ok_or(ObjectError::TypeError)?)
            .map_err(|_| ObjectError::TypeError)?;
        if index >= dimension {
            return Err(ObjectError::TypeError);
        }
        offset = offset
            .checked_mul(dimension)
            .and_then(|value| value.checked_add(index))
            .ok_or(ObjectError::Layout)?;
    }
    array_row_major_ref(ctx, array, offset)
}

pub fn register(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ObjectError> {
    register_one(
        runtime,
        ctx,
        "MAKE-ARRAY",
        LambdaList::with_rest(&[DIMENSIONS], OPTIONS),
        make_array_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "AREF",
        LambdaList::with_rest(&[ARRAY], INDEX),
        aref_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ROW-MAJOR-AREF",
        LambdaList::fixed(&[ARRAY, INDEX]),
        row_major_aref_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SVREF",
        LambdaList::fixed(&[ARRAY, INDEX]),
        svref_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAYP",
        LambdaList::fixed(&[ARRAY]),
        arrayp_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTORP",
        LambdaList::fixed(&[ARRAY]),
        vectorp_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SIMPLE-VECTOR-P",
        LambdaList::fixed(&[ARRAY]),
        simple_vector_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-RANK",
        LambdaList::fixed(&[ARRAY]),
        array_rank_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-DIMENSION",
        LambdaList::fixed(&[ARRAY, INDEX]),
        array_dimension_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-DIMENSIONS",
        LambdaList::fixed(&[ARRAY]),
        array_dimensions_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-TOTAL-SIZE",
        LambdaList::fixed(&[ARRAY]),
        array_total_size_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-IN-BOUNDS-P",
        LambdaList::with_rest(&[ARRAY], INDEX),
        array_in_bounds_builtin,
    )?;
    register_one(runtime, ctx, "ADJUSTABLE-ARRAY-P", LambdaList::fixed(&[ARRAY]), adjustable_array_p_builtin)?;
    register_one(runtime, ctx, "ARRAY-HAS-FILL-POINTER-P", LambdaList::fixed(&[ARRAY]), array_has_fill_pointer_p_builtin)?;
    register_one(runtime, ctx, "FILL-POINTER", LambdaList::fixed(&[ARRAY]), fill_pointer_builtin)?;
    register_one(runtime, ctx, "ADJUST-ARRAY", LambdaList::with_optional(&[ARRAY, DIMENSIONS], &[OPTIONS]), adjust_array_builtin)?;
    register_one(runtime, ctx, "VECTOR-PUSH", LambdaList::fixed(&[ELEMENT, ARRAY]), vector_push_builtin)?;
    register_one(runtime, ctx, "VECTOR-PUSH-EXTEND", LambdaList::with_optional(&[ELEMENT, ARRAY], &[EXTENSION]), vector_push_extend_builtin)?;
    register_one(runtime, ctx, "VECTOR-POP", LambdaList::fixed(&[ARRAY]), vector_pop_builtin)?;
    Ok(())
}
