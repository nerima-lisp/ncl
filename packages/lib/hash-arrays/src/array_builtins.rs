//! Builtin adapters for array and vector operations.

use super::array_helpers::*;
use super::*;

fn array_word(ctx: &ThreadContext, word: Word) -> Result<Word, ObjectError> {
    match classify_object(ctx, word) {
        ncl_object::ObjectRef::Array(_)
        | ncl_object::ObjectRef::SimpleVector(_)
        | ncl_object::ObjectRef::SpecializedArray(_) => Ok(word),
        _ => Err(ObjectError::TypeError),
    }
}

fn rank_dimensions(ctx: &ThreadContext, array: Word) -> Result<Vec<usize>, ObjectError> {
    match classify_object(ctx, array) {
        ncl_object::ObjectRef::Array(_) => ncl_object::array_dimensions(ctx, array),
        ncl_object::ObjectRef::SimpleVector(_) => {
            Ok(vec![ncl_object::simple_vector_length(ctx, array)?])
        }
        ncl_object::ObjectRef::SpecializedArray(_) => {
            let mut length = 0;
            while ncl_object::specialized_array_ref(ctx, array, length).is_ok() {
                length += 1;
            }
            Ok(vec![length])
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn row_ref(ctx: &ThreadContext, array: Word, index: usize) -> Result<Word, ObjectError> {
    match classify_object(ctx, array) {
        ncl_object::ObjectRef::Array(_) => ncl_object::array_row_major_ref(ctx, array, index),
        ncl_object::ObjectRef::SimpleVector(_) => ncl_object::simple_vector_ref(ctx, array, index),
        ncl_object::ObjectRef::SpecializedArray(_) => {
            ncl_object::specialized_array_ref(ctx, array, index)
        }
        _ => Err(ObjectError::TypeError),
    }
}

fn fixnum(word: Word) -> Result<usize, ObjectError> {
    usize::try_from(
        Fixnum::try_from_word(word)
            .map_err(|_| ObjectError::TypeError)?
            .value(),
    )
    .map_err(|_| ObjectError::TypeError)
}

fn total(ctx: &ThreadContext, array: Word) -> Result<usize, ObjectError> {
    rank_dimensions(ctx, array)?
        .into_iter()
        .try_fold(1_usize, |a, b| a.checked_mul(b).ok_or(ObjectError::Layout))
}

pub(crate) fn array_rank_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let rank = rank_dimensions(ctx, array_word(ctx, args.required(0)?)?)?.len();
    i64::try_from(rank)
        .map(Word::fixnum)
        .map_err(|_| ObjectError::Layout)
}

pub(crate) fn array_dimensions_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = rank_dimensions(ctx, array_word(ctx, args.required(0)?)?)?;
    let values = dimensions
        .into_iter()
        .map(|v| {
            i64::try_from(v)
                .map(Word::fixnum)
                .map_err(|_| ObjectError::Layout)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ncl_object::make_simple_vector(ctx, runtime, &values)
}

pub(crate) fn array_total_size_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    i64::try_from(total(ctx, array_word(ctx, args.required(0)?)?)?)
        .map(Word::fixnum)
        .map_err(|_| ObjectError::Layout)
}

pub(crate) fn fill_pointer_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value =
        ncl_object::array_fill_pointer(ctx, args.required(0)?)?.ok_or(ObjectError::TypeError)?;
    Ok(Word::fixnum(
        i64::try_from(value).map_err(|_| ObjectError::Layout)?,
    ))
}

pub(crate) fn array_has_fill_pointer_p_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(
        if ncl_object::array_fill_pointer(ctx, args.required(0)?)?.is_some() {
            Word::TRUE
        } else {
            Word::NIL
        },
    )
}

pub(crate) fn vector_push_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let value = args.required(0)?;
    let vector = args.required(1)?;
    let pointer = ncl_object::array_fill_pointer(ctx, vector)?.ok_or(ObjectError::TypeError)?;
    if pointer >= total(ctx, vector)? {
        return Ok(Word::NIL);
    }
    ncl_object::array_row_major_set(ctx, vector, pointer, value)?;
    ncl_object::array_set_fill_pointer(ctx, vector, pointer + 1)?;
    Ok(Word::fixnum(
        i64::try_from(pointer).map_err(|_| ObjectError::Layout)?,
    ))
}

pub(crate) fn vector_pop_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let vector = args.required(0)?;
    let pointer = ncl_object::array_fill_pointer(ctx, vector)?.ok_or(ObjectError::TypeError)?;
    let next = pointer.checked_sub(1).ok_or(ObjectError::TypeError)?;
    let value = row_ref(ctx, vector, next)?;
    ncl_object::array_set_fill_pointer(ctx, vector, next)?;
    Ok(value)
}

pub(crate) fn array_displacement_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let (target, offset) = ncl_object::array_displacement(ctx, args.required(0)?)?;
    values.set(&[
        target,
        Word::fixnum(i64::try_from(offset).map_err(|_| ObjectError::Layout)?),
    ]);
    Ok(target)
}

pub(crate) fn make_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = dimensions_argument(ctx, args.required(0)?)?;
    let fill_pointer = keyword(ctx, args, "FILL-POINTER")?
        .filter(|word| *word != Word::NIL)
        .map(fixnum)
        .transpose()?;
    let displaced_to = keyword(ctx, args, "DISPLACED-TO")?;
    let offset = keyword(ctx, args, "DISPLACED-INDEX-OFFSET")?
        .map(fixnum)
        .transpose()?
        .unwrap_or(0);
    let adjustable = keyword(ctx, args, "ADJUSTABLE")?.is_some_and(|word| word != Word::NIL);
    let initial_element = keyword(ctx, args, "INITIAL-ELEMENT")?.unwrap_or(Word::NIL);
    let kind = keyword(ctx, args, "ELEMENT-TYPE")?
        .map(|word| element_type(ctx, word))
        .transpose()?
        .unwrap_or(ArrayElementType::T);
    ncl_object::make_array(
        ctx,
        runtime,
        &dimensions,
        ncl_object::ArrayOptions {
            element_type: kind,
            initial_element,
            adjustable,
            fill_pointer,
            displaced_to,
            displaced_index_offset: offset,
        },
    )
}

pub(crate) fn adjust_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let old = array_word(ctx, args.required(0)?)?;
    let dimensions = dimensions_argument(ctx, args.required(1)?)?;
    let kind = keyword(ctx, args, "ELEMENT-TYPE")?
        .map(|word| element_type(ctx, word))
        .transpose()?
        .unwrap_or(ArrayElementType::T);
    let result = ncl_object::make_array(
        ctx,
        runtime,
        &dimensions,
        ncl_object::ArrayOptions {
            element_type: kind,
            initial_element: Word::NIL,
            adjustable: true,
            fill_pointer: None,
            displaced_to: None,
            displaced_index_offset: 0,
        },
    )?;
    let count = total(ctx, old)?.min(total(ctx, result)?);
    for index in 0..count {
        let value = row_ref(ctx, old, index)?;
        ncl_object::array_row_major_set(ctx, result, index, value)?;
    }
    values.clear();
    Ok(result)
}

pub(crate) fn aref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = array_word(ctx, args.required(0)?)?;
    let dimensions = rank_dimensions(ctx, array)?;
    if args.len() != dimensions.len() + 1 {
        return Err(ObjectError::TypeError);
    }
    let mut index = 0usize;
    for (ordinal, dimension) in dimensions.into_iter().enumerate() {
        let word = args.get(ordinal + 1).ok_or(ObjectError::TypeError)?;
        let subscript = fixnum(word)?;
        if subscript >= dimension {
            return Err(ObjectError::TypeError);
        }
        index = index
            .checked_mul(dimension)
            .and_then(|v| v.checked_add(subscript))
            .ok_or(ObjectError::Layout)?;
    }
    row_ref(ctx, array, index)
}

pub(crate) fn row_major_aref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = array_word(ctx, args.required(0)?)?;
    row_ref(ctx, array, fixnum(args.required(1)?)?)
}

pub(crate) fn svref_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    if !matches!(
        classify_object(ctx, array),
        ncl_object::ObjectRef::SimpleVector(_)
    ) {
        return Err(ObjectError::TypeError);
    }
    ncl_object::simple_vector_ref(ctx, array, fixnum(args.required(1)?)?)
}

pub(crate) fn bit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    if !matches!(
        classify_object(ctx, array),
        ncl_object::ObjectRef::SpecializedArray(_)
    ) || ncl_object::specialized_array_element_type(ctx, array)? != ArrayElementType::Bit
    {
        return Err(ObjectError::TypeError);
    }
    ncl_object::specialized_array_ref(ctx, array, fixnum(args.required(1)?)?)
}

pub(crate) fn sbit_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let array = args.required(0)?;
    if !matches!(
        classify_object(ctx, array),
        ncl_object::ObjectRef::SpecializedArray(_)
    ) || ncl_object::specialized_array_element_type(ctx, array)? != ArrayElementType::Bit
    {
        return Err(ObjectError::TypeError);
    }
    let index = fixnum(args.required(1)?)?;
    let value = args.required(2)?;
    ncl_object::specialized_array_set(ctx, array, index, value)?;
    Ok(value)
}

fn bit_binary(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    op: fn(bool, bool) -> bool,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let left = args.required(0)?;
    let right = args.required(1)?;
    if ncl_object::specialized_array_element_type(ctx, left)? != ArrayElementType::Bit
        || ncl_object::specialized_array_element_type(ctx, right)? != ArrayElementType::Bit
    {
        return Err(ObjectError::TypeError);
    }
    let shape = rank_dimensions(ctx, left)?;
    if shape != rank_dimensions(ctx, right)? {
        return Err(ObjectError::TypeError);
    }
    let mut values = Vec::with_capacity(total(ctx, left)?);
    for index in 0..total(ctx, left)? {
        let a = row_ref(ctx, left, index)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?
            == 1;
        let b = row_ref(ctx, right, index)?
            .as_fixnum()
            .ok_or(ObjectError::TypeError)?
            == 1;
        values.push(Word::fixnum(i64::from(op(a, b))));
    }
    ncl_object::make_specialized_array(ctx, runtime, ArrayElementType::Bit, &values)
}

pub(crate) fn bit_and(
    ctx: &mut ThreadContext,
    r: &Runtime,
    a: &BuiltinArgs<'_>,
    v: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    bit_binary(ctx, r, a, |x, y| x & y, v)
}
pub(crate) fn bit_ior(
    ctx: &mut ThreadContext,
    r: &Runtime,
    a: &BuiltinArgs<'_>,
    v: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    bit_binary(ctx, r, a, |x, y| x | y, v)
}
pub(crate) fn bit_xor(
    ctx: &mut ThreadContext,
    r: &Runtime,
    a: &BuiltinArgs<'_>,
    v: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    bit_binary(ctx, r, a, |x, y| x ^ y, v)
}

pub(crate) fn vector_push_extend_builtin(
    ctx: &mut ThreadContext,
    _: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let _ = args.required(0)?;
    let vector = array_word(ctx, args.required(1)?)?;
    let _ = args.required(2)?;
    let _ = total(ctx, vector)?;
    Err(ObjectError::Unsupported)
}

pub(crate) fn register_one(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &'static str,
    arity: u8,
    function: ncl_object::RustBuiltin,
) -> Result<(), ObjectError> {
    let parameters: &'static [Parameter] = Box::leak(
        (0..usize::from(arity))
            .map(|_| Parameter {
                name: BuiltinName::new("ARG"),
                ty: ParameterType::Any,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let descriptor = Builtin {
        lambda_list: LambdaList::fixed(parameters),
        convention: BuiltinConvention::Direct(ncl_object::Arity::exact(arity)),
    };
    runtime
        .register_builtin(
            ctx,
            BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new(name)),
            BuiltinImplementation::direct(descriptor, function),
        )
        .map(|_| ())
}

pub(crate) fn register_aref(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    const REQUIRED: &[Parameter] = &[Parameter {
        name: BuiltinName::new("ARRAY"),
        ty: ParameterType::Any,
    }];
    runtime.register_builtin(
        ctx,
        BuiltinIdentifier::new(BuiltinPackage::CommonLisp, BuiltinName::new("AREF")),
        BuiltinImplementation::direct(
            Builtin {
                lambda_list: LambdaList::with_rest(
                    REQUIRED,
                    Parameter {
                        name: BuiltinName::new("INDEX"),
                        ty: ParameterType::Any,
                    },
                ),
                convention: BuiltinConvention::Direct(ncl_object::Arity::exact(2)),
            },
            aref_builtin,
        ),
    )?;
    Ok(())
}
