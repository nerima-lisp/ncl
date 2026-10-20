use ncl_object::{
    ArrayElementType, ArrayOptions, BuiltinArgs, MultipleValues, ObjectError, ObjectRef, Runtime,
    ThreadContext, Word, array_row_major_set, classify_object, make_array, with_rooted_slice,
};

use super::helpers::{dimension_values, flatten_initial_contents, list_values};
use super::symbol_text;

fn named_element_type(name: &str) -> Option<ArrayElementType> {
    match name.to_ascii_uppercase().as_str() {
        "NIL" | "T" => Some(ArrayElementType::T),
        "BIT" => Some(ArrayElementType::Bit),
        "CHARACTER" => Some(ArrayElementType::Character),
        "BASE-CHAR" => Some(ArrayElementType::BaseChar),
        "FIXNUM" | "INTEGER" => Some(ArrayElementType::Fixnum),
        "SIGNED-BYTE" => Some(ArrayElementType::Signed),
        "UNSIGNED-BYTE" => Some(ArrayElementType::Unsigned),
        "FLOAT" | "SHORT-FLOAT" | "SINGLE-FLOAT" => Some(ArrayElementType::SingleFloat),
        "DOUBLE-FLOAT" | "LONG-FLOAT" => Some(ArrayElementType::DoubleFloat),
        _ => None, // check-added-lines: allow(wildcard) reject unknown type names
    }
}

fn element_type_designator(
    ctx: &ThreadContext,
    designator: Word,
) -> Result<ArrayElementType, ObjectError> {
    match classify_object(ctx, designator) {
        ObjectRef::Symbol(_) => {
            named_element_type(&symbol_text(ctx, designator)?).ok_or(ObjectError::TypeError)
        }
        ObjectRef::Cons(_) => {
            let parts = list_values(ctx, designator)?;
            match parts.as_slice() {
                [name, width] => {
                    let name = symbol_text(ctx, *name)?.to_ascii_uppercase();
                    if width.as_fixnum().is_none_or(|width| width <= 0) {
                        return Err(ObjectError::TypeError);
                    }
                    match name.as_str() {
                        "SIGNED-BYTE" => Ok(ArrayElementType::Signed),
                        "UNSIGNED-BYTE" => Ok(ArrayElementType::Unsigned),
                        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported element type names
                    }
                }
                [name, lower, _upper] => {
                    if symbol_text(ctx, *name)?.eq_ignore_ascii_case("INTEGER") {
                        if lower.as_fixnum().ok_or(ObjectError::TypeError)? < 0 {
                            Ok(ArrayElementType::Signed)
                        } else {
                            Ok(ArrayElementType::Unsigned)
                        }
                    } else {
                        Err(ObjectError::TypeError)
                    }
                }
                _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject malformed type lists
            }
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported designators
    }
}

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
    let options = &args.as_slice()[1..]; // check-added-lines: allow(index) options trail the dimensions argument
    if !options.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    for &[key, value] in options.as_chunks::<2>().0 {
        match symbol_text(ctx, key)?
            .to_ascii_uppercase()
            .trim_start_matches(':')
        {
            "ELEMENT-TYPE" => {
                element_type = element_type_designator(ctx, value)?;
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
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown options
        }
    }
    // CLHS 7.2 for MAKE-ARRAY: it is an error to supply both :initial-element
    // and :initial-contents.
    if initial_element_set && initial_contents.is_some() {
        return Err(ObjectError::TypeError);
    }
    if !initial_element_set {
        initial_element = match element_type {
            ArrayElementType::Bit
            | ArrayElementType::Fixnum
            | ArrayElementType::Signed
            | ArrayElementType::Unsigned => Word::fixnum(0),
            ArrayElementType::Character | ArrayElementType::BaseChar => Word::character(0),
            ArrayElementType::T | ArrayElementType::SingleFloat | ArrayElementType::DoubleFloat => {
                initial_element
            }
        };
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
