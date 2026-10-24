use super::{ParseError, Reader};
use crate::ScalarTypeHint;

pub(super) fn u32(value: u64) -> Result<u32, ParseError> {
    u32::try_from(value).map_err(|_| ParseError("integer out of range".into()))
}

pub(super) fn u8(value: u64) -> Result<u8, ParseError> {
    u8::try_from(value).map_err(|_| ParseError("integer out of range".into()))
}

pub(super) fn text_version(header: &str) -> Result<u8, ParseError> {
    match header.rsplit_once(' ') {
        Some((name, "ir3")) if !name.is_empty() => Ok(3),
        Some((name, "ir2")) if !name.is_empty() => Ok(2),
        Some(_) => return Err(ParseError("unsupported IR text version".into())),
        None if !header.is_empty() => Ok(1),
        None => return Err(ParseError("expected fn header".into())),
    }
}

pub(super) fn fixnum_specialization_read(
    r: &mut Reader<'_>,
) -> Result<Option<crate::FixnumSpecialization>, ParseError> {
    match r.u()? {
        0 => Ok(None),
        1 => {
            let operation = crate::FixnumOp::from_tag(r.u()?)
                .ok_or_else(|| ParseError("bad fixnum operation".into()))?;
            let mask = u8(r.u()?)?;
            crate::FixnumSpecialization::from_mask(operation, mask)
                .map(Some)
                .ok_or_else(|| ParseError("bad fixnum guard mask".into()))
        }
        _ => Err(ParseError("bad fixnum specialization flag".into())), // check-added-lines: allow(wildcard) reject unknown serialized flag tags.
    }
}

pub(super) fn scalar_type_hint_read(r: &mut Reader<'_>) -> Result<ScalarTypeHint, ParseError> {
    Ok(match r.u()? {
        0 => ScalarTypeHint::Fixnum,
        1 => ScalarTypeHint::Integer,
        2 => ScalarTypeHint::Number,
        3 => ScalarTypeHint::Boolean,
        4 => ScalarTypeHint::NonNumber,
        _ => return Err(ParseError("bad scalar type hint".into())), // check-added-lines: allow(wildcard) reject unknown serialized type tags.
    })
}
