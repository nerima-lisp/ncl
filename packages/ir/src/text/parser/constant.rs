use super::checked::{u8, u32};
use super::{
    ArrayElementType, Constant, ConstantIndex, FunctionId, ParseError, Reader,
    SpecializedArrayElementType, StructureKind,
};

pub(super) fn constant_read(r: &mut Reader<'_>) -> Result<Constant, ParseError> {
    Ok(match r.u()? {
        0 => Constant::Fixnum(r.i()?),
        1 => Constant::Character(u32(r.u()?)?),
        2 => Constant::SingleFloat(f32::from_bits(u32(r.u()?)?)),
        3 => Constant::DoubleFloat(f64::from_bits(r.u()?)),
        4 => Constant::Symbol {
            package: r.s()?,
            name: r.s()?,
        },
        5 => Constant::Object(ConstantIndex(u32(r.u()?)?)),
        16 => Constant::LiteralObject {
            identity: u32(r.u()?)?,
            value: ConstantIndex(u32(r.u()?)?),
        },
        6 => Constant::StringBytes(
            (0..r.u()?)
                .map(|_| Ok(u8(r.u()?)?))
                .collect::<Result<Vec<_>, ParseError>>()?,
        ),
        11 => {
            let kind = match r.u()? {
                0 => StructureKind::Cons,
                1 => StructureKind::SimpleVector,
                2 => {
                    let element_type = match r.u()? {
                        0 => SpecializedArrayElementType::Bit,
                        1 => SpecializedArrayElementType::Character,
                        2 => SpecializedArrayElementType::BaseChar,
                        3 => SpecializedArrayElementType::Fixnum,
                        4 => SpecializedArrayElementType::Signed,
                        5 => SpecializedArrayElementType::Unsigned,
                        6 => SpecializedArrayElementType::SingleFloat,
                        7 => SpecializedArrayElementType::DoubleFloat,
                        /* check-added-lines: allow(wildcard) Unknown encoded specialized element types are invalid. */
                        _ => {
                            return Err(ParseError("bad specialized array element type".into()));
                        }
                    };
                    StructureKind::SpecializedArray { element_type }
                }
                3 => {
                    let element_type = match r.u()? {
                        0 => ArrayElementType::T,
                        1 => ArrayElementType::Bit,
                        2 => ArrayElementType::Character,
                        3 => ArrayElementType::BaseChar,
                        4 => ArrayElementType::Fixnum,
                        5 => ArrayElementType::Signed,
                        6 => ArrayElementType::Unsigned,
                        7 => ArrayElementType::SingleFloat,
                        8 => ArrayElementType::DoubleFloat,
                        _ => return Err(ParseError("bad array element type".into())), // check-added-lines: allow(wildcard) encoded element type is validated explicitly
                    };
                    let dimensions = (0..r.u()?)
                        .map(|_| {
                            usize::try_from(r.u()?)
                                .map_err(|_| ParseError("array dimension out of range".into()))
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    StructureKind::Array {
                        dimensions,
                        element_type,
                    }
                }
                _ => return Err(ParseError("bad structure kind".into())), // check-added-lines: allow(wildcard) encoded descriptor kind is validated explicitly
            };
            Constant::Structure {
                kind,
                elements: (0..r.u()?)
                    .map(|_| Ok(ConstantIndex(u32(r.u()?)?)))
                    .collect::<Result<Vec<_>, ParseError>>()?,
            }
        }
        7 => Constant::Nil,
        8 => Constant::T,
        9 => Constant::Unbound,
        10 => Constant::FunctionEntry(FunctionId(u32(r.u()?)?)),
        12 => Constant::Bignum {
            negative: r.b()?,
            limbs: (0..r.u()?)
                .map(|_| u32(r.u()?))
                .collect::<Result<Vec<_>, ParseError>>()?,
        },
        13 => Constant::Ratio {
            numerator: ConstantIndex(u32(r.u()?)?),
            denominator: ConstantIndex(u32(r.u()?)?),
        },
        14 => Constant::Complex {
            real: ConstantIndex(u32(r.u()?)?),
            imaginary: ConstantIndex(u32(r.u()?)?),
        },
        15 => Constant::UninternedSymbol {
            identity: u32(r.u()?)?,
            name: r.s()?,
        },
        _ => return Err(ParseError("bad constant".into())), // check-added-lines: allow(wildcard) Unknown constant tags are invalid.
    })
}
