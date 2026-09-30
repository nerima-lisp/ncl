use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinArgs, MultipleValues, ObjectError, Runtime,
    ThreadContext, Word, array_row_major_set, make_array, with_rooted_slice,
};

use super::helpers::{dimension_values, flatten_initial_contents};
use super::symbol_text;

pub(super) fn make_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = dimension_values(ctx, args.required(0)?)?;
    let mut element_type = ArrayElementType::T;
    let mut initial_element = Word::NIL;
    let mut initial_element_set = false;
    let mut initial_contents = None;
    let mut adjustable = false;
    let mut fill_pointer = None;
    let mut displaced_to = None;
    let mut displaced_index_offset = 0;
    // check-added-lines: allow(index)
    let options = &args.as_slice()[1..];
    if !options.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    for &[key, value] in options.as_chunks::<2>().0 {
        match symbol_text(ctx, key)?
            .to_ascii_uppercase()
            .trim_start_matches(':')
        {
            "ELEMENT-TYPE" => {
                element_type = match symbol_text(ctx, value)?.to_ascii_uppercase().as_str() {
                    "T" => ArrayElementType::T,
                    "BIT" => ArrayElementType::Bit,
                    "CHARACTER" => ArrayElementType::Character,
                    "BASE-CHAR" => ArrayElementType::BaseChar,
                    "FIXNUM" => ArrayElementType::Fixnum,
                    "SIGNED-BYTE" => ArrayElementType::Signed,
                    "UNSIGNED-BYTE" => ArrayElementType::Unsigned,
                    "SINGLE-FLOAT" => ArrayElementType::SingleFloat,
                    "DOUBLE-FLOAT" => ArrayElementType::DoubleFloat,
                    _ => return Err(ObjectError::TypeError),
                }
            }
            "INITIAL-ELEMENT" => {
                initial_element = value;
                initial_element_set = true;
            }
            "INITIAL-CONTENTS" => initial_contents = Some(value),
            "ADJUSTABLE" => adjustable = value != Word::NIL,
            "FILL-POINTER" => {
                fill_pointer = Some(
                    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?,
                );
            }
            "DISPLACED-TO" => displaced_to = (value != Word::NIL).then_some(value),
            "DISPLACED-INDEX-OFFSET" => {
                displaced_index_offset =
                    usize::try_from(value.as_fixnum().ok_or(ObjectError::TypeError)?)
                        .map_err(|_| ObjectError::TypeError)?;
            }
            _ => return Err(ObjectError::TypeError),
        }
    }
    // CLHS 7.2 for MAKE-ARRAY: it is an error to supply both :initial-element
    // and :initial-contents.
    if initial_element_set && initial_contents.is_some() {
        return Err(ObjectError::TypeError);
    }
    if element_type == ArrayElementType::Bit && !initial_element_set {
        initial_element = Word::fixnum(0);
    }
    let array_options = ArrayOptions {
        element_type,
        initial_element,
        adjustable,
        fill_pointer,
        displaced_to,
        displaced_index_offset,
    };
    let Some(contents) = initial_contents else {
        return make_array(ctx, runtime, &dimensions, array_options);
    };
    // Flatten the nested `:initial-contents` structure before allocating so
    // that every referenced Lisp object is rooted across the allocation.
    let flattened = flatten_initial_contents(ctx, contents, &dimensions)?;
    with_rooted_slice(ctx, &flattened, |ctx, rooted| {
        let array = make_array(ctx, runtime, &dimensions, array_options)?;
        for (index, value) in rooted.iter().enumerate() {
            array_row_major_set(ctx, array, index, *value)?;
        }
        Ok(array)
    })
}
