use ncl_ir::{
    ArrayElementType, Constant, ConstantIndex, SpecializedArrayElementType, StructureKind, Ty,
};

use crate::ids::NodeId;
use crate::node::{Literal, LiteralElementType, NumberLiteral};

use super::{FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn literal(
        &mut self,
        node: NodeId,
        literal: &Literal,
    ) -> Result<ncl_ir::ValueId, LowerError> {
        let constant = self.literal_value(node, literal)?;
        let ty = literal_type(&constant);
        let value = self.push_constant(constant, ty)?;
        self.to_word(value, ty)
    }

    fn literal_value(&mut self, node: NodeId, literal: &Literal) -> Result<Constant, LowerError> {
        Ok(match literal {
            Literal::Nil => Constant::Nil,
            Literal::True => Constant::T,
            Literal::Symbol(symbol) => self.symbol_constant(*symbol)?,
            Literal::Number(number) => self.number_constant(node, number)?,
            Literal::Character(character) => Constant::Character(*character),
            Literal::String(characters) => Constant::StringBytes(encode_utf8(characters)),
            Literal::Pathname(_) => return self.unsupported(node, "pathname-literal"),
            Literal::Cons(_, _) | Literal::Vector(_) => self.structure_constant(node, literal)?,
            Literal::Array { identity, .. } | Literal::SpecializedArray { identity, .. } => {
                Constant::Object(self.array_constant(node, literal, *identity)?)
            }
        })
    }

    fn number_constant(
        &mut self,
        node: NodeId,
        number: &NumberLiteral,
    ) -> Result<Constant, LowerError> {
        Ok(match number {
            NumberLiteral::Fixnum(value) => Constant::Fixnum(*value),
            NumberLiteral::Bignum { negative, limbs } => Constant::Bignum {
                negative: *negative,
                limbs: limbs.clone(),
            },
            NumberLiteral::SingleFloat(bits) => Constant::SingleFloat(f32::from_bits(*bits)),
            NumberLiteral::DoubleFloat(bits) => Constant::DoubleFloat(f64::from_bits(*bits)),
            NumberLiteral::Ratio {
                numerator,
                denominator,
            } => {
                let numerator = self.number_constant(node, numerator)?;
                let numerator = self.add_constant(numerator)?;
                let denominator = self.number_constant(node, denominator)?;
                let denominator = self.add_constant(denominator)?;
                Constant::Ratio {
                    numerator,
                    denominator,
                }
            }
            NumberLiteral::Complex { real, imaginary } => {
                let real = self.number_constant(node, real)?;
                let real = self.add_constant(real)?;
                let imaginary = self.number_constant(node, imaginary)?;
                let imaginary = self.add_constant(imaginary)?;
                Constant::Complex { real, imaginary }
            }
        })
    }

    fn structure_constant(
        &mut self,
        node: NodeId,
        literal: &Literal,
    ) -> Result<Constant, LowerError> {
        let (kind, children) = match literal {
            Literal::Cons(car, cdr) => (StructureKind::Cons, vec![car.as_ref(), cdr.as_ref()]),
            Literal::Vector(elements) => (StructureKind::SimpleVector, elements.iter().collect()),
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
            Literal::SpecializedArray {
                element_type,
                elements,
                ..
            } => (
                StructureKind::SpecializedArray {
                    element_type: ir_specialized_array_element_type(node, *element_type)?,
                },
                elements.iter().collect(),
            ),
            _ => return self.unsupported(node, "quoted-structure"),
        };
        let elements = children
            .into_iter()
            .map(|child| {
                let constant = self.literal_value(node, child)?;
                self.add_constant(constant)
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        Ok(Constant::Structure { kind, elements })
    }

    fn array_constant(
        &mut self,
        node: NodeId,
        literal: &Literal,
        identity: u32,
    ) -> Result<ConstantIndex, LowerError> {
        if let Some(index) = self.literal_objects.get(&identity).copied() {
            return Ok(index);
        }
        let descriptor = self.structure_constant(node, literal)?;
        let descriptor = self.add_constant(descriptor)?;
        let index = self.add_constant(Constant::LiteralObject {
            identity,
            value: descriptor,
        })?;
        let _ = self.literal_objects.insert(identity, index);
        Ok(index)
    }
}

const fn literal_type(constant: &Constant) -> Ty {
    match constant {
        Constant::Fixnum(_) => Ty::I64,
        Constant::SingleFloat(_) | Constant::DoubleFloat(_) => Ty::F64,
        Constant::Character(_)
        | Constant::Symbol { .. }
        | Constant::UninternedSymbol { .. }
        | Constant::Object(_)
        | Constant::LiteralObject { .. }
        | Constant::StringBytes(_)
        | Constant::Structure { .. }
        | Constant::Nil
        | Constant::T
        | Constant::Unbound
        | Constant::FunctionEntry(_)
        | Constant::Bignum { .. }
        | Constant::Ratio { .. }
        | Constant::Complex { .. } => Ty::Word,
    }
}

fn encode_utf8(characters: &[char]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for character in characters {
        let mut buffer = [0_u8; 4];
        bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    }
    bytes
}

const fn ir_array_element_type(element_type: LiteralElementType) -> ArrayElementType {
    match element_type {
        LiteralElementType::T => ArrayElementType::T,
        LiteralElementType::Bit => ArrayElementType::Bit,
        LiteralElementType::Character => ArrayElementType::Character,
        LiteralElementType::BaseChar => ArrayElementType::BaseChar,
        LiteralElementType::Fixnum => ArrayElementType::Fixnum,
        LiteralElementType::Signed => ArrayElementType::Signed,
        LiteralElementType::Unsigned => ArrayElementType::Unsigned,
        LiteralElementType::SingleFloat => ArrayElementType::SingleFloat,
        LiteralElementType::DoubleFloat => ArrayElementType::DoubleFloat,
    }
}

const fn ir_specialized_array_element_type(
    node: NodeId,
    element_type: LiteralElementType,
) -> Result<SpecializedArrayElementType, LowerError> {
    match element_type {
        LiteralElementType::Bit => Ok(SpecializedArrayElementType::Bit),
        LiteralElementType::Character => Ok(SpecializedArrayElementType::Character),
        LiteralElementType::BaseChar => Ok(SpecializedArrayElementType::BaseChar),
        LiteralElementType::Fixnum => Ok(SpecializedArrayElementType::Fixnum),
        LiteralElementType::Signed => Ok(SpecializedArrayElementType::Signed),
        LiteralElementType::Unsigned => Ok(SpecializedArrayElementType::Unsigned),
        LiteralElementType::SingleFloat => Ok(SpecializedArrayElementType::SingleFloat),
        LiteralElementType::DoubleFloat => Ok(SpecializedArrayElementType::DoubleFloat),
        LiteralElementType::T => Err(LowerError::Unsupported {
            node,
            operation: "specialized-array-with-general-element-type",
        }),
    }
}
