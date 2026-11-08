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
        let record_size = (20 + symbol.package.len() + symbol.name.len() + 3) & !3;
        section_size += record_size;
        check_section_size(section_size, None)?;
    }

    let count = symbols.len() as u32;
    let mut output = SectionWriter::new();
    output.put_u32(count);
    for symbol in symbols {
        output.put_u32(symbol.id);
        output.put_u32(symbol.binding_kind.tag());
        output.put_u32(0);
        write_text(&mut output, &symbol.package);
        write_text(&mut output, &symbol.name);
        output.zero_padding(4);
    }
    Ok(output.into_bytes())
}

/// Decodes the canonical symbol section. `base_offset` is its file offset.
pub fn decode(bytes: &[u8], base_offset: usize) -> Result<Vec<FaslSymbol>, FaslError> {
    check_section_size(bytes.len(), Some(base_offset))?;
    let mut reader = SectionReader::new(bytes, base_offset, FaslRejectReason::InvalidReference);
    let count_offset = reader.absolute_position();
    let count = reader.u32()? as usize;
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

fn check_section_size(size: usize, offset: Option<usize>) -> Result<(), FaslError> {
    if size <= MAX_SECTION_BYTES {
        return Ok(());
    }
    Err(offset.map_or_else(
        || FaslError::new(FaslRejectReason::ResourceLimitExceeded),
        |offset| FaslError::at(FaslRejectReason::ResourceLimitExceeded, offset),
    ))
}

fn write_text(output: &mut SectionWriter, value: &str) {
    let length = value.len() as u32;
    output.put_u32(length);
    output.put_bytes(value.as_bytes());
}

fn read_text(reader: &mut SectionReader<'_>) -> Result<String, FaslError> {
    let length_offset = reader.absolute_position();
    let length = reader.u32()? as usize;
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
#[path = "fasl_symbols_tests.rs"]
mod tests;
