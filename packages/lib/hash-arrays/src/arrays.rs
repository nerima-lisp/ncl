//! Array and vector builtin implementations.

use ncl_object::{
    Builtin, BuiltinImplementation, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    array_dimensions, array_row_major_ref, classify_object, simple_vector_length,
    simple_vector_ref, specialized_array_length,
};

fn index(value: Word) -> Result<usize, ObjectError> {
    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
        .map_err(|_| ObjectError::TypeError)
}
fn truth(value: bool) -> Word {
    if value { Word::TRUE } else { Word::NIL }
}
fn vector_length(ctx: &ThreadContext, object: Word) -> Result<usize, ObjectError> {
    match classify_object(ctx, object) {
        ObjectRef::SimpleVector(_) => simple_vector_length(ctx, object),
        ObjectRef::SpecializedArray(_) => specialized_array_length(ctx, object),
        ObjectRef::Array(_) => {
            let dimensions = array_dimensions(ctx, object)?;
            if dimensions.len() == 1 {
                Ok(dimensions[0])
            } else {
                Err(ObjectError::TypeError)
            }
        }
        _ => Err(ObjectError::TypeError),
    }
}
fn unavailable(
    _: &mut ThreadContext,
    _: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    Err(ObjectError::Unsupported)
}
fn arrayp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(truth(matches!(
        classify_object(ctx, args[0]),
        ObjectRef::Array(_) | ObjectRef::SpecializedArray(_)
    )))
}
fn vectorp(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(truth(vector_length(ctx, args[0]).is_ok()))
}
fn simple_vector_p(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(truth(matches!(
        classify_object(ctx, args[0]),
        ObjectRef::SimpleVector(_)
    )))
}
fn bit_vector_p(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    Ok(truth(
        matches!(
            classify_object(ctx, args[0]),
            ObjectRef::SpecializedArray(_)
        ) && ncl_object::specialized_array_element_type(ctx, args[0])
            == Ok(ncl_object::ArrayElementType::Bit),
    ))
}
fn rank(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let rank = match classify_object(ctx, args[0]) {
        ObjectRef::Array(_) => array_dimensions(ctx, args[0])?.len(),
        ObjectRef::SimpleVector(_) | ObjectRef::SpecializedArray(_) => 1,
        _ => return Err(ObjectError::TypeError),
    };
    Ok(Word::fixnum(
        i64::try_from(rank).map_err(|_| ObjectError::Layout)?,
    ))
}
fn total_size(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let size = match classify_object(ctx, args[0]) {
        ObjectRef::SimpleVector(_) | ObjectRef::SpecializedArray(_) => vector_length(ctx, args[0])?,
        ObjectRef::Array(_) => array_dimensions(ctx, args[0])?
            .into_iter()
            .try_fold(1usize, |a, b| a.checked_mul(b))
            .ok_or(ObjectError::Layout)?,
        _ => return Err(ObjectError::TypeError),
    };
    Ok(Word::fixnum(
        i64::try_from(size).map_err(|_| ObjectError::Layout)?,
    ))
}
fn aref(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    array_row_major_ref(ctx, args[0], index(args[1])?)
}
fn svref(
    ctx: &mut ThreadContext,
    args: &[Word],
    _: &mut ncl_object::MultipleValues,
) -> Result<Word, ObjectError> {
    if args.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    match classify_object(ctx, args[0]) {
        ObjectRef::SimpleVector(_) => simple_vector_ref(ctx, args[0], index(args[1])?),
        ObjectRef::SpecializedArray(_) => {
            ncl_object::specialized_array_ref(ctx, args[0], index(args[1])?)
        }
        ObjectRef::Array(_) => array_row_major_ref(ctx, args[0], index(args[1])?),
        _ => Err(ObjectError::TypeError),
    }
}

/// Register array/vector names. Allocation and mutation remain unsupported until object accessors expose a runtime handle to callbacks.
pub fn register(runtime: &Runtime) -> Result<(), ObjectError> {
    let entries: &[(&str, ncl_object::RustBuiltin)] = &[
        ("MAKE-ARRAY", unavailable),
        ("AREF", aref),
        ("ROW-MAJOR-AREF", aref),
        ("SVREF", svref),
        ("ARRAYP", arrayp),
        ("VECTORP", vectorp),
        ("SIMPLE-VECTOR-P", simple_vector_p),
        ("BIT-VECTOR-P", bit_vector_p),
        ("SIMPLE-BIT-VECTOR-P", bit_vector_p),
        ("ARRAY-RANK", rank),
        ("ARRAY-TOTAL-SIZE", total_size),
        ("ADJUST-ARRAY", unavailable),
        ("FILL-POINTER", unavailable),
        ("VECTOR-PUSH", unavailable),
        ("VECTOR-PUSH-EXTEND", unavailable),
        ("VECTOR-POP", unavailable),
        ("BIT", unavailable),
        ("SBIT", unavailable),
        ("BIT-AND", unavailable),
        ("BIT-ANDC1", unavailable),
        ("BIT-ANDC2", unavailable),
        ("BIT-EQV", unavailable),
        ("BIT-IOR", unavailable),
        ("BIT-NAND", unavailable),
        ("BIT-NOR", unavailable),
        ("BIT-NOT", unavailable),
        ("BIT-ORC1", unavailable),
        ("BIT-ORC2", unavailable),
        ("BIT-XOR", unavailable),
    ];
    let mut ctx = ThreadContext::new();
    ctx.register(runtime)?;
    for (name, function) in entries {
        runtime.register_builtin(
            &mut ctx,
            "COMMON-LISP",
            name,
            BuiltinImplementation::direct(
                Builtin {
                    arity: 0,
                    direct: false,
                    lambda_list: "&rest args",
                },
                *function,
            ),
        )?;
    }
    Ok(())
}
