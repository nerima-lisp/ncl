use ncl_hir::{Literal as HirLiteral, LiteralElementType, NumberLiteral as HirNumber};

use crate::literal::{Literal, NumberLiteral};

use super::HirBuildError;
use super::context::BuildContext;

impl BuildContext<'_> {
    pub(super) fn literal(&mut self, literal: &Literal) -> Result<HirLiteral, HirBuildError> {
        Ok(match literal {
            Literal::Nil => HirLiteral::Nil,
            Literal::T => HirLiteral::True,
            Literal::Symbol(symbol) if symbol.is_common_lisp("NIL") => HirLiteral::Nil,
            Literal::Symbol(symbol) if symbol.is_common_lisp("T") => HirLiteral::True,
            Literal::Symbol(symbol) => HirLiteral::Symbol(self.intern(symbol)?),
            Literal::Number(number) => HirLiteral::Number(match number {
                NumberLiteral::Fixnum(value) => HirNumber::Fixnum(*value),
                NumberLiteral::Bignum { negative, limbs } => HirNumber::Bignum {
                    negative: *negative,
                    limbs: limbs.clone(),
                },
                NumberLiteral::Ratio {
                    numerator,
                    denominator,
                } => HirNumber::Ratio {
                    numerator: Box::new(number_literal(numerator)),
                    denominator: Box::new(number_literal(denominator)),
                },
                NumberLiteral::SingleFloat(value) => HirNumber::SingleFloat(value.to_bits()),
                NumberLiteral::DoubleFloat(value) => HirNumber::DoubleFloat(value.to_bits()),
                NumberLiteral::Complex { real, imaginary } => HirNumber::Complex {
                    real: Box::new(number_literal(real)),
                    imaginary: Box::new(number_literal(imaginary)),
                },
            }),
            Literal::Character(value) => HirLiteral::Character(*value),
            Literal::String(value) => HirLiteral::String(value.clone()),
            Literal::Pathname(value) => HirLiteral::Pathname(value.clone()),
            Literal::Cons(car, cdr) => {
                HirLiteral::Cons(Box::new(self.literal(car)?), Box::new(self.literal(cdr)?))
            }
            Literal::Vector(items) => HirLiteral::Vector(
                items
                    .iter()
                    .map(|item| self.literal(item))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Literal::Array {
                identity,
                dimensions,
                element_type,
                elements,
            } => HirLiteral::Array {
                identity: *identity,
                dimensions: dimensions.clone(),
                element_type: map_element_type(*element_type),
                elements: elements
                    .iter()
                    .map(|item| self.literal(item))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            Literal::SpecializedArray {
                identity,
                element_type,
                elements,
            } => HirLiteral::SpecializedArray {
                identity: *identity,
                element_type: map_element_type(*element_type),
                elements: elements
                    .iter()
                    .map(|item| self.literal(item))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        })
    }
}

fn number_literal(number: &NumberLiteral) -> HirNumber {
    match number {
        NumberLiteral::Fixnum(value) => HirNumber::Fixnum(*value),
        NumberLiteral::Bignum { negative, limbs } => HirNumber::Bignum {
            negative: *negative,
            limbs: limbs.clone(),
        },
        NumberLiteral::Ratio {
            numerator,
            denominator,
        } => HirNumber::Ratio {
            numerator: Box::new(number_literal(numerator)),
            denominator: Box::new(number_literal(denominator)),
        },
        NumberLiteral::SingleFloat(value) => HirNumber::SingleFloat(value.to_bits()),
        NumberLiteral::DoubleFloat(value) => HirNumber::DoubleFloat(value.to_bits()),
        NumberLiteral::Complex { real, imaginary } => HirNumber::Complex {
            real: Box::new(number_literal(real)),
            imaginary: Box::new(number_literal(imaginary)),
        },
    }
}

const fn map_element_type(element_type: ncl_object::ArrayElementType) -> LiteralElementType {
    match element_type {
        ncl_object::ArrayElementType::T => LiteralElementType::T,
        ncl_object::ArrayElementType::Bit => LiteralElementType::Bit,
        ncl_object::ArrayElementType::Character => LiteralElementType::Character,
        ncl_object::ArrayElementType::BaseChar => LiteralElementType::BaseChar,
        ncl_object::ArrayElementType::Fixnum => LiteralElementType::Fixnum,
        ncl_object::ArrayElementType::Signed => LiteralElementType::Signed,
        ncl_object::ArrayElementType::Unsigned => LiteralElementType::Unsigned,
        ncl_object::ArrayElementType::SingleFloat => LiteralElementType::SingleFloat,
        ncl_object::ArrayElementType::DoubleFloat => LiteralElementType::DoubleFloat,
    }
}
