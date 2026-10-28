//! Lowering quoted and self-evaluating literals to `ncl-ir` constants.
//!
//! The constant table holds scalars, symbols, strings, and descriptors, plus
//! structural entries (cons, vector, array, bignum, ratio, complex) that
//! reference earlier table entries by index.

use ncl_ir::{
    ArrayElementType as IrArrayElementType, Constant, ConstantIndex, Convert, OpKind,
    SpecializedArrayElementType, StructureKind, Ty, ValueId,
};
use ncl_object::ArrayElementType;

use crate::literal::{Literal, NumberLiteral};
use crate::symbols::SymbolRef;

use super::error::LowerError;
use super::function::FunctionLowerer;

/// Lower a literal to a single `Word` value.
pub(super) fn lower_literal(
    f: &mut FunctionLowerer,
    literal: &Literal,
) -> Result<ValueId, LowerError> {
    match literal {
        Literal::Nil => f.word_constant(Constant::Nil),
        Literal::T => f.word_constant(Constant::T),
        Literal::Symbol(symbol) => f.symbol(symbol),
        Literal::Character(character) => f.word_constant(Constant::Character(*character)),
        Literal::String(characters) => {
            let mut bytes = Vec::new();
            for character in characters {
                let mut buffer = [0_u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
            f.word_constant(Constant::StringBytes(bytes))
        }
        Literal::Pathname(namestring) => lower_pathname(f, namestring),
        Literal::Number(number) => lower_number(f, number),
        Literal::Cons(_, _) | Literal::Vector(_) => {
            let descriptor = structure_constant(f, literal)?;
            f.word_constant(descriptor)
        }
        Literal::Array { identity, .. } | Literal::SpecializedArray { identity, .. } => {
            let index = array_constant_index(f, literal, *identity)?;
            f.word_constant(Constant::Object(index))
        }
    }
}

fn lower_pathname(f: &mut FunctionLowerer, namestring: &[char]) -> Result<ValueId, LowerError> {
    let mut bytes = Vec::new();
    for character in namestring {
        let mut buffer = [0_u8; 4];
        bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    }
    let namestring = f.word_constant(Constant::StringBytes(bytes))?;
    let symbol = f.symbol(&SymbolRef::interned("COMMON-LISP", "PATHNAME"))?;
    let function = f.one(
        OpKind::LoadField {
            object: symbol,
            field: u32::try_from(ncl_object::symbol_offset::FUNCTION).map_err(|_| {
                LowerError::Ir {
                    detail: "function symbol offset does not fit u32".to_owned(),
                }
            })?,
        },
        Ty::Word,
    )?;
    let raw_argc = f.fixnum(1)?;
    let argc = f.one(
        OpKind::Convert {
            op: Convert::I64ToWord,
            value: raw_argc,
        },
        Ty::Word,
    )?;
    f.safepoint()?;
    f.one(
        OpKind::CallClosure {
            closure: function,
            args: vec![argc, namestring],
            named_symbol: Some(symbol),
        },
        Ty::Word,
    )
}

fn structure_constant(f: &mut FunctionLowerer, literal: &Literal) -> Result<Constant, LowerError> {
    let (kind, children): (StructureKind, Vec<&Literal>) = match literal {
        Literal::Cons(car, cdr) => (StructureKind::Cons, vec![car, cdr]),
        Literal::Vector(elements) => (StructureKind::SimpleVector, elements.iter().collect()),
        Literal::SpecializedArray {
            element_type,
            elements,
            ..
        } => (
            StructureKind::SpecializedArray {
                element_type: ir_specialized_array_element_type(*element_type)?,
            },
            elements.iter().collect(),
        ),
        Literal::Array {
            dimensions,
            element_type,
            elements,
            ..
        } => (
            StructureKind::Array {
                dimensions: dimensions.clone(),
                element_type: ir_array_element_type(*element_type),
            },
            elements.iter().collect(),
        ),
        Literal::Nil
        | Literal::T
        | Literal::Symbol(_)
        | Literal::Number(_)
        | Literal::Character(_)
        | Literal::String(_)
        | Literal::Pathname(_) => {
            // check-added-lines: allow(unsupported) unsupported literal families remain explicit
            return Err(LowerError::Unsupported {
                form: "quoted structure",
            });
        }
    };
    let elements = children
        .into_iter()
        .map(|child| {
            let constant = scalar_or_structure_constant(f, child)?;
            Ok(f.add_constant(constant))
        })
        .collect::<Result<Vec<_>, LowerError>>()?;
    Ok(Constant::Structure { kind, elements })
}

fn array_constant_index(
    f: &mut FunctionLowerer,
    literal: &Literal,
    identity: u32,
) -> Result<ConstantIndex, LowerError> {
    if let Some(index) = f.literal_constant(identity) {
        return Ok(index);
    }
    let descriptor = structure_constant(f, literal)?;
    let descriptor = f.add_constant(descriptor);
    let index = f.add_constant(Constant::LiteralObject {
        identity,
        value: descriptor,
    });
    f.remember_literal_constant(identity, index);
    Ok(index)
}

const fn ir_specialized_array_element_type(
    element_type: ArrayElementType,
) -> Result<SpecializedArrayElementType, LowerError> {
    match element_type {
        ArrayElementType::Bit => Ok(SpecializedArrayElementType::Bit),
        ArrayElementType::Character => Ok(SpecializedArrayElementType::Character),
        ArrayElementType::BaseChar => Ok(SpecializedArrayElementType::BaseChar),
        ArrayElementType::Fixnum => Ok(SpecializedArrayElementType::Fixnum),
        ArrayElementType::Signed => Ok(SpecializedArrayElementType::Signed),
        ArrayElementType::Unsigned => Ok(SpecializedArrayElementType::Unsigned),
        ArrayElementType::SingleFloat => Ok(SpecializedArrayElementType::SingleFloat),
        ArrayElementType::DoubleFloat => Ok(SpecializedArrayElementType::DoubleFloat),
        // check-added-lines: allow(unsupported) T is a general-array element type, not a specialized-array element type.
        ArrayElementType::T => Err(LowerError::Unsupported {
            form: "specialized array with element type T",
        }),
    }
}

const fn ir_array_element_type(element_type: ArrayElementType) -> IrArrayElementType {
    match element_type {
        ArrayElementType::T => IrArrayElementType::T,
        ArrayElementType::Bit => IrArrayElementType::Bit,
        ArrayElementType::Character => IrArrayElementType::Character,
        ArrayElementType::BaseChar => IrArrayElementType::BaseChar,
        ArrayElementType::Fixnum => IrArrayElementType::Fixnum,
        ArrayElementType::Signed => IrArrayElementType::Signed,
        ArrayElementType::Unsigned => IrArrayElementType::Unsigned,
        ArrayElementType::SingleFloat => IrArrayElementType::SingleFloat,
        ArrayElementType::DoubleFloat => IrArrayElementType::DoubleFloat,
    }
}

fn scalar_or_structure_constant(
    f: &mut FunctionLowerer,
    literal: &Literal,
) -> Result<Constant, LowerError> {
    match literal {
        Literal::Nil => Ok(Constant::Nil),
        Literal::T => Ok(Constant::T),
        Literal::Symbol(symbol) => super::symbol_constant(symbol),
        Literal::Character(character) => Ok(Constant::Character(*character)),
        Literal::String(characters) => {
            let mut bytes = Vec::new();
            for character in characters {
                let mut buffer = [0_u8; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
            Ok(Constant::StringBytes(bytes))
        }
        Literal::Number(number) => number_literal_constant(f, number),
        Literal::Cons(_, _) | Literal::Vector(_) => structure_constant(f, literal),
        Literal::Array { identity, .. } | Literal::SpecializedArray { identity, .. } => {
            let index = array_constant_index(f, literal, *identity)?;
            Ok(Constant::Object(index))
        }
        Literal::Pathname(_) => {
            // check-added-lines: allow(unsupported) unsupported literal families remain explicit
            Err(LowerError::Unsupported {
                form: "quoted structure",
            })
        }
    }
}

/// Lower a numeric literal to a constant-table entry, recursively lowering
/// the numerator/denominator or real/imaginary parts of ratios and
/// complexes into their own earlier entries (mirroring how
/// [`structure_constant`] lowers cons/vector elements).
fn number_literal_constant(
    f: &mut FunctionLowerer,
    number: &NumberLiteral,
) -> Result<Constant, LowerError> {
    Ok(match number {
        NumberLiteral::Fixnum(value) => Constant::Fixnum(*value),
        NumberLiteral::SingleFloat(value) => Constant::SingleFloat(*value),
        NumberLiteral::DoubleFloat(value) => Constant::DoubleFloat(*value),
        NumberLiteral::Bignum { negative, limbs } => Constant::Bignum {
            negative: *negative,
            limbs: limbs.clone(),
        },
        NumberLiteral::Ratio {
            numerator,
            denominator,
        } => {
            let numerator = number_literal_constant(f, numerator)?;
            let numerator = f.add_constant(numerator);
            let denominator = number_literal_constant(f, denominator)?;
            let denominator = f.add_constant(denominator);
            Constant::Ratio {
                numerator,
                denominator,
            }
        }
        NumberLiteral::Complex { real, imaginary } => {
            let real = number_literal_constant(f, real)?;
            let real = f.add_constant(real);
            let imaginary = number_literal_constant(f, imaginary)?;
            let imaginary = f.add_constant(imaginary);
            Constant::Complex { real, imaginary }
        }
    })
}

fn lower_number(f: &mut FunctionLowerer, number: &NumberLiteral) -> Result<ValueId, LowerError> {
    match number {
        NumberLiteral::Fixnum(value) => {
            let raw = f.fixnum(*value)?;
            f.one(
                OpKind::Convert {
                    op: Convert::I64ToWord,
                    value: raw,
                },
                Ty::Word,
            )
        }
        NumberLiteral::SingleFloat(value) => {
            let raw = f.constant(Constant::SingleFloat(*value))?;
            f.one(
                OpKind::Convert {
                    op: Convert::F64ToWord,
                    value: raw,
                },
                Ty::Word,
            )
        }
        NumberLiteral::DoubleFloat(value) => {
            let raw = f.constant(Constant::DoubleFloat(*value))?;
            f.one(
                OpKind::Convert {
                    op: Convert::F64ToWord,
                    value: raw,
                },
                Ty::Word,
            )
        }
        NumberLiteral::Bignum { .. }
        | NumberLiteral::Ratio { .. }
        | NumberLiteral::Complex { .. } => {
            let constant = number_literal_constant(f, number)?;
            f.word_constant(constant)
        }
    }
}
