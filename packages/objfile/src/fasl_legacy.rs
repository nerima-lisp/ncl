use crate::fasl_relocation as relocation_wire;
use crate::{ObjectError, Relocation};
use relocation_wire::{decode as decode_relocations, encode as encode_relocations, width};

#[cfg(test)]
#[path = "fasl_coverage_tests.rs"]
mod coverage_tests;

#[cfg(test)]
#[path = "fasl_legacy_edge_tests.rs"]
mod edge_tests;

/// Architecture encoded by NCL binary formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Architecture {
    /// 64-bit x86.
    X86_64 = 1,
    /// 64-bit `AArch64`.
    Aarch64 = 2,
}

impl Architecture {
    /// Returns the stable architecture tag used by FASL headers.
    #[must_use]
    pub const fn wire_tag(self) -> u8 {
        match self {
            Self::X86_64 => 1,
            Self::Aarch64 => 2,
        }
    }
}

/// The fixed FASL header values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FaslHeader {
    /// Target instruction-set architecture.
    pub architecture: Architecture,
    /// Feature bits required by the image.
    pub features: u64,
}

/// The logical sections carried by a FASL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaslSection {
    /// Machine code bytes.
    pub code: Vec<u8>,
    /// Code and metadata relocations.
    pub relocations: Vec<Relocation>,
    /// Tagged constant payload.
    pub constants: Vec<u8>,
    /// Symbol binding payload.
    pub symbols: Vec<u8>,
    /// Safepoint map payload.
    pub stack_maps: Vec<u8>,
    /// Debug record payload.
    pub debug: Vec<u8>,
}

/// A complete FASL value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fasl {
    /// Fixed header values.
    pub header: FaslHeader,
    /// Variable sections in wire order.
    pub sections: FaslSection,
}

/// Stateless FASL serializer.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaslWriter;

impl FaslWriter {
    /// Serializes a FASL with the fixed 64-byte little-endian header.
    ///
    /// # Errors
    ///
    /// Returns an error when a section or relocation exceeds the wire limits.
    pub fn write(fasl: &Fasl) -> Result<Vec<u8>, ObjectError> {
        let mut out = vec![0; 64];
        write_at(&mut out, 0, b"NCLFASL\0", "FASL header")?;
        write_at(&mut out, 8, &2u16.to_le_bytes(), "FASL header")?;
        write_at(
            &mut out,
            10,
            &[fasl.header.architecture.wire_tag(), 8, 1, 64],
            "FASL header",
        )?;
        write_at(
            &mut out,
            16,
            &fasl.header.features.to_le_bytes(),
            "FASL header",
        )?;
        let mut append = |bytes: &[u8]| -> Result<(u32, u32), ObjectError> {
            let offset = u32::try_from(out.len()).map_err(|_| ObjectError::InvalidField {
                field: "file size",
                value: u64::MAX,
            })?;
            let size = u32::try_from(bytes.len()).map_err(|_| ObjectError::InvalidField {
                field: "section size",
                value: u64::MAX,
            })?;
            out.extend_from_slice(bytes);
            Ok((offset, size))
        };
        let (code_offset, code_size) = append(&fasl.sections.code)?;
        let reloc_bytes = encode_relocations(&fasl.sections.relocations)?;
        let (reloc_offset, _) = append(&reloc_bytes)?;
        let (constant_offset, constant_size) = append(&fasl.sections.constants)?;
        let (symbol_offset, symbol_size) = append(&fasl.sections.symbols)?;
        let (stack_offset, stack_size) = append(&fasl.sections.stack_maps)?;
        append(&fasl.sections.debug)?;
        for (at, value) in [
            (24, code_offset),
            (28, code_size),
            (32, reloc_offset),
            (40, constant_offset),
            (44, constant_size),
            (48, symbol_offset),
            (52, symbol_size),
            (56, stack_offset),
            (60, stack_size),
        ] {
            write_at(&mut out, at, &value.to_le_bytes(), "FASL header")?;
        }
        write_at(
            &mut out,
            36,
            &(u32::try_from(fasl.sections.relocations.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "relocation count",
                    value: u64::MAX,
                }
            })?)
            .to_le_bytes(),
            "FASL header",
        )?;
        Ok(out)
    }
}

/// Stateless FASL validator and reader.
#[derive(Clone, Copy, Debug, Default)]
pub struct FaslReader;

impl FaslReader {
    /// Validates and reads a FASL, rejecting incompatible headers before sections.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed input or an incompatible target header.
    pub fn read(
        bytes: &[u8],
        architecture: Architecture,
        features: u64,
    ) -> Result<Fasl, ObjectError> {
        let version = validate_fasl_header(bytes, architecture, features)?;
        let FaslRanges {
            code,
            reloc,
            constants,
            symbols,
            stack,
            debug_start,
        } = read_fasl_ranges(bytes, version)?;
        let relocations = decode_relocations(reloc, version, u32_at(bytes, 36)?)?;
        for relocation in &relocations {
            if relocation.section.0 != 0 {
                let index = usize::try_from(relocation.section.0).map_err(|_| {
                    ObjectError::InvalidField {
                        field: "FASL relocation section",
                        value: u64::from(relocation.section.0),
                    }
                })?;
                return Err(ObjectError::InvalidReference {
                    kind: "FASL relocation section",
                    index,
                });
            }
            let relocation_width = width(relocation.kind);
            let relocation_width_in_bytes =
                usize::try_from(relocation_width).map_err(|_| ObjectError::InvalidField {
                    field: "relocation width",
                    value: u64::from(relocation_width),
                })?;
            if usize::try_from(relocation.offset)
                .ok()
                .and_then(|offset| offset.checked_add(relocation_width_in_bytes))
                .is_none_or(|end| end > code.len())
            {
                return Err(ObjectError::OutOfBounds {
                    section: "FASL relocation",
                    offset: u64::from(relocation.offset),
                    size: u64::from(relocation_width),
                });
            }
        }
        Ok(Fasl {
            header: FaslHeader {
                architecture,
                features,
            },
            sections: FaslSection {
                code: code.to_vec(),
                relocations,
                constants: constants.to_vec(),
                symbols: symbols.to_vec(),
                stack_maps: stack.to_vec(),
                debug: bytes
                    .get(
                        crate::types::target_usize(
                            u64::from(debug_start),
                            "debug offset",
                            u64::from(debug_start),
                        )?..,
                    )
                    .map_or_else(Vec::new, <[u8]>::to_vec),
            },
        })
    }
}

fn validate_fasl_header(
    bytes: &[u8],
    architecture: Architecture,
    features: u64,
) -> Result<u16, ObjectError> {
    if bytes.len() < 64 {
        return Err(ObjectError::Truncated {
            offset: bytes.len(),
            needed: 64,
        });
    }
    if bytes.get(..8) != Some(b"NCLFASL\0".as_slice()) {
        return Err(ObjectError::InvalidField {
            field: "magic",
            value: 0,
        });
    }
    let version = u16_at(bytes, 8)?;
    if !matches!(version, 1 | 2) {
        return Err(ObjectError::InvalidField {
            field: "version",
            value: u64::from(version),
        });
    }
    let actual_architecture = byte_at(bytes, 10)?;
    if actual_architecture != architecture.wire_tag() {
        return Err(ObjectError::InvalidField {
            field: "architecture",
            value: u64::from(actual_architecture),
        });
    }
    let pointer_width = byte_at(bytes, 11)?;
    if pointer_width != 8 {
        return Err(ObjectError::InvalidField {
            field: "pointer width",
            value: u64::from(pointer_width),
        });
    }
    let endianness = byte_at(bytes, 12)?;
    let header_size = byte_at(bytes, 13)?;
    if endianness != 1 || header_size != 64 {
        return Err(ObjectError::InvalidField {
            field: "header",
            value: u64::from(header_size),
        });
    }
    if u16_at(bytes, 14)? != 0 {
        return Err(ObjectError::InvalidField {
            field: "reserved",
            value: u64::from(u16_at(bytes, 14)?),
        });
    }
    let actual_features = u64_at(bytes, 16)?;
    if actual_features != features {
        return Err(ObjectError::InvalidField {
            field: "feature bitmap",
            value: actual_features,
        });
    }
    Ok(version)
}

struct FaslRanges<'a> {
    code: &'a [u8],
    reloc: &'a [u8],
    constants: &'a [u8],
    symbols: &'a [u8],
    stack: &'a [u8],
    debug_start: u32,
}

fn read_fasl_ranges(bytes: &[u8], version: u16) -> Result<FaslRanges<'_>, ObjectError> {
    let code = range(bytes, "code", u32_at(bytes, 24)?, u32_at(bytes, 28)?)?;
    let reloc_offset = u32_at(bytes, 32)?;
    let reloc_count = u32_at(bytes, 36)?;
    let constant_offset = u32_at(bytes, 40)?;
    let reloc_size = if version == 1 {
        reloc_count
            .checked_mul(16)
            .ok_or(ObjectError::InvalidField {
                field: "relocation size",
                value: u64::MAX,
            })?
    } else {
        constant_offset
            .checked_sub(reloc_offset)
            .ok_or(ObjectError::Overlap {
                first: "relocation",
                second: "constant",
            })?
    };
    let reloc = range(bytes, "relocation", reloc_offset, reloc_size)?;
    let constants = range(bytes, "constant", u32_at(bytes, 40)?, u32_at(bytes, 44)?)?;
    let symbols = range(bytes, "symbol", u32_at(bytes, 48)?, u32_at(bytes, 52)?)?;
    let stack_offset = u32_at(bytes, 56)?;
    let stack_size = u32_at(bytes, 60)?;
    let stack = range(bytes, "stack map", stack_offset, stack_size)?;
    let debug_start =
        stack_offset
            .checked_add(stack_size)
            .ok_or_else(|| ObjectError::OutOfBounds {
                section: "debug",
                offset: u64::from(stack_offset),
                size: u64::from(stack_size),
            })?;
    validate_section_order(&[
        ("code", u32_at(bytes, 24)?, u32_at(bytes, 28)?),
        ("relocation", reloc_offset, reloc_size),
        ("constant", u32_at(bytes, 40)?, u32_at(bytes, 44)?),
        ("symbol", u32_at(bytes, 48)?, u32_at(bytes, 52)?),
        ("stack map", stack_offset, stack_size),
    ])?;
    Ok(FaslRanges {
        code,
        reloc,
        constants,
        symbols,
        stack,
        debug_start,
    })
}

fn fixed_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ObjectError> {
    let end = offset
        .checked_add(N)
        .ok_or(ObjectError::Truncated { offset, needed: N })?;
    let source = bytes
        .get(offset..end)
        .ok_or(ObjectError::Truncated { offset, needed: N })?;
    let mut value = [0; N];
    value.copy_from_slice(source);
    Ok(value)
}

fn byte_at(bytes: &[u8], offset: usize) -> Result<u8, ObjectError> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ObjectError::Truncated { offset, needed: 1 })
}

fn write_at(
    bytes: &mut [u8],
    offset: usize,
    value: &[u8],
    section: &'static str,
) -> Result<(), ObjectError> {
    let size = u64::try_from(value.len()).unwrap_or(u64::MAX);
    let error = || ObjectError::OutOfBounds {
        section,
        offset: u64::try_from(offset).unwrap_or(u64::MAX),
        size,
    };
    let end = offset.checked_add(value.len()).ok_or_else(error)?;
    let destination = bytes.get_mut(offset..end).ok_or_else(error)?;
    destination.copy_from_slice(value);
    Ok(())
}

fn u16_at(b: &[u8], p: usize) -> Result<u16, ObjectError> {
    Ok(u16::from_le_bytes(fixed_array(b, p)?))
}
fn u32_at(b: &[u8], p: usize) -> Result<u32, ObjectError> {
    Ok(u32::from_le_bytes(fixed_array(b, p)?))
}
fn u64_at(b: &[u8], p: usize) -> Result<u64, ObjectError> {
    Ok(u64::from_le_bytes(fixed_array(b, p)?))
}
fn range<'a>(
    bytes: &'a [u8],
    name: &'static str,
    offset: u32,
    size: u32,
) -> Result<&'a [u8], ObjectError> {
    let end = offset
        .checked_add(size)
        .ok_or_else(|| ObjectError::OutOfBounds {
            section: name,
            offset: u64::from(offset),
            size: u64::from(size),
        })?;
    let start = usize::try_from(offset).map_err(|_| ObjectError::OutOfBounds {
        section: name,
        offset: u64::from(offset),
        size: u64::from(size),
    })?;
    let end = usize::try_from(end).map_err(|_| ObjectError::OutOfBounds {
        section: name,
        offset: u64::from(offset),
        size: u64::from(size),
    })?;
    bytes
        .get(start..end)
        .ok_or_else(|| ObjectError::OutOfBounds {
            section: name,
            offset: u64::from(offset),
            size: u64::from(size),
        })
}

fn validate_section_order(sections: &[(&'static str, u32, u32)]) -> Result<(), ObjectError> {
    let mut previous_end = 64u32;
    for (name, offset, size) in sections {
        if *offset < previous_end {
            return Err(ObjectError::Overlap {
                first: "previous FASL section",
                second: name,
            });
        }
        let end = offset
            .checked_add(*size)
            .ok_or_else(|| ObjectError::OutOfBounds {
                section: name,
                offset: u64::from(*offset),
                size: u64::from(*size),
            })?;
        previous_end = end;
    }
    Ok(())
}
