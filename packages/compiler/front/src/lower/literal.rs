//! Lowering quoted and self-evaluating literals to `ncl-ir` constants.
//!
//! The constant table holds scalars, symbols, strings, and descriptors; it has
//! no structural object, so a quoted cons, vector, or non-fixnum number has no
//! representation and is reported as [`LowerError::Unsupported`].

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
        Literal::Cons(_, _) | Literal::Vector(_) => {
            let descriptor = structure_constant(f, literal)?;
            f.word_constant(descriptor)
        }
        // check-added-lines: allow(unsupported) unsupported literal families remain explicit
        Literal::Array { .. } | Literal::BitVector(_) => Err(LowerError::Unsupported {
            form: "quoted structure",
        }),
    }
}

fn structure_constant(f: &mut FunctionLowerer, literal: &Literal) -> Result<Constant, LowerError> {
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
        Literal::Number(NumberLiteral::Fixnum(value)) => Ok(Constant::Fixnum(*value)),
        Literal::Number(NumberLiteral::SingleFloat(value)) => Ok(Constant::SingleFloat(*value)),
        Literal::Number(NumberLiteral::DoubleFloat(value)) => Ok(Constant::DoubleFloat(*value)),
        Literal::Cons(_, _) | Literal::Vector(_) => structure_constant(f, literal),
        Literal::Number(
            NumberLiteral::Bignum { .. }
            | NumberLiteral::Ratio { .. }
            | NumberLiteral::Complex { .. },
        )
        | Literal::Array { .. }
        | Literal::BitVector(_) => Err(LowerError::Unsupported {
            // check-added-lines: allow(unsupported) unsupported literal families remain explicit
            form: "quoted structure",
        }),
    }
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
        | NumberLiteral::Complex { .. } => Err(LowerError::Unsupported {
            form: "quoted number",
        }),
    }
}
