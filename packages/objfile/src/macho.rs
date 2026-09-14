use crate::{ObjectError, Relocation, SectionId};

/// Mach-O CPU architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachArchitecture {
    /// x86-64 Mach-O CPU type.
    X86_64,
    /// arm64 Mach-O CPU type.
    Arm64,
}

/// A Mach-O section with its segment and section names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachSection {
    /// Generic section identifier.
    pub id: SectionId,
    /// Mach-O segment name.
    pub segment: String,
    /// Mach-O section name.
    pub name: String,
    /// Section payload.
    pub bytes: Vec<u8>,
}

/// A relocatable 64-bit Mach-O object description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachObject {
    /// Target CPU.
    pub architecture: MachArchitecture,
    /// Sections to emit.
    pub sections: Vec<MachSection>,
    /// Relocation records.
    pub relocations: Vec<Relocation>,
}

impl MachObject {
    /// Writes a structurally valid Mach-O 64-bit relocatable object.
    ///
    /// # Errors
    ///
    /// Returns an error when section counts or sizes exceed Mach-O limits.
    pub fn write(&self) -> Result<Vec<u8>, ObjectError> {
        validate_input(self)?;
        let cputype = match self.architecture {
            MachArchitecture::X86_64 => 0x0100_0007_u32,
            MachArchitecture::Arm64 => 0x0100_000c_u32,
        };
        let nsects = u32::try_from(self.sections.len()).map_err(|_| ObjectError::InvalidField {
            field: "section count",
            value: u64::MAX,
        })?;
        let segment_size = 72u32 + nsects * 80;
        let symtab_size = 24u32;
        let commands = segment_size + 24 + 32;
        let header_size = 32usize + commands as usize;
        let mut out = vec![0; header_size];
        let mut offsets = Vec::new();
        for section in &self.sections {
            align(&mut out, 8);
            offsets.push(
                u32::try_from(out.len()).map_err(|_| ObjectError::InvalidField {
                    field: "section offset",
                    value: u64::MAX,
                })?,
            );
            out.extend_from_slice(&section.bytes);
        }
        let mut relocation_offsets = Vec::with_capacity(self.sections.len());
        let mut relocation_counts = Vec::with_capacity(self.sections.len());
        for section in &self.sections {
            let entries: Vec<&Relocation> = self
                .relocations
                .iter()
                .filter(|relocation| relocation.section == section.id)
                .collect();
            align(&mut out, 4);
            relocation_offsets.push(u32::try_from(out.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "relocation offset",
                    value: u64::MAX,
                }
            })?);
            relocation_counts.push(u32::try_from(entries.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "relocation count",
                    value: u64::MAX,
                }
            })?);
            for relocation in entries {
                out.extend_from_slice(
                    &encode_relocation(relocation, self.architecture)?.to_le_bytes(),
                );
            }
        }
        let symoff = u32::try_from(out.len()).map_err(|_| ObjectError::InvalidField {
            field: "symbol table offset",
            value: u64::MAX,
        })?;
        out.extend_from_slice(&[0; 24]);
        let stroff = u32::try_from(out.len()).map_err(|_| ObjectError::InvalidField {
            field: "string table offset",
            value: u64::MAX,
        })?;
        out.push(0);
        let mut cursor = 32usize;
        write_segment(
            &mut out,
            &mut cursor,
            &self.sections,
            &offsets,
            &relocation_offsets,
            &relocation_counts,
            segment_size,
            nsects,
        )?;
        write_u32(&mut out, cursor, 0x2);
        write_u32(&mut out, cursor + 4, 24);
        write_u32(&mut out, cursor + 8, symoff);
        write_u32(&mut out, cursor + 12, 0);
        write_u32(&mut out, cursor + 16, stroff);
        write_u32(&mut out, cursor + 20, 1);
        cursor += 24;
        write_u32(&mut out, cursor, 0x32);
        write_u32(&mut out, cursor + 4, 32);
        write_u32(&mut out, cursor + 8, 1);
        write_u32(&mut out, cursor + 12, 0);
        write_u32(&mut out, cursor + 16, 0);
        write_u32(&mut out, cursor + 20, 0);
        write_u32(&mut out, cursor + 24, 0);
        write_u32(&mut out, cursor + 28, 0);
        out[0..4].copy_from_slice(&0xfeed_facf_u32.to_le_bytes());
        out[4..8].copy_from_slice(&cputype.to_le_bytes());
        out[8..12].copy_from_slice(&3u32.to_le_bytes());
        out[12..16].copy_from_slice(&1u32.to_le_bytes());
        out[16..20].copy_from_slice(&3u32.to_le_bytes());
        out[20..24].copy_from_slice(&commands.to_le_bytes());
        out[24..28].copy_from_slice(&0u32.to_le_bytes());
        out[28..32].copy_from_slice(&0u32.to_le_bytes());
        let _ = symtab_size;
        Ok(out)
    }
}

fn write_segment(
    out: &mut [u8],
    cursor: &mut usize,
    sections: &[MachSection],
    offsets: &[u32],
    relocation_offsets: &[u32],
    relocation_counts: &[u32],
    command_size: u32,
    nsects: u32,
) -> Result<(), ObjectError> {
    write_u32(out, *cursor, 0x19);
    write_u32(out, *cursor + 4, command_size);
    write_name(out, *cursor + 8, "__TEXT");
    write_u32(out, *cursor + 64, nsects);
    write_u32(out, *cursor + 68, 0);
    *cursor += 72;
    for ((section, offset), (relocation_offset, relocation_count)) in sections
        .iter()
        .zip(offsets)
        .zip(relocation_offsets.iter().zip(relocation_counts))
    {
        write_name(out, *cursor, &section.name);
        write_name(out, *cursor + 16, &section.segment);
        write_u64(
            out,
            *cursor + 40,
            u64::try_from(section.bytes.len()).map_err(|_| ObjectError::InvalidField {
                field: "section size",
                value: u64::MAX,
            })?,
        );
        write_u32(out, *cursor + 48, *offset);
        write_u32(out, *cursor + 56, *relocation_offset);
        write_u32(out, *cursor + 60, *relocation_count);
        *cursor += 80;
    }
    Ok(())
}
fn write_name(out: &mut [u8], at: usize, name: &str) {
    let bytes = name.as_bytes();
    let length = bytes.len().min(16);
    out[at..at + length].copy_from_slice(&bytes[..length]);
}
fn write_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn write_u64(out: &mut [u8], at: usize, value: u64) {
    out[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn encode_relocation(
    relocation: &Relocation,
    architecture: MachArchitecture,
) -> Result<u64, ObjectError> {
    let kind = match (architecture, relocation.kind) {
        (
            MachArchitecture::Arm64 | MachArchitecture::X86_64,
            crate::RelocKind::Abs64 | crate::RelocKind::ExternalSymbol,
        ) => 0,
        (MachArchitecture::Arm64, crate::RelocKind::Adrp21) => 3,
        (MachArchitecture::Arm64, crate::RelocKind::Add12) => 4,
        (MachArchitecture::Arm64, crate::RelocKind::Branch26)
        | (MachArchitecture::X86_64, crate::RelocKind::PcRel32 | crate::RelocKind::Plt32) => 2,
        _ => return Err(ObjectError::UnsupportedRelocation(relocation.kind)),
    };
    let (symbol, external) = match relocation.symbol {
        crate::SymbolRef::Local(index) => (index, 0u32),
        crate::SymbolRef::External(_) => (0, 1),
    };
    let pcrel = u32::from(matches!(
        relocation.kind,
        crate::RelocKind::PcRel32 | crate::RelocKind::Plt32
    ));
    let length = if matches!(relocation.kind, crate::RelocKind::Abs64) {
        3
    } else {
        2
    };
    let descriptor = symbol | (pcrel << 24) | (length << 25) | (external << 27) | (kind << 28);
    Ok(
        u64::from(u32::from_le_bytes(relocation.offset.to_le_bytes()))
            | (u64::from(descriptor) << 32),
    )
}
fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let padding = (alignment - bytes.len() % alignment) % alignment;
    bytes.resize(bytes.len() + padding, 0);
}

/// Checks the Mach-O magic, class, and endianness before a caller parses it.
///
/// # Errors
///
/// Returns an error when the input is truncated or targets another CPU.
pub fn validate_macho(bytes: &[u8], architecture: MachArchitecture) -> Result<(), ObjectError> {
    if bytes.len() < 32 {
        return Err(ObjectError::Truncated {
            offset: bytes.len(),
            needed: 32,
        });
    }
    if u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| ObjectError::Truncated {
        offset: 0,
        needed: 4,
    })?) != 0xfeed_facf
    {
        return Err(ObjectError::InvalidField {
            field: "Mach-O magic",
            value: 0,
        });
    }
    let expected = match architecture {
        MachArchitecture::X86_64 => 0x0100_0007,
        MachArchitecture::Arm64 => 0x0100_000c,
    };
    let cpu = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| ObjectError::Truncated {
        offset: 4,
        needed: 4,
    })?);
    if cpu != expected {
        return Err(ObjectError::InvalidField {
            field: "Mach-O CPU",
            value: u64::from(cpu),
        });
    }
    let ncmds =
        u32::from_le_bytes(
            bytes[16..20]
                .try_into()
                .map_err(|_| ObjectError::Truncated {
                    offset: 16,
                    needed: 4,
                })?,
        );
    let sizeofcmds = usize::try_from(u32::from_le_bytes(bytes[20..24].try_into().map_err(
        |_| ObjectError::Truncated {
            offset: 20,
            needed: 4,
        },
    )?))
    .map_err(|_| ObjectError::InvalidField {
        field: "load commands",
        value: u64::MAX,
    })?;
    let command_end = 32usize
        .checked_add(sizeofcmds)
        .ok_or(ObjectError::InvalidStructure("Mach-O command overflow"))?;
    if command_end > bytes.len() || ncmds == 0 {
        return Err(ObjectError::InvalidStructure(
            "invalid Mach-O load commands",
        ));
    }
    let mut cursor = 32usize;
    let mut has_segment = false;
    for _ in 0..ncmds {
        let command = read_u32(bytes, cursor, "Mach-O command")?;
        let size =
            usize::try_from(read_u32(bytes, cursor + 4, "Mach-O command size")?).map_err(|_| {
                ObjectError::InvalidField {
                    field: "Mach-O command size",
                    value: u64::MAX,
                }
            })?;
        if size < 8 || cursor.checked_add(size).is_none_or(|end| end > command_end) {
            return Err(ObjectError::InvalidStructure("invalid Mach-O command size"));
        }
        if command == 0x19 {
            has_segment = true;
            if size < 72 {
                return Err(ObjectError::InvalidStructure("short LC_SEGMENT_64"));
            }
            let nsects = usize::try_from(read_u32(bytes, cursor + 64, "Mach-O section count")?)
                .map_err(|_| ObjectError::InvalidField {
                    field: "Mach-O section count",
                    value: u64::MAX,
                })?;
            let section_bytes = nsects.checked_mul(80).ok_or(ObjectError::InvalidStructure(
                "Mach-O section table overflow",
            ))?;
            if 72usize
                .checked_add(section_bytes)
                .is_none_or(|end| end > size)
            {
                return Err(ObjectError::InvalidStructure("short Mach-O section table"));
            }
            for index in 0..nsects {
                let section = cursor + 72 + index * 80;
                let offset =
                    usize::try_from(read_u32(bytes, section + 48, "Mach-O section offset")?)
                        .map_err(|_| ObjectError::InvalidField {
                            field: "Mach-O section offset",
                            value: u64::MAX,
                        })?;
                let section_size =
                    usize::try_from(read_u64(bytes, section + 40, "Mach-O section size")?)
                        .map_err(|_| ObjectError::InvalidField {
                            field: "Mach-O section size",
                            value: u64::MAX,
                        })?;
                if offset
                    .checked_add(section_size)
                    .is_none_or(|end| end > bytes.len())
                {
                    return Err(ObjectError::OutOfBounds {
                        section: "Mach-O section",
                        offset: u64::try_from(offset).unwrap_or(u64::MAX),
                        size: u64::try_from(section_size).unwrap_or(u64::MAX),
                    });
                }
            }
        }
        cursor += size;
    }
    if !has_segment {
        return Err(ObjectError::InvalidStructure("missing Mach-O load command"));
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize, field: &'static str) -> Result<u32, ObjectError> {
    let end = offset
        .checked_add(4)
        .ok_or(ObjectError::InvalidStructure("Mach-O field overflow"))?;
    bytes
        .get(offset..end)
        .ok_or(ObjectError::Truncated { offset, needed: 4 })
        .map(|value| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
        .map_err(|_| ObjectError::InvalidField { field, value: 0 })
}

fn read_u64(bytes: &[u8], offset: usize, field: &'static str) -> Result<u64, ObjectError> {
    let end = offset
        .checked_add(8)
        .ok_or(ObjectError::InvalidStructure("Mach-O field overflow"))?;
    bytes
        .get(offset..end)
        .ok_or(ObjectError::Truncated { offset, needed: 8 })
        .and_then(|value| {
            let Ok(array) = <[u8; 8]>::try_from(value) else {
                return Err(ObjectError::Truncated { offset, needed: 8 });
            };
            Ok(u64::from_le_bytes(array))
        })
        .map_err(|_| ObjectError::InvalidField { field, value: 0 })
}

/// Stateless Mach-O validator.
#[derive(Clone, Copy, Debug, Default)]
pub struct MachReader;

impl MachReader {
    /// Validates a 64-bit Mach-O object for the requested architecture.
    ///
    /// # Errors
    ///
    /// Returns an error when the Mach-O header or load commands are malformed.
    pub fn validate(bytes: &[u8], architecture: MachArchitecture) -> Result<(), ObjectError> {
        validate_macho(bytes, architecture)
    }
}

fn validate_input(object: &MachObject) -> Result<(), ObjectError> {
    for section in &object.sections {
        if section.name.len() > 16 || section.segment.len() > 16 {
            return Err(ObjectError::InvalidName);
        }
    }
    for relocation in &object.relocations {
        if !object
            .sections
            .iter()
            .any(|section| section.id == relocation.section)
        {
            return Err(ObjectError::InvalidReference {
                kind: "section",
                index: relocation.section.0 as usize,
            });
        }
        let supported = matches!(
            relocation.kind,
            crate::RelocKind::Abs64
                | crate::RelocKind::PcRel32
                | crate::RelocKind::Plt32
                | crate::RelocKind::Branch26
                | crate::RelocKind::Adrp21
                | crate::RelocKind::Add12
        );
        if !supported {
            return Err(ObjectError::UnsupportedRelocation(relocation.kind));
        }
    }
    Ok(())
}
