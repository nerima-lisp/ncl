use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::{SectionReader, SectionWriter};

const MAX_RECORDS: usize = 1_048_576;
const MAX_STRING_BYTES: usize = 1_048_576;
const MAX_SECTION_BYTES: usize = 256 * 1024 * 1024;

/// The kind of binding described by a FASL symbol record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum SymbolBindingKind {
    /// A Common Lisp symbol binding.
    Lisp,
    /// An imported native function binding.
    NativeFunction,
}

impl SymbolBindingKind {
    const fn tag(self) -> u32 {
        match self {
            Self::Lisp => 1,
            Self::NativeFunction => 2,
        }
    }

    const fn from_tag(tag: u32, offset: usize) -> Result<Self, FaslError> {
        match tag {
            1 => Ok(Self::Lisp),
            2 => Ok(Self::NativeFunction),
            0 | 3..=u32::MAX => Err(FaslError::at(FaslRejectReason::InvalidReference, offset)),
        }
    }
}

/// One imported symbol or native function name in the FASL symbol table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaslSymbol {
    /// Dense identifier used by constant and relocation records.
    pub id: u32,
    /// Lisp or native-function binding category.
    pub binding_kind: SymbolBindingKind,
    /// Package or native import namespace.
    pub package: String,
    /// External symbol name.
    pub name: String,
}

/// Encodes the canonical symbol section.
pub fn encode(symbols: &[FaslSymbol]) -> Result<Vec<u8>, FaslError> {
    check_record_count(symbols.len())?;
    validate_order(symbols)?;
    let mut section_size = 4_usize;
    for symbol in symbols {
        validate_text(&symbol.package)?;
        validate_text(&symbol.name)?;
        let record_size = 20_usize
            .checked_add(symbol.package.len())
            .and_then(|size| size.checked_add(symbol.name.len()))
            .and_then(|size| size.checked_add(3))
            .map(|size| size & !3)
            .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        section_size = section_size
            .checked_add(record_size)
            .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        if section_size > MAX_SECTION_BYTES {
            return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
        }
    }

    let count = u32::try_from(symbols.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    let mut output = SectionWriter::new();
    output.put_u32(count);
    for symbol in symbols {
        validate_text(&symbol.package)?;
        validate_text(&symbol.name)?;
        output.put_u32(symbol.id);
        output.put_u32(symbol.binding_kind.tag());
        output.put_u32(0);
        write_text(&mut output, &symbol.package)?;
        write_text(&mut output, &symbol.name)?;
        output.zero_padding(4);
    }
    Ok(output.into_bytes())
}

/// Decodes the canonical symbol section. `base_offset` is its file offset.
pub fn decode(bytes: &[u8], base_offset: usize) -> Result<Vec<FaslSymbol>, FaslError> {
    if bytes.len() > MAX_SECTION_BYTES {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            base_offset,
        ));
    }
    let mut reader = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidReference);
    let count_offset = reader.absolute_position();
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, count_offset))?;
    if count > MAX_RECORDS {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            count_offset,
        ));
    }
    if count > reader.remaining() / 20 {
        return Err(FaslError::at(
            FaslRejectReason::InvalidReference,
            count_offset,
        ));
    }
    let mut symbols = Vec::with_capacity(count);
    for _ in 0..count {
        let record_offset = reader.absolute_position();
        let id = reader.u32()?;
        if usize::try_from(id).ok() != Some(symbols.len()) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidReference,
                record_offset,
            ));
        }
        let kind_offset = reader.absolute_position();
        let binding_kind = SymbolBindingKind::from_tag(reader.u32()?, kind_offset)?;
        let flags_offset = reader.absolute_position();
        if reader.u32()? != 0 {
            return Err(FaslError::at(
                FaslRejectReason::InvalidReference,
                flags_offset,
            ));
        }
        let package = read_text(&mut reader)?;
        let name = read_text(&mut reader)?;
        reader.zero_padding(4)?;
        let current = (binding_kind.tag(), package.as_str(), name.as_str());
        if symbols.last().is_some_and(|prior: &FaslSymbol| {
            (
                prior.binding_kind.tag(),
                prior.package.as_str(),
                prior.name.as_str(),
            ) >= current
        }) {
            return Err(FaslError::at(
                FaslRejectReason::InvalidReference,
                record_offset,
            ));
        }
        symbols.push(FaslSymbol {
            id,
            binding_kind,
            package,
            name,
        });
    }
    reader.finish()?;
    Ok(symbols)
}

fn validate_order(symbols: &[FaslSymbol]) -> Result<(), FaslError> {
    let mut previous: Option<(u32, &str, &str)> = None;
    for (index, symbol) in symbols.iter().enumerate() {
        if usize::try_from(symbol.id).ok() != Some(index) {
            return Err(FaslError::new(FaslRejectReason::InvalidReference));
        }
        validate_text(&symbol.package)?;
        validate_text(&symbol.name)?;
        let current = (
            symbol.binding_kind.tag(),
            symbol.package.as_str(),
            symbol.name.as_str(),
        );
        if previous.is_some_and(|prior| prior >= current) {
            return Err(FaslError::new(FaslRejectReason::InvalidReference));
        }
        previous = Some(current);
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), FaslError> {
    if value.len() > MAX_STRING_BYTES || value.as_bytes().contains(&0) {
        return Err(FaslError::new(if value.len() > MAX_STRING_BYTES {
            FaslRejectReason::ResourceLimitExceeded
        } else {
            FaslRejectReason::InvalidReference
        }));
    }
    Ok(())
}

const fn check_record_count(count: usize) -> Result<(), FaslError> {
    if count > MAX_RECORDS {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(())
}

fn write_text(output: &mut SectionWriter, value: &str) -> Result<(), FaslError> {
    let length = u32::try_from(value.len())
        .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
    output.put_u32(length);
    output.put_bytes(value.as_bytes());
    Ok(())
}

fn read_text(reader: &mut SectionReader<'_>) -> Result<String, FaslError> {
    let length_offset = reader.absolute_position();
    let length = usize::try_from(reader.u32()?)
        .map_err(|_| FaslError::at(FaslRejectReason::ResourceLimitExceeded, length_offset))?;
    if length > MAX_STRING_BYTES {
        return Err(FaslError::at(
            FaslRejectReason::ResourceLimitExceeded,
            length_offset,
        ));
    }
    let bytes_offset = reader.absolute_position();
    let bytes = reader.take(length)?;
    if bytes.contains(&0) {
        return Err(FaslError::at(
            FaslRejectReason::InvalidReference,
            bytes_offset,
        ));
    }
    let value = core::str::from_utf8(bytes).map_err(|error| {
        FaslError::at(
            FaslRejectReason::InvalidReference,
            bytes_offset.saturating_add(error.valid_up_to()),
        )
    })?;
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

    #[test]
    fn symbol_section_matches_canonical_bytes_and_round_trips() {
        let symbols = [FaslSymbol {
            id: 0,
            binding_kind: SymbolBindingKind::Lisp,
            package: "CL-USER".into(),
            name: "X".into(),
        }];
        let expected = [
            1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, b'C', b'L', b'-', b'U',
            b'S', b'E', b'R', 1, 0, 0, 0, b'X',
        ];
        let encoded = encode(&symbols);
        assert_eq!(encoded, Ok(expected.to_vec()));
        assert_eq!(decode(&expected, 320), Ok(symbols.to_vec()));
    }

    #[test]
    fn symbol_section_rejects_bad_order_flags_strings_and_padding() {
        let valid = encode(&[FaslSymbol {
            id: 0,
            binding_kind: SymbolBindingKind::Lisp,
            package: String::new(),
            name: "A".into(),
        }]);
        let valid = must_succeed(valid);

        let mut bad_flags = valid.clone();
        bad_flags[12..16].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            decode(&bad_flags, 320).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::InvalidReference, 332))
        );

        let mut bad_padding = valid;
        let last = bad_padding.len() - 1;
        bad_padding[last] = 1;
        assert_eq!(
            decode(&bad_padding, 320).map(|_| ()),
            Err(FaslError::at(
                FaslRejectReason::InvalidReference,
                320 + last
            ))
        );

        let unsorted = [
            FaslSymbol {
                id: 0,
                binding_kind: SymbolBindingKind::NativeFunction,
                package: "P".into(),
                name: "Z".into(),
            },
            FaslSymbol {
                id: 1,
                binding_kind: SymbolBindingKind::Lisp,
                package: "P".into(),
                name: "A".into(),
            },
        ];
        assert_eq!(
            encode(&unsorted),
            Err(FaslError::new(FaslRejectReason::InvalidReference))
        );
        assert_eq!(
            encode(&[FaslSymbol {
                id: 0,
                binding_kind: SymbolBindingKind::Lisp,
                package: "P\0".into(),
                name: "A".into(),
            }]),
            Err(FaslError::new(FaslRejectReason::InvalidReference))
        );
    }
}
