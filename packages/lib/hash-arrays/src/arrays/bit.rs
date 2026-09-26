use super::*;

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

pub(super) fn bit_not_builtin(
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
