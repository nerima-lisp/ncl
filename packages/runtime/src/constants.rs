use std::collections::HashMap;

use ncl_object::{
    ArrayElementType, ArrayOptions, HeapRootedWord, ObjectError, Package, Runtime as ObjectRuntime,
    ThreadContext, Word, array_row_major_set, make_array, make_bignum_from_limbs, make_complex,
    make_cons, make_double, make_ratio, make_simple_vector, make_specialized_array, make_string,
};

use crate::RuntimeError;

pub fn resolve_constant(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    constant: &ncl_ir::Constant,
    previous: &[Word],
    uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
    literal_objects: &mut HashMap<u32, HeapRootedWord>,
) -> Result<Word, RuntimeError> {
    ncl_object::with_roots(ctx, previous, |ctx, previous| {
        let resolved = |index: &ncl_ir::ConstantIndex| -> Result<Word, ObjectError> {
            previous
                .get(usize::try_from(index.0).map_err(|_| ObjectError::Layout)?)
                .map(|value| **value)
                .ok_or(ObjectError::Layout)
        };
        resolve_constant_value(
            ctx,
            runtime,
            constant,
            &resolved,
            uninterned_symbols,
            literal_objects,
        )
    })
    .map_err(Into::into)
}

fn resolve_constant_value(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    constant: &ncl_ir::Constant,
    resolved: &impl Fn(&ncl_ir::ConstantIndex) -> Result<Word, ObjectError>,
    uninterned_symbols: &mut HashMap<u32, HeapRootedWord>,
    literal_objects: &mut HashMap<u32, HeapRootedWord>,
) -> Result<Word, ObjectError> {
    let value = match constant {
        ncl_ir::Constant::Fixnum(value) => Word::fixnum(*value),
        ncl_ir::Constant::Character(value) => Word::character(*value),
        // `FunctionEntry` is resolved by `Runtime::make_constants` before it
        // ever reaches this helper; NIL is a harmless fallback here.
        ncl_ir::Constant::Nil | ncl_ir::Constant::FunctionEntry(_) => Word::NIL,
        ncl_ir::Constant::T => Word::TRUE,
        // check-added-lines: allow(unbound) IR explicitly represents this sentinel.
        ncl_ir::Constant::Unbound => Word::UNBOUND,
        ncl_ir::Constant::Object(index) => resolved(index)?,
        ncl_ir::Constant::LiteralObject { identity, value } => {
            if let Some(object) = literal_objects.get(identity) {
                object.get()
            } else {
                let object = resolved(value)?;
                literal_objects.insert(*identity, HeapRootedWord::new(ctx, object)?);
                object
            }
        }
        ncl_ir::Constant::StringBytes(bytes) => {
            let text = std::str::from_utf8(bytes).map_err(|_| ObjectError::Layout)?;
            make_string(ctx, runtime, &text.chars().collect::<Vec<_>>())?
        }
        ncl_ir::Constant::Structure { kind, elements } => {
            resolve_structure_constant(ctx, runtime, kind, elements, resolved)?
        }
        ncl_ir::Constant::DoubleFloat(value) => make_double(ctx, runtime, *value)?.as_word(),
        ncl_ir::Constant::SingleFloat(value) => {
            make_double(ctx, runtime, f64::from(*value))?.as_word()
        }
        ncl_ir::Constant::Symbol { package, name } => {
            let package_word = runtime
                .find_package(ctx, package)
                .ok_or(ObjectError::Layout)?;
            Package::from_word(package_word)
                .intern(ctx, runtime, name)
                .map(|(symbol, _)| symbol)?
        }
        ncl_ir::Constant::UninternedSymbol { identity, name } => {
            crate::uninterned_symbols::resolve(ctx, runtime, *identity, name, uninterned_symbols)?
        }
        ncl_ir::Constant::Bignum { negative, limbs } => {
            make_bignum_from_limbs(ctx, runtime, *negative, limbs)?.as_word()
        }
        ncl_ir::Constant::Ratio {
            numerator,
            denominator,
        } => make_ratio(ctx, runtime, resolved(numerator)?, resolved(denominator)?)?.as_word(),
        ncl_ir::Constant::Complex { real, imaginary } => {
            make_complex(ctx, runtime, resolved(real)?, resolved(imaginary)?)?.as_word()
        }
    };
    Ok(value)
}

fn resolve_structure_constant(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    kind: &ncl_ir::StructureKind,
    elements: &[ncl_ir::ConstantIndex],
    resolved: &impl Fn(&ncl_ir::ConstantIndex) -> Result<Word, ObjectError>,
) -> Result<Word, ObjectError> {
    let values = elements
        .iter()
        .map(resolved)
        .collect::<Result<Vec<_>, ObjectError>>()?;
    match kind {
        ncl_ir::StructureKind::Cons => {
            let [car, cdr] = values.as_slice() else {
                return Err(ObjectError::Layout);
            };
            make_cons(ctx, runtime, *car, *cdr)
        }
        ncl_ir::StructureKind::SimpleVector => make_simple_vector(ctx, runtime, &values),
        ncl_ir::StructureKind::SpecializedArray { element_type } => {
            let element_type = runtime_specialized_array_element_type(*element_type);
            make_specialized_array(ctx, runtime, element_type, &values)
        }
        ncl_ir::StructureKind::Array {
            dimensions,
            element_type,
        } => resolve_general_array(ctx, runtime, dimensions, *element_type, &values),
    }
}

const fn runtime_specialized_array_element_type(
    element_type: ncl_ir::SpecializedArrayElementType,
) -> ArrayElementType {
    match element_type {
        ncl_ir::SpecializedArrayElementType::Bit => ArrayElementType::Bit,
        ncl_ir::SpecializedArrayElementType::Character => ArrayElementType::Character,
        ncl_ir::SpecializedArrayElementType::BaseChar => ArrayElementType::BaseChar,
        ncl_ir::SpecializedArrayElementType::Fixnum => ArrayElementType::Fixnum,
        ncl_ir::SpecializedArrayElementType::Signed => ArrayElementType::Signed,
        ncl_ir::SpecializedArrayElementType::Unsigned => ArrayElementType::Unsigned,
        ncl_ir::SpecializedArrayElementType::SingleFloat => ArrayElementType::SingleFloat,
        ncl_ir::SpecializedArrayElementType::DoubleFloat => ArrayElementType::DoubleFloat,
    }
}

const fn runtime_array_element_type(element_type: ncl_ir::ArrayElementType) -> ArrayElementType {
    match element_type {
        ncl_ir::ArrayElementType::T => ArrayElementType::T,
        ncl_ir::ArrayElementType::Bit => ArrayElementType::Bit,
        ncl_ir::ArrayElementType::Character => ArrayElementType::Character,
        ncl_ir::ArrayElementType::BaseChar => ArrayElementType::BaseChar,
        ncl_ir::ArrayElementType::Fixnum => ArrayElementType::Fixnum,
        ncl_ir::ArrayElementType::Signed => ArrayElementType::Signed,
        ncl_ir::ArrayElementType::Unsigned => ArrayElementType::Unsigned,
        ncl_ir::ArrayElementType::SingleFloat => ArrayElementType::SingleFloat,
        ncl_ir::ArrayElementType::DoubleFloat => ArrayElementType::DoubleFloat,
    }
}

fn resolve_general_array(
    ctx: &mut ThreadContext,
    runtime: &ObjectRuntime,
    dimensions: &[usize],
    element_type: ncl_ir::ArrayElementType,
    values: &[Word],
) -> Result<Word, ObjectError> {
    let element_type = runtime_array_element_type(element_type);
    let total = dimensions
        .iter()
        .try_fold(1_usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(ObjectError::Layout)?;
    if total != values.len() {
        return Err(ObjectError::Layout);
    }
    ncl_object::with_roots(ctx, values, |ctx, rooted_values| {
        let initial_element = rooted_values
            .first()
            .map_or_else(|| default_array_element(element_type), |value| **value);
        let array = make_array(
            ctx,
            runtime,
            dimensions,
            ArrayOptions {
                element_type,
                initial_element,
                adjustable: false,
                fill_pointer: None,
                displaced_to: None,
                displaced_index_offset: 0,
            },
        )?;
        for (index, value) in rooted_values.iter().copied().enumerate() {
            array_row_major_set(ctx, array, index, *value)?;
        }
        Ok(array)
    })
}

const fn default_array_element(element_type: ArrayElementType) -> Word {
    match element_type {
        ArrayElementType::Bit
        | ArrayElementType::Fixnum
        | ArrayElementType::Signed
        | ArrayElementType::Unsigned => Word::fixnum(0),
        ArrayElementType::Character | ArrayElementType::BaseChar => Word::character(0),
        ArrayElementType::T | ArrayElementType::SingleFloat | ArrayElementType::DoubleFloat => {
            Word::NIL
        }
    }
}
