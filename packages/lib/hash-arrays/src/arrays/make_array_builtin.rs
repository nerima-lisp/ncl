use ncl_object::{
    ArrayElementType, ArrayOptions, Bignum, BuiltinArgs, MultipleValues, ObjectError, ObjectRef,
    Runtime, ThreadContext, Word, array_row_major_set, bignum_limbs, bignum_sign, classify_object,
    make_array, with_rooted_slice,
};

use super::helpers::{dimension_values, flatten_initial_contents, list_values};
use super::symbol_text;

fn named_element_type(name: &str) -> Option<ArrayElementType> {
    match name.to_ascii_uppercase().as_str() {
        "NIL" | "T" => Some(ArrayElementType::T),
        "BIT" => Some(ArrayElementType::Bit),
        "CHARACTER" => Some(ArrayElementType::Character),
        "BASE-CHAR" => Some(ArrayElementType::BaseChar),
        "FIXNUM" => Some(ArrayElementType::Fixnum),
        "INTEGER" | "SIGNED-BYTE" => Some(ArrayElementType::Signed),
        "UNSIGNED-BYTE" => Some(ArrayElementType::Unsigned),
        "FLOAT" | "SHORT-FLOAT" | "SINGLE-FLOAT" => Some(ArrayElementType::SingleFloat),
        "DOUBLE-FLOAT" | "LONG-FLOAT" => Some(ArrayElementType::DoubleFloat),
        _ => None, // check-added-lines: allow(wildcard) reject unknown type names
    }
}

fn integer_is_negative(ctx: &ThreadContext, value: Word) -> Result<bool, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) => Ok(value < 0),
        ObjectRef::Bignum(value) => bignum_sign(ctx, Bignum::from_word(value)),
        // check-added-lines: allow(wildcard) type bounds must be integers
        _ => Err(ObjectError::TypeError),
    }
}

fn integer_bound(ctx: &ThreadContext, value: Word) -> Result<Option<bool>, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(_) | ObjectRef::Bignum(_) => Ok(Some(integer_is_negative(ctx, value)?)),
        ObjectRef::Symbol(_) if symbol_text(ctx, value)? == "*" => Ok(None),
        ObjectRef::Cons(_) => {
            let bounds = list_values(ctx, value)?;
            match bounds.as_slice() {
                [bound] => Ok(Some(integer_is_negative(ctx, *bound)?)),
                // check-added-lines: allow(wildcard) exclusive bounds must contain one integer.
                _ => Err(ObjectError::TypeError),
            }
        }
        // check-added-lines: allow(wildcard) malformed integer bounds are rejected
        _ => Err(ObjectError::TypeError),
    }
}

fn byte_size(ctx: &ThreadContext, value: Word) -> Result<(), ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Symbol(_) if symbol_text(ctx, value)? == "*" => Ok(()),
        ObjectRef::Fixnum(size) if size > 0 => Ok(()),
        ObjectRef::Bignum(value) => {
            let value = Bignum::from_word(value);
            if bignum_sign(ctx, value)? || bignum_limbs(ctx, value)?.iter().all(|limb| *limb == 0) {
                Err(ObjectError::TypeError)
            } else {
                Ok(())
            }
        }
        // check-added-lines: allow(wildcard) unsupported byte sizes are rejected
        _ => Err(ObjectError::TypeError),
    }
}

fn mod_element_type(ctx: &ThreadContext, size: Word) -> Result<ArrayElementType, ObjectError> {
    if matches!(classify_object(ctx, size), ObjectRef::Symbol(_)) {
        return Err(ObjectError::TypeError);
    }
    byte_size(ctx, size)?;
    Ok(if matches!(size.as_fixnum(), Some(1 | 2)) {
        ArrayElementType::Bit
    } else {
        ArrayElementType::Unsigned
    })
}

fn integer_element_type(
    ctx: &ThreadContext,
    lower: Option<Word>,
    upper: Option<Word>,
) -> Result<ArrayElementType, ObjectError> {
    let lower = lower.map(|bound| integer_bound(ctx, bound)).transpose()?;
    if let Some(upper) = upper {
        integer_bound(ctx, upper)?;
    }
    Ok(if lower.flatten().is_some_and(|is_negative| !is_negative) {
        ArrayElementType::Unsigned
    } else {
        ArrayElementType::Signed
    })
}

fn mod_element(value: Word, modulus: Word, ctx: &ThreadContext) -> Result<bool, ObjectError> {
    match classify_object(ctx, value) {
        ObjectRef::Fixnum(value) if value >= 0 => match classify_object(ctx, modulus) {
            ObjectRef::Fixnum(modulus) => Ok(value < modulus),
            ObjectRef::Bignum(_) => Ok(true),
            // The parsed MOD size is a positive integer.
            // check-added-lines: allow(wildcard) ObjectRef is non-exhaustive.
            _ => Err(ObjectError::TypeError),
        },
        ObjectRef::Bignum(value) if !bignum_sign(ctx, Bignum::from_word(value))? => {
            let ObjectRef::Bignum(modulus) = classify_object(ctx, modulus) else {
                return Ok(false);
            };
            let value_limbs = bignum_limbs(ctx, Bignum::from_word(value))?;
            let modulus_limbs = bignum_limbs(ctx, Bignum::from_word(modulus))?;
            Ok(value_limbs.len() < modulus_limbs.len()
                || (value_limbs.len() == modulus_limbs.len()
                    && value_limbs.iter().rev().cmp(modulus_limbs.iter().rev())
                        == std::cmp::Ordering::Less))
        }
        // MOD contains only non-negative integers.
        // check-added-lines: allow(wildcard) ObjectRef is non-exhaustive.
        _ => Ok(false),
    }
}

fn fast_element_type_designator(
    ctx: &ThreadContext,
    designator: Word,
) -> Result<(ArrayElementType, Option<Word>), ObjectError> {
    match classify_object(ctx, designator) {
        ObjectRef::Symbol(_) => Ok((
            named_element_type(&symbol_text(ctx, designator)?).ok_or(ObjectError::TypeError)?,
            None,
        )),
        ObjectRef::Cons(_) => {
            let parts = list_values(ctx, designator)?;
            match parts.as_slice() {
                [name] => match symbol_text(ctx, *name)?.to_ascii_uppercase().as_str() {
                    "INTEGER" => Ok((integer_element_type(ctx, None, None)?, None)),
                    "SIGNED-BYTE" => Ok((ArrayElementType::Signed, None)),
                    "UNSIGNED-BYTE" => Ok((ArrayElementType::Unsigned, None)),
                    _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported element type names
                },
                [name, width] => {
                    let name = symbol_text(ctx, *name)?.to_ascii_uppercase();
                    match name.as_str() {
                        "INTEGER" => Ok((integer_element_type(ctx, Some(*width), None)?, None)),
                        "MOD" => Ok((mod_element_type(ctx, *width)?, Some(*width))),
                        "SIGNED-BYTE" => {
                            byte_size(ctx, *width)?;
                            Ok((ArrayElementType::Signed, None))
                        }
                        "UNSIGNED-BYTE" => {
                            byte_size(ctx, *width)?;
                            Ok((ArrayElementType::Unsigned, None))
                        }
                        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported element type names
                    }
                }
                [name, lower, upper]
                    if symbol_text(ctx, *name)?.eq_ignore_ascii_case("INTEGER") =>
                {
                    Ok((integer_element_type(ctx, Some(*lower), Some(*upper))?, None))
                }
                _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject malformed type lists
            }
        }
        _ => Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unsupported designators
    }
}

fn element_type_designator(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    designator: Word,
) -> Result<(ArrayElementType, Option<Word>), ObjectError> {
    let designator = ncl_object::HeapRootedWord::new(ctx, designator)?;
    if let Ok(element_type) = fast_element_type_designator(ctx, designator.get()) {
        return Ok(element_type);
    }
    let expanded = ncl_types::expand_deftype_specifier(ctx, runtime, designator.get())
        .map_err(|_| ObjectError::TypeError)?;
    let expanded = ncl_object::HeapRootedWord::new(ctx, expanded)?;
    if let Ok(element_type) = fast_element_type_designator(ctx, expanded.get()) {
        return Ok(element_type);
    }
    let specifier = ncl_types::parse_type_specifier_with_runtime(ctx, runtime, expanded.get())
        .map_err(|_| ObjectError::TypeError)?;

    if let Some(element_type) = array_element_type_from_specifier(&specifier) {
        return Ok((element_type, None));
    }
    if contains_unexpanded_deftype(&specifier) {
        return Err(ObjectError::TypeError);
    }
    Ok((ArrayElementType::T, None))
}

const fn array_element_type_from_specifier(
    specifier: &ncl_types::TypeSpecifier,
) -> Option<ArrayElementType> {
    match specifier {
        ncl_types::TypeSpecifier::Class(_) => Some(ArrayElementType::T),
        ncl_types::TypeSpecifier::Named(named) => Some(match named {
            ncl_types::NamedType::Character => ArrayElementType::Character,
            ncl_types::NamedType::BaseChar => ArrayElementType::BaseChar,
            ncl_types::NamedType::Fixnum => ArrayElementType::Fixnum,
            ncl_types::NamedType::Integer => ArrayElementType::Signed,
            ncl_types::NamedType::Float
            | ncl_types::NamedType::ShortFloat
            | ncl_types::NamedType::SingleFloat => ArrayElementType::SingleFloat,
            ncl_types::NamedType::DoubleFloat | ncl_types::NamedType::LongFloat => {
                ArrayElementType::DoubleFloat
            }
            _ => ArrayElementType::T, // check-added-lines: allow(wildcard) other named types upgrade to T
        }),
        _ => None, // check-added-lines: allow(wildcard) other parsed specifiers use T
    }
}

fn contains_unexpanded_deftype(specifier: &ncl_types::TypeSpecifier) -> bool {
    match specifier {
        ncl_types::TypeSpecifier::Deftype { name, args } => match name.as_str() {
            "MOD" => {
                !matches!(args.as_slice(), [ncl_types::Value::Integer(modulus)] if *modulus > 0)
            }
            "SIGNED-BYTE" | "UNSIGNED-BYTE" => {
                !args.is_empty()
                    && !matches!(args.as_slice(), [ncl_types::Value::Integer(size)] if *size >= 0)
            }
            _ => true, // check-added-lines: allow(wildcard) unknown deftype names still need expansion
        },
        ncl_types::TypeSpecifier::Or(specifiers)
        | ncl_types::TypeSpecifier::And(specifiers)
        | ncl_types::TypeSpecifier::Values(specifiers) => {
            specifiers.iter().any(contains_unexpanded_deftype)
        }
        ncl_types::TypeSpecifier::Not(specifier) => contains_unexpanded_deftype(specifier),
        ncl_types::TypeSpecifier::Array { element_type, .. }
        | ncl_types::TypeSpecifier::Vector { element_type, .. } => element_type
            .as_deref()
            .is_some_and(contains_unexpanded_deftype),
        ncl_types::TypeSpecifier::Cons { car, cdr } => {
            contains_unexpanded_deftype(car) || contains_unexpanded_deftype(cdr)
        }
        ncl_types::TypeSpecifier::Function {
            lambda_list,
            return_type,
        } => {
            lambda_list.iter().any(contains_unexpanded_deftype)
                || contains_unexpanded_deftype(return_type)
        }
        _ => false, // check-added-lines: allow(wildcard) remaining forms have no nested specifiers
    }
}

pub(super) fn upgraded_array_element_type_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let designator = args.required(0)?;
    let (element_type, _) = element_type_designator(ctx, runtime, designator)?;
    super::helpers::array_element_type_symbol(ctx, runtime, element_type)
}

pub(super) fn make_array_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dimensions = dimension_values(ctx, args.required(0)?)?;
    let mut element_type = ArrayElementType::T;
    let mut modulus = None;
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
                (element_type, modulus) = element_type_designator(ctx, runtime, value)?;
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
    if let Some(modulus) = modulus
        && initial_element_set
        && !mod_element(initial_element, modulus, ctx)?
    {
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
    if let Some(modulus) = modulus {
        for &value in &flattened {
            if !mod_element(value, modulus, ctx)? {
                return Err(ObjectError::TypeError);
            }
        }
    }
    with_rooted_slice(ctx, &flattened, |ctx, rooted| {
        let array = make_array(ctx, runtime, &dimensions, array_options)?;
        for (index, value) in rooted.iter().enumerate() {
            array_row_major_set(ctx, array, index, *value)?;
        }
        Ok(array)
    })
}
