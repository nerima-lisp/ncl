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
        let mut header = [0; 64];
        header[..8].copy_from_slice(b"NCLFASL\0");
        header[8..10].copy_from_slice(&2u16.to_le_bytes());
        header[10..14].copy_from_slice(&[fasl.header.architecture.wire_tag(), 8, 1, 64]);
        header[16..24].copy_from_slice(&fasl.header.features.to_le_bytes());
        let reloc_bytes = encode_relocations(&fasl.sections.relocations)?;
        let sections = [
            fasl.sections.code.as_slice(),
            reloc_bytes.as_slice(),
            fasl.sections.constants.as_slice(),
            fasl.sections.symbols.as_slice(),
            fasl.sections.stack_maps.as_slice(),
            fasl.sections.debug.as_slice(),
        ];
        let mut file_size = 64_u64;
        for section in &sections {
            let size = section.len() as u64;
            if size > u64::from(u32::MAX) {
                return Err(ObjectError::InvalidField {
                    field: "section size",
                    value: size,
                });
            }
            file_size += size;
            if file_size > u64::from(u32::MAX) {
                return Err(ObjectError::InvalidField {
                    field: "file size",
                    value: file_size,
                });
            }
        }
        let mut out = header.to_vec();
        let mut ranges = [(0_u32, 0_u32); 6];
        for (index, section) in sections.iter().enumerate() {
            ranges[index] = (out.len() as u32, section.len() as u32);
            out.extend_from_slice(section);
        }
        let [
            (code_offset, code_size),
            (reloc_offset, _),
            (constant_offset, constant_size),
            (symbol_offset, symbol_size),
            (stack_offset, stack_size),
            _,
        ] = ranges;
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
            out[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        let relocation_count = fasl.sections.relocations.len() as u32;
        out[36..40].copy_from_slice(&relocation_count.to_le_bytes());
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
        let relocations = decode_relocations(reloc, version, u32_at(bytes, 36))?;
        for relocation in &relocations {
            if relocation.section.0 != 0 {
                let index = relocation.section.0 as usize;
                return Err(ObjectError::InvalidReference {
                    kind: "FASL relocation section",
                    index,
                });
            }
            let relocation_width = width(relocation.kind);
            let offset = relocation.offset as usize;
            let end = offset + relocation_width as usize;
            if end > code.len() {
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
                debug: bytes[debug_start as usize..].to_vec(),
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
    let version = u16_at(bytes, 8);
    if !matches!(version, 1 | 2) {
        return Err(ObjectError::InvalidField {
            field: "version",
            value: u64::from(version),
        });
    }
    let actual_architecture = bytes[10];
    if actual_architecture != architecture.wire_tag() {
        return Err(ObjectError::InvalidField {
            field: "architecture",
            value: u64::from(actual_architecture),
        });
    }
    let pointer_width = bytes[11];
    if pointer_width != 8 {
        return Err(ObjectError::InvalidField {
            field: "pointer width",
            value: u64::from(pointer_width),
        });
    }
    let endianness = bytes[12];
    let header_size = bytes[13];
    if endianness != 1 || header_size != 64 {
        return Err(ObjectError::InvalidField {
            field: "header",
            value: u64::from(header_size),
        });
    }
    if u16_at(bytes, 14) != 0 {
        return Err(ObjectError::InvalidField {
            field: "reserved",
            value: u64::from(u16_at(bytes, 14)),
        });
    }
    let actual_features = u64_at(bytes, 16);
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
    let code = range(bytes, "code", u32_at(bytes, 24), u32_at(bytes, 28))?;
    let reloc_offset = u32_at(bytes, 32);
    let reloc_count = u32_at(bytes, 36);
    let constant_offset = u32_at(bytes, 40);
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
    let constants = range(bytes, "constant", u32_at(bytes, 40), u32_at(bytes, 44))?;
    let symbols = range(bytes, "symbol", u32_at(bytes, 48), u32_at(bytes, 52))?;
    let stack_offset = u32_at(bytes, 56);
    let stack_size = u32_at(bytes, 60);
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
        ("code", u32_at(bytes, 24), u32_at(bytes, 28)),
        ("relocation", reloc_offset, reloc_size),
        ("constant", u32_at(bytes, 40), u32_at(bytes, 44)),
        ("symbol", u32_at(bytes, 48), u32_at(bytes, 52)),
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

fn u16_at(b: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([b[p], b[p + 1]])
}
fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}
fn u64_at(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes([
        b[p],
        b[p + 1],
        b[p + 2],
        b[p + 3],
        b[p + 4],
        b[p + 5],
        b[p + 6],
        b[p + 7],
    ])
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
    let start = offset as usize;
    let end = end as usize;
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
