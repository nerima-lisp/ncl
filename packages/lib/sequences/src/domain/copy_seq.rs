use ncl_object::{
    ArrayElementType, LispError, ObjectError, ObjectRef, Runtime, ThreadContext, Word,
    array_dimensions, array_row_major_ref, classify_object, make_simple_vector,
    make_specialized_array, with_rooted_slice,
};

use super::{list, sequence_value};

pub fn copy_seq_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, LispError> {
    match classify_object(ctx, object) {
        ObjectRef::Array(_) | ObjectRef::SpecializedArray(_) => {
            copy_array(ctx, runtime, object).map_err(LispError::from)
        }
        // check-added-lines: allow(wildcard) lists and strings retain their existing sequence copy path
        _ => {
            let sequence = sequence_value(ctx, object).map_err(LispError::from)?;
            list::copy_seq(ctx, runtime, sequence)
        }
    }
}

fn copy_array(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<Word, ObjectError> {
    let dimensions = array_dimensions(ctx, object)?;
    let length = match dimensions.as_slice() {
        [length] => *length,
        _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) COPY-SEQ supports rank-one arrays only
    };
    let length = if matches!(classify_object(ctx, object), ObjectRef::Array(_))
        && ncl_object::array::array_has_fill_pointer_p(ctx, object)?
    {
        ncl_object::array::fill_pointer(ctx, object)?
    } else {
        length
    };
    let element_type = ncl_object::array::array_element_type(ctx, object)?;
    let values = (0..length)
        .map(|index| array_row_major_ref(ctx, object, index))
        .collect::<Result<Vec<_>, _>>()?;
    with_rooted_slice(ctx, &values, |ctx, values| match element_type {
        ArrayElementType::T => make_simple_vector(ctx, runtime, values),
        element_type => make_specialized_array(ctx, runtime, element_type, values),
    })
}
