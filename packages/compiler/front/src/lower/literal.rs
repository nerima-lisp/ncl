//! Lowering quoted and self-evaluating literals to `ncl-ir` constants.
//!
//! The constant table holds scalars, symbols, strings, and descriptors; it has
//! no structural object, so a quoted cons, vector, or non-fixnum number has no
//! representation and is reported as [`LowerError::Unsupported`].

use ncl_ir::{Constant, Convert, OpKind, Ty, ValueId};

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
        | Literal::BitVector(_) => Err(LowerError::Unsupported {
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
