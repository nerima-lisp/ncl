use crate::{ObjectError, RelocKind, Relocation, SectionId, SymbolRef};

#[cfg(test)]
#[path = "fasl_coverage_tests.rs"]
mod coverage_tests;

/// Architecture encoded by NCL binary formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Architecture {
    /// 64-bit x86.
    X86_64 = 1,
    /// 64-bit `AArch64`.
    Aarch64 = 2,
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
        out[0..8].copy_from_slice(b"NCLFASL\0");
        out[8..10].copy_from_slice(&1u16.to_le_bytes());
        out[10] = fasl.header.architecture as u8;
        out[11] = 8;
        out[12] = 1;
        out[13] = 64;
        out[16..24].copy_from_slice(&fasl.header.features.to_le_bytes());
        let mut append = |bytes: &[u8]| -> Result<(u32, u32), ObjectError> {
            let offset = u32::try_from(out.len()).map_err(|_| ObjectError::InvalidField {
                field: "file size",
                value: out.len() as u64,
            })?;
            let size = u32::try_from(bytes.len()).map_err(|_| ObjectError::InvalidField {
                field: "section size",
                value: bytes.len() as u64,
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
            out[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        out[36..40].copy_from_slice(
            &(u32::try_from(fasl.sections.relocations.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "relocation count",
                    value: u64::MAX,
                }
            })?)
            .to_le_bytes(),
        );
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
        validate_fasl_header(bytes, architecture, features)?;
        let FaslRanges {
            code,
            reloc,
            constants,
            symbols,
            stack,
            debug_start,
        } = read_fasl_ranges(bytes)?;
        let relocations = decode_relocations(reloc)?;
        for relocation in &relocations {
            if relocation.section.0 != 0 {
                return Err(ObjectError::InvalidReference {
                    kind: "FASL relocation section",
                    index: usize::try_from(relocation.section.0).unwrap_or(usize::MAX),
                });
            }
            if usize::try_from(relocation.offset).map_or(true, |offset| offset >= code.len()) {
                return Err(ObjectError::OutOfBounds {
                    section: "FASL relocation",
                    offset: u64::from(relocation.offset),
                    size: 1,
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
                        usize::try_from(debug_start).map_err(|_| ObjectError::InvalidField {
                            field: "debug offset",
                            value: u64::from(debug_start),
                        })?..,
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
) -> Result<(), ObjectError> {
    if bytes.len() < 64 {
        return Err(ObjectError::Truncated {
            offset: bytes.len(),
            needed: 64,
        });
    }
    if &bytes[0..8] != b"NCLFASL\0" {
        return Err(ObjectError::InvalidField {
            field: "magic",
            value: 0,
        });
    }
    if u16_at(bytes, 8)? != 1 {
        return Err(ObjectError::InvalidField {
            field: "version",
            value: u64::from(u16_at(bytes, 8)?),
        });
    }
    if bytes[10] != architecture as u8 {
        return Err(ObjectError::InvalidField {
            field: "architecture",
            value: u64::from(bytes[10]),
        });
    }
    if bytes[11] != 8 {
        return Err(ObjectError::InvalidField {
            field: "pointer width",
            value: u64::from(bytes[11]),
        });
    }
    if bytes[12] != 1 || bytes[13] != 64 {
        return Err(ObjectError::InvalidField {
            field: "header",
            value: u64::from(bytes[13]),
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
    Ok(())
}

struct FaslRanges<'a> {
    code: &'a [u8],
    reloc: &'a [u8],
    constants: &'a [u8],
    symbols: &'a [u8],
    stack: &'a [u8],
    debug_start: u32,
}

fn read_fasl_ranges(bytes: &[u8]) -> Result<FaslRanges<'_>, ObjectError> {
    let code = range(bytes, "code", u32_at(bytes, 24)?, u32_at(bytes, 28)?)?;
    let reloc_size = u32_at(bytes, 36)?
        .checked_mul(16)
        .ok_or(ObjectError::InvalidField {
            field: "relocation size",
            value: u64::MAX,
        })?;
    let reloc = range(bytes, "relocation", u32_at(bytes, 32)?, reloc_size)?;
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
        ("relocation", u32_at(bytes, 32)?, reloc_size),
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

fn encode_relocations(items: &[Relocation]) -> Result<Vec<u8>, ObjectError> {
    let mut out = Vec::with_capacity(items.len() * 16);
    for item in items {
        out.extend_from_slice(&item.section.0.to_le_bytes());
        out.extend_from_slice(&item.offset.to_le_bytes());
        out.extend_from_slice(&(kind_number(item.kind)).to_le_bytes());
        out.extend_from_slice(
            &i32::try_from(item.addend)
                .map_err(|_| ObjectError::InvalidField {
                    field: "relocation addend",
                    value: item.addend.unsigned_abs(),
                })?
                .to_le_bytes(),
        );
    }
    Ok(out)
}
fn decode_relocations(bytes: &[u8]) -> Result<Vec<Relocation>, ObjectError> {
    if !bytes.len().is_multiple_of(16) {
        return Err(ObjectError::InvalidField {
            field: "relocation size",
            value: bytes.len() as u64,
        });
    }
    bytes
        .as_chunks::<16>()
        .0
        .iter()
        .map(|r| {
            Ok(Relocation {
                section: SectionId(u32::from_le_bytes(r[0..4].try_into().map_err(|_| {
                    ObjectError::Truncated {
                        offset: 0,
                        needed: 4,
                    }
                })?)),
                offset: u32::from_le_bytes(r[4..8].try_into().map_err(|_| {
                    ObjectError::Truncated {
                        offset: 4,
                        needed: 4,
                    }
                })?),
                kind: number_kind(u32::from_le_bytes(r[8..12].try_into().map_err(|_| {
                    ObjectError::Truncated {
                        offset: 8,
                        needed: 4,
                    }
                })?))?,
                symbol: SymbolRef::Local(0),
                addend: i64::from(i32::from_le_bytes(r[12..16].try_into().map_err(|_| {
                    ObjectError::Truncated {
                        offset: 12,
                        needed: 4,
                    }
                })?)),
            })
        })
        .collect()
}
const fn kind_number(kind: RelocKind) -> u32 {
    match kind {
        RelocKind::Abs64 => 0,
        RelocKind::PcRel32 => 1,
        RelocKind::Plt32 => 2,
        RelocKind::Branch26 => 3,
        RelocKind::Adrp21 => 4,
        RelocKind::Add12 => 5,
        RelocKind::CondBranch19 => 6,
        RelocKind::CodeEntry => 7,
        RelocKind::ExternalSymbol => 8,
    }
}
fn number_kind(value: u32) -> Result<RelocKind, ObjectError> {
    [
        RelocKind::Abs64,
        RelocKind::PcRel32,
        RelocKind::Plt32,
        RelocKind::Branch26,
        RelocKind::Adrp21,
        RelocKind::Add12,
        RelocKind::CondBranch19,
        RelocKind::CodeEntry,
        RelocKind::ExternalSymbol,
    ]
    .get(value as usize)
    .copied()
    .ok_or_else(|| ObjectError::InvalidField {
        field: "relocation kind",
        value: u64::from(value),
    })
}
fn u16_at(b: &[u8], p: usize) -> Result<u16, ObjectError> {
    b.get(p..p + 2)
        .ok_or(ObjectError::Truncated {
            offset: p,
            needed: 2,
        })
        .map(|x| u16::from_le_bytes([x[0], x[1]]))
}
fn u32_at(b: &[u8], p: usize) -> Result<u32, ObjectError> {
    b.get(p..p + 4)
        .ok_or(ObjectError::Truncated {
            offset: p,
            needed: 4,
        })
        .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}
fn u64_at(b: &[u8], p: usize) -> Result<u64, ObjectError> {
    b.get(p..p + 8)
        .ok_or(ObjectError::Truncated {
            offset: p,
            needed: 8,
        })
        .map(|x| u64::from_le_bytes([x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]]))
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
    bytes
        .get(offset as usize..end as usize)
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
