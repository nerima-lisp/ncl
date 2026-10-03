//! Lowering quoted and self-evaluating literals to `ncl-ir` constants.
//!
//! The constant table holds scalars, symbols, strings, and descriptors, plus
//! structural entries (cons, vector, array, bignum, ratio, complex) that
//! reference earlier table entries by index.

use ncl_ir::{Constant, Convert, OpKind, StructureKind, Ty, ValueId};

use crate::literal::{Literal, NumberLiteral};

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
        Literal::Number(number) => lower_number(f, number),
        Literal::Cons(_, _)
        | Literal::Vector(_)
        | Literal::Array { .. }
        | Literal::BitVector(_) => {
            let descriptor = structure_constant(f, literal)?;
            f.word_constant(descriptor)
        }
    }
}

fn structure_constant(f: &mut FunctionLowerer, literal: &Literal) -> Result<Constant, LowerError> {
    if matches!(literal, Literal::Array { .. } | Literal::BitVector(_)) {
        return array_constant(f, literal);
    }
    let (kind, children): (StructureKind, Vec<&Literal>) = match literal {
        Literal::Cons(car, cdr) => (StructureKind::Cons, vec![car, cdr]),
        Literal::Vector(elements) => (StructureKind::SimpleVector, elements.iter().collect()),
        Literal::Nil
        | Literal::T
        | Literal::Symbol(_)
        | Literal::Number(_)
        | Literal::Character(_)
        | Literal::String(_)
        | Literal::Array { .. }
        | Literal::BitVector(_) => {
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

fn scalar_or_structure_constant(
    f: &mut FunctionLowerer,
    literal: &Literal,
) -> Result<Constant, LowerError> {
    match literal {
        Literal::Nil => Ok(Constant::Nil),
        Literal::T => Ok(Constant::T),
        Literal::Symbol(symbol) => Ok(Constant::Symbol {
            package: symbol
                .package
                .as_ref()
                .map_or_else(|| "NCL-UNINTERNED".to_owned(), Clone::clone),
            name: symbol.name.clone(),
        }),
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
        Literal::Array { .. } | Literal::BitVector(_) => array_constant(f, literal),
    }
}

fn array_constant(f: &mut FunctionLowerer, literal: &Literal) -> Result<Constant, LowerError> {
    let (dimensions, element_type, values): (Vec<usize>, ncl_ir::ArrayElementType, Vec<Literal>) =
        match literal {
            Literal::Array {
                dimensions,
                element_type,
                elements,
            } => (
                dimensions.clone(),
                array_element_type(*element_type),
                elements.clone(),
            ),
            Literal::BitVector(bits) => (
                vec![bits.len()],
                ncl_ir::ArrayElementType::Bit,
                bits.iter()
                    .map(|bit| Literal::fixnum(i64::from(*bit)))
                    .collect(),
            ),
            Literal::Nil
            | Literal::T
            | Literal::Symbol(..)
            | Literal::Character(..)
            | Literal::String(..)
            | Literal::Number(..)
            | Literal::Cons(..)
            | Literal::Vector(..) => {
                // check-added-lines: allow(unsupported) array-like literal fallback
                return Err(LowerError::Unsupported {
                    form: "quoted structure",
                });
            }
        };
    let elements = values
        .iter()
        .map(|value| {
            let constant = scalar_or_structure_constant(f, value)?;
            Ok(f.add_constant(constant))
        })
        .collect::<Result<Vec<_>, LowerError>>()?;
    Ok(Constant::Array {
        dimensions,
        element_type,
        elements,
    })
}

const fn array_element_type(value: ncl_object::ArrayElementType) -> ncl_ir::ArrayElementType {
    match value {
        ncl_object::ArrayElementType::T => ncl_ir::ArrayElementType::T,
        ncl_object::ArrayElementType::Bit => ncl_ir::ArrayElementType::Bit,
        ncl_object::ArrayElementType::Character => ncl_ir::ArrayElementType::Character,
        ncl_object::ArrayElementType::BaseChar => ncl_ir::ArrayElementType::BaseChar,
        ncl_object::ArrayElementType::Fixnum => ncl_ir::ArrayElementType::Fixnum,
        ncl_object::ArrayElementType::Signed => ncl_ir::ArrayElementType::Signed,
        ncl_object::ArrayElementType::Unsigned => ncl_ir::ArrayElementType::Unsigned,
        ncl_object::ArrayElementType::SingleFloat => ncl_ir::ArrayElementType::SingleFloat,
        ncl_object::ArrayElementType::DoubleFloat => ncl_ir::ArrayElementType::DoubleFloat,
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
