use super::{
    ArrayElementType, ArrayOptions, BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime,
    ThreadContext, Word, array_element_type, array_row_major_ref, array_row_major_set, array_shape,
    classify_object, make_array, pop_root, push_root, row_major_index,
};

fn with_word_roots<T>(
    ctx: &mut ThreadContext,
    values: &mut [Word],
    f: impl FnOnce(&mut ThreadContext, &[Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let tokens = values
        .iter_mut()
        .map(|value| push_root(ctx, value))
        .collect::<Vec<_>>();
    let result = f(ctx, values);
    for token in tokens.into_iter().rev() {
        if !pop_root(ctx, token) {
            return Err(ObjectError::Layout);
        }
    }
    result
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
    let mut roots = [
        args.required(0)?,
        args.required(1)?,
        args.get(2).unwrap_or(Word::NIL),
    ];
    let has_result = args.get(2).is_some();
    with_word_roots(ctx, &mut roots, |ctx, roots| {
        let left = roots[0];
        let right = roots[1];
        let dimensions = array_shape(ctx, left)?;
        if array_element_type(ctx, left)? != ArrayElementType::Bit
            || array_element_type(ctx, right)? != ArrayElementType::Bit
            || array_shape(ctx, right)? != dimensions
        {
            return Err(ObjectError::TypeError);
        }
        let mut result = bit_result(ctx, runtime, &dimensions, has_result.then_some(roots[2]))?;
        with_word_roots(ctx, std::slice::from_mut(&mut result), |ctx, result| {
            let total = dimensions
                .iter()
                .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
                .ok_or(ObjectError::Layout)?;
            for index in 0..total {
                array_row_major_set(
                    ctx,
                    result[0],
                    index,
                    Word::fixnum(i64::from(op(
                        bit_value(ctx, roots[0], index)?,
                        bit_value(ctx, roots[1], index)?,
                    ))),
                )?;
            }
            Ok(result[0])
        })
    })
}

pub(super) fn bit_not_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut roots = [args.required(0)?, args.get(1).unwrap_or(Word::NIL)];
    let has_result = args.get(1).is_some();
    with_word_roots(ctx, &mut roots, |ctx, roots| {
        let source = roots[0];
        let dimensions = array_shape(ctx, source)?;
        if array_element_type(ctx, source)? != ArrayElementType::Bit {
            return Err(ObjectError::TypeError);
        }
        let mut result = bit_result(ctx, runtime, &dimensions, has_result.then_some(roots[1]))?;
        with_word_roots(ctx, std::slice::from_mut(&mut result), |ctx, result| {
            let total = dimensions
                .iter()
                .try_fold(1usize, |size, dimension| size.checked_mul(*dimension))
                .ok_or(ObjectError::Layout)?;
            for index in 0..total {
                let value = bit_value(ctx, roots[0], index)?;
                array_row_major_set(ctx, result[0], index, Word::fixnum(i64::from(1 - value)))?;
            }
            Ok(result[0])
        })
    })
}

pub(super) fn bit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    let index = row_major_index(&array_shape(ctx, array)?, &args.as_slice()[1..])?;
    bit_value(ctx, array, index).map(|value| Word::fixnum(i64::from(value)))
}

pub(super) fn sbit_builtin(
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
                Some(0 | 1) => value,
                _ => return Err(ObjectError::TypeError),
            };
            array_row_major_set(ctx, array, index, value)?;
            Ok(value)
        }
        None => Ok(Word::fixnum(i64::from(current))),
    }
}

const fn bit_and(a: u8, b: u8) -> u8 {
    a & b
}
const fn bit_andc1(a: u8, b: u8) -> u8 {
    (1 - a) & b
}
const fn bit_andc2(a: u8, b: u8) -> u8 {
    a & (1 - b)
}
const fn bit_eqv(a: u8, b: u8) -> u8 {
    1 - (a ^ b)
}
const fn bit_ior(a: u8, b: u8) -> u8 {
    a | b
}
const fn bit_nand(a: u8, b: u8) -> u8 {
    1 - (a & b)
}
const fn bit_nor(a: u8, b: u8) -> u8 {
    1 - (a | b)
}
const fn bit_orc1(a: u8, b: u8) -> u8 {
    (1 - a) | b
}
const fn bit_orc2(a: u8, b: u8) -> u8 {
    a | (1 - b)
}
const fn bit_xor(a: u8, b: u8) -> u8 {
    a ^ b
}

macro_rules! binary_bit_builtin {
    ($name:ident, $op:ident) => {
        pub(super) fn $name(
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
