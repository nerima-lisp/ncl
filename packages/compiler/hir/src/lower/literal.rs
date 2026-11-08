use ncl_ir::{
    ArrayElementType, Constant, ConstantIndex, Convert, SpecializedArrayElementType, StructureKind,
    Ty,
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
        let (ty, conversion) = literal_type(&constant);
        let value = self.push_constant(constant, ty)?;
        self.coerce_to_word(value, conversion)
    }

    fn literal_value(&mut self, node: NodeId, literal: &Literal) -> Result<Constant, LowerError> {
        Ok(match literal {
            Literal::Nil => Constant::Nil,
            Literal::True => Constant::T,
            Literal::Symbol(symbol) => self.symbol_constant(*symbol)?,
            Literal::Number(number) => self.number_constant(number)?,
            Literal::Character(character) => Constant::Character(*character),
            Literal::String(characters) => Constant::StringBytes(encode_utf8(characters)),
            Literal::Pathname(_) => return Self::unsupported(node, "pathname-literal"),
            Literal::Cons(car, cdr) => self.structure_constant(
                node,
                StructureKind::Cons,
                vec![car.as_ref(), cdr.as_ref()],
            )?,
            Literal::Vector(elements) => self.structure_constant(
                node,
                StructureKind::SimpleVector,
                elements.iter().collect(),
            )?,
            Literal::Array {
                identity,
                dimensions,
                element_type,
                elements,
            } => Constant::Object(self.array_constant(
                node,
                *identity,
                StructureKind::Array {
                    dimensions: dimensions.clone(),
                    element_type: ir_array_element_type(*element_type),
                },
                elements.iter().collect(),
            )?),
            Literal::SpecializedArray {
                identity,
                element_type: LiteralElementType::T,
                elements,
            } => Constant::Object(self.array_constant(
                node,
                *identity,
                StructureKind::Array {
                    dimensions: vec![elements.len()],
                    element_type: ArrayElementType::T,
                },
                elements.iter().collect(),
            )?),
            Literal::SpecializedArray {
                identity,
                element_type,
                elements,
            } => Constant::Object(
                self.array_constant(
                    node,
                    *identity,
                    StructureKind::SpecializedArray {
                        element_type: ir_specialized_array_element_type(*element_type)
                            .ok_or(LowerError::InvalidSpecializedArray(node))?,
                    },
                    elements.iter().collect(),
                )?,
            ),
        })
    }

    fn number_constant(&mut self, number: &NumberLiteral) -> Result<Constant, LowerError> {
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
                let numerator = self.number_constant(numerator)?;
                let numerator = self.add_constant(numerator)?;
                let denominator = self.number_constant(denominator)?;
                let denominator = self.add_constant(denominator)?;
                Constant::Ratio {
                    numerator,
                    denominator,
                }
            }
            NumberLiteral::Complex { real, imaginary } => {
                let real = self.number_constant(real)?;
                let real = self.add_constant(real)?;
                let imaginary = self.number_constant(imaginary)?;
                let imaginary = self.add_constant(imaginary)?;
                Constant::Complex { real, imaginary }
            }
        })
    }

    fn structure_constant(
        &mut self,
        node: NodeId,
        kind: StructureKind,
        children: Vec<&Literal>,
    ) -> Result<Constant, LowerError> {
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
        identity: u32,
        kind: StructureKind,
        children: Vec<&Literal>,
    ) -> Result<ConstantIndex, LowerError> {
        if let Some(index) = self.literal_objects.get(&identity).copied() {
            return Ok(index);
        }
        let descriptor = self.structure_constant(node, kind, children)?;
        let descriptor = self.add_constant(descriptor)?;
        let index = self.add_constant(Constant::LiteralObject {
            identity,
            value: descriptor,
        })?;
        let _ = self.literal_objects.insert(identity, index);
        Ok(index)
    }
}

const fn literal_type(constant: &Constant) -> (Ty, Option<Convert>) {
    match constant {
        Constant::Fixnum(_) => (Ty::I64, Some(Convert::I64ToWord)),
        Constant::SingleFloat(_) | Constant::DoubleFloat(_) => (Ty::F64, Some(Convert::F64ToWord)),
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
        | Constant::Complex { .. } => (Ty::Word, None),
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
    element_type: LiteralElementType,
) -> Option<SpecializedArrayElementType> {
    match element_type {
        LiteralElementType::Bit => Some(SpecializedArrayElementType::Bit),
        LiteralElementType::Character => Some(SpecializedArrayElementType::Character),
        LiteralElementType::BaseChar => Some(SpecializedArrayElementType::BaseChar),
        LiteralElementType::Fixnum => Some(SpecializedArrayElementType::Fixnum),
        LiteralElementType::Signed => Some(SpecializedArrayElementType::Signed),
        LiteralElementType::Unsigned => Some(SpecializedArrayElementType::Unsigned),
        LiteralElementType::SingleFloat => Some(SpecializedArrayElementType::SingleFloat),
        LiteralElementType::DoubleFloat => Some(SpecializedArrayElementType::DoubleFloat),
        LiteralElementType::T => None,
    }
}
