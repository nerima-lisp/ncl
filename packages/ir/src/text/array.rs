use crate::{ArrayElementType, Constant, ConstantIndex};

use super::{ParseError, parser::Reader};

pub(super) fn read(r: &mut Reader<'_>) -> Result<Constant, ParseError> {
    let dimensions = (0..r.u()?)
        .map(|_| usize::try_from(r.u()?).map_err(|_| ParseError("array dimension overflow".into())))
        .collect::<Result<Vec<_>, _>>()?;
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
        _ => return Err(ParseError("bad array element type".into())),
    };
    Ok(Constant::Array {
        dimensions,
        element_type,
        elements: (0..r.u()?)
            .map(|_| Ok(ConstantIndex(super::parser::u32(r.u()?)?)))
            .collect::<Result<Vec<_>, ParseError>>()?,
    })
}
