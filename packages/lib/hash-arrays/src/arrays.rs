use ncl_object::array::{
    adjust_array, adjustable_array_p, array_displacement, array_element_type,
    array_has_fill_pointer_p, fill_pointer, vector_pop, vector_push, vector_push_extend,
};
use ncl_object::package::{nil, truth};
use ncl_object::{
    array_dimensions, array_row_major_ref, array_row_major_set, car, cdr, classify_object,
    make_array, make_cons, simple_vector_length, simple_vector_ref, string_length,
    ArrayElementType, ArrayOptions, BuiltinArgs, BuiltinName, LambdaList, MultipleValues,
    ObjectError, ObjectRef, Package, Parameter, ParameterType, Runtime, ThreadContext, Word,
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
const RESULT: Parameter = Parameter {
    name: BuiltinName::new("RESULT"),
    ty: ParameterType::Any,
};
const VALUE: Parameter = Parameter {
    name: BuiltinName::new("VALUE"),
    ty: ParameterType::Any,
};

fn array_element_type_symbol(
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
        ObjectRef::Array(_) | ObjectRef::SpecializedArray(_) => array_dimensions(ctx, value),
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
        Ok(Word::fixnum(
            i64::try_from(value).map_err(|_| ObjectError::Layout)?,
        ))
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
        .map(|value| {
            usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                .map_err(|_| ObjectError::TypeError)
        })
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
    let (index, adjusted) = vector_push_extend(
        ctx,
        runtime,
        args.required(0)?,
        args.required(1)?,
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
    let offset = row_major_index(&array_shape(ctx, array)?, &args.as_slice()[1..])?;
    array_row_major_ref(ctx, array, offset)
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

fn bit_value(ctx: &ThreadContext, array: Word, index: usize) -> Result<u8, ObjectError> {
    match array_row_major_ref(ctx, array, index)?.as_fixnum() {
        Some(0) => Ok(0),
        Some(1) => Ok(1),
        _ => Err(ObjectError::TypeError),
    }
}

fn bit_result(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    dimensions: &[usize],
    result: Option<Word>,
) -> Result<Word, ObjectError> {
    let result = match result {
        Some(result) => result,
        None => make_array(
            ctx,
            runtime,
            dimensions,
            ArrayOptions {
                element_type: ArrayElementType::Bit,
                initial_element: Word::fixnum(0),
                adjustable: false,
                fill_pointer: None,
                displaced_to: None,
                displaced_index_offset: 0,
            },
        )?,
    };
    if array_element_type(ctx, result)? != ArrayElementType::Bit
        || array_shape(ctx, result)? != dimensions
    {
        return Err(ObjectError::TypeError);
    }
    Ok(result)
}

fn bit_binary_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    op: fn(u8, u8) -> u8,
) -> Result<Word, ObjectError> {
    let left = args.required(0)?;
    let right = args.required(1)?;
    let dimensions = array_shape(ctx, left)?;
    if array_element_type(ctx, left)? != ArrayElementType::Bit
        || array_element_type(ctx, right)? != ArrayElementType::Bit
        || array_shape(ctx, right)? != dimensions
    {
        return Err(ObjectError::TypeError);
    }
    let result = bit_result(ctx, runtime, &dimensions, args.get(2))?;
    let total = dimensions
        .iter()
        .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    for index in 0..total {
        array_row_major_set(
            ctx,
            result,
            index,
            Word::fixnum(i64::from(op(
                bit_value(ctx, left, index)?,
                bit_value(ctx, right, index)?,
            ))),
        )?;
    }
    Ok(result)
}

fn bit_not_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let source = args.required(0)?;
    let dimensions = array_shape(ctx, source)?;
    if array_element_type(ctx, source)? != ArrayElementType::Bit {
        return Err(ObjectError::TypeError);
    }
    let result = bit_result(ctx, runtime, &dimensions, args.get(1))?;
    let total = dimensions
        .iter()
        .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    for index in 0..total {
        let value = bit_value(ctx, source, index)?;
        array_row_major_set(ctx, result, index, Word::fixnum(i64::from(1 - value)))?;
    }
    Ok(result)
}

fn bit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    let index = row_major_index(&array_shape(ctx, array)?, &args.as_slice()[1..])?;
    bit_value(ctx, array, index).map(|value| Word::fixnum(i64::from(value)))
}

fn sbit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    let shape = array_shape(ctx, array)?;
    if !matches!(classify_object(ctx, array), ObjectRef::SpecializedArray(_))
        || array_element_type(ctx, array)? != ArrayElementType::Bit
        || shape.len() != 1
    {
        return Err(ObjectError::TypeError);
    }
    let index = args.required(1)?;
    let index = usize::try_from(index.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)?;
    let current = bit_value(ctx, array, index)?;
    match args.get(2) {
        Some(value) => {
            let value = match value.as_fixnum() {
                Some(0) | Some(1) => value,
                _ => return Err(ObjectError::TypeError),
            };
            array_row_major_set(ctx, array, index, value)?;
            Ok(value)
        }
        None => Ok(Word::fixnum(i64::from(current))),
    }
}

fn bit_and(a: u8, b: u8) -> u8 {
    a & b
}
fn bit_andc1(a: u8, b: u8) -> u8 {
    (1 - a) & b
}
fn bit_andc2(a: u8, b: u8) -> u8 {
    a & (1 - b)
}
fn bit_eqv(a: u8, b: u8) -> u8 {
    1 - (a ^ b)
}
fn bit_ior(a: u8, b: u8) -> u8 {
    a | b
}
fn bit_nand(a: u8, b: u8) -> u8 {
    1 - (a & b)
}
fn bit_nor(a: u8, b: u8) -> u8 {
    1 - (a | b)
}
fn bit_orc1(a: u8, b: u8) -> u8 {
    (1 - a) | b
}
fn bit_orc2(a: u8, b: u8) -> u8 {
    a | (1 - b)
}
fn bit_xor(a: u8, b: u8) -> u8 {
    a ^ b
}

macro_rules! binary_bit_builtin {
    ($name:ident, $op:ident) => {
        fn $name(
            ctx: &mut ThreadContext,
            runtime: &Runtime,
            args: &BuiltinArgs<'_>,
            values: &mut MultipleValues,
        ) -> Result<Word, ObjectError> {
            let _ = values;
            bit_binary_builtin(ctx, runtime, args, $op)
        }
    };
}
binary_bit_builtin!(bit_and_builtin, bit_and);
binary_bit_builtin!(bit_andc1_builtin, bit_andc1);
binary_bit_builtin!(bit_andc2_builtin, bit_andc2);
binary_bit_builtin!(bit_eqv_builtin, bit_eqv);
binary_bit_builtin!(bit_ior_builtin, bit_ior);
binary_bit_builtin!(bit_nand_builtin, bit_nand);
binary_bit_builtin!(bit_nor_builtin, bit_nor);
binary_bit_builtin!(bit_orc1_builtin, bit_orc1);
binary_bit_builtin!(bit_orc2_builtin, bit_orc2);
binary_bit_builtin!(bit_xor_builtin, bit_xor);

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
    register_one(
        runtime,
        ctx,
        "ADJUSTABLE-ARRAY-P",
        LambdaList::fixed(&[ARRAY]),
        adjustable_array_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-HAS-FILL-POINTER-P",
        LambdaList::fixed(&[ARRAY]),
        array_has_fill_pointer_p_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "FILL-POINTER",
        LambdaList::fixed(&[ARRAY]),
        fill_pointer_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ADJUST-ARRAY",
        LambdaList::with_optional(&[ARRAY, DIMENSIONS], &[OPTIONS]),
        adjust_array_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTOR-PUSH",
        LambdaList::fixed(&[ELEMENT, ARRAY]),
        vector_push_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTOR-PUSH-EXTEND",
        LambdaList::with_optional(&[ELEMENT, ARRAY], &[EXTENSION]),
        vector_push_extend_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "VECTOR-POP",
        LambdaList::fixed(&[ARRAY]),
        vector_pop_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-ELEMENT-TYPE",
        LambdaList::fixed(&[ARRAY]),
        array_element_type_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-DISPLACEMENT",
        LambdaList::fixed(&[ARRAY]),
        array_displacement_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "ARRAY-ROW-MAJOR-INDEX",
        LambdaList::with_rest(&[ARRAY], INDEX),
        array_row_major_index_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT",
        LambdaList::with_rest(&[ARRAY], INDEX),
        bit_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "SBIT",
        LambdaList::with_optional(&[ARRAY, INDEX], &[VALUE]),
        sbit_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-AND",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_and_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ANDC1",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_andc1_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ANDC2",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_andc2_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-EQV",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_eqv_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-IOR",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_ior_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-NAND",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_nand_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-NOR",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_nor_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-NOT",
        LambdaList::with_optional(&[ARRAY], &[RESULT]),
        bit_not_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ORC1",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_orc1_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-ORC2",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_orc2_builtin,
    )?;
    register_one(
        runtime,
        ctx,
        "BIT-XOR",
        LambdaList::with_optional(&[ARRAY, INDEX], &[RESULT]),
        bit_xor_builtin,
    )?;
    Ok(())
}
