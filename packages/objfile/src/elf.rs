use crate::{ObjectError, RelocKind, Relocation, Section, SectionId, SymbolRef};

/// ELF machine architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfArchitecture {
    /// x86-64 ELF machine 62.
    X86_64,
    /// `AArch64` ELF machine 183.
    Aarch64,
}

/// ELF section category used by the object writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfSectionKind {
    /// Executable code.
    Text,
    /// Read-only data.
    Rodata,
    /// NCL metadata.
    Metadata,
}

/// A section supplied to the ELF writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElfSection {
    /// Generic section identifier.
    pub id: SectionId,
    /// Section category.
    pub kind: ElfSectionKind,
    /// Section payload.
    pub bytes: Vec<u8>,
}

/// A symbol supplied to the ELF writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElfSymbol {
    /// Symbol name.
    pub name: String,
    /// Defining section, or `None` for undefined symbols.
    pub section: Option<SectionId>,
    /// Symbol value.
    pub value: u64,
    /// Whether the symbol is global.
    pub global: bool,
}

/// A relocatable ELF64 object description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElfObject {
    /// Target machine.
    pub architecture: ElfArchitecture,
    /// Payload sections.
    pub sections: Vec<ElfSection>,
    /// Relocation records.
    pub relocations: Vec<Relocation>,
    /// Local and global symbols.
    pub symbols: Vec<ElfSymbol>,
}

impl ElfObject {
    /// Writes a relocatable ELF64 object containing the NCL sections and relocations.
    ///
    /// # Errors
    ///
    /// Returns an error when a section, symbol, or relocation cannot be represented.
    pub fn write(&self) -> Result<Vec<u8>, ObjectError> {
        validate_input(self)?;
        let machine = match self.architecture {
            ElfArchitecture::X86_64 => 62u16,
            ElfArchitecture::Aarch64 => 183,
        };
        let names = [
            "",
            ".text",
            ".rodata",
            ".ncl",
            ".rela.text",
            ".rela.ncl",
            ".symtab",
            ".strtab",
            ".shstrtab",
        ];
        let mut shstr = vec![0];
        let mut name_offsets = Vec::new();
        for name in names.iter().skip(1) {
            name_offsets.push(u32::try_from(shstr.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "section name table size",
                    value: u64::MAX,
                }
            })?);
            shstr.extend_from_slice(name.as_bytes());
            shstr.push(0);
        }
        let mut strtab = vec![0];
        let mut symbol_names = Vec::new();
        for symbol in &self.symbols {
            symbol_names.push(u32::try_from(strtab.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "string table size",
                    value: u64::MAX,
                }
            })?);
            strtab.extend_from_slice(symbol.name.as_bytes());
            strtab.push(0);
        }
        let mut out = vec![0; 64];
        let mut ranges = [(0u64, 0u64); 9];
        for (index, kind) in [
            ElfSectionKind::Text,
            ElfSectionKind::Rodata,
            ElfSectionKind::Metadata,
        ]
        .iter()
        .enumerate()
        {
            if let Some(section) = self.sections.iter().find(|section| section.kind == *kind) {
                align(&mut out, 8);
                ranges[index + 1] = (out.len() as u64, section.bytes.len() as u64);
                out.extend_from_slice(&section.bytes);
            }
        }
        let text_id = self
            .sections
            .iter()
            .find(|s| s.kind == ElfSectionKind::Text)
            .map(|s| s.id);
        let ncl_id = self
            .sections
            .iter()
            .find(|s| s.kind == ElfSectionKind::Metadata)
            .map(|s| s.id);
        let (rela_text, rela_ncl) = (
            encode_rela(self, text_id, machine)?,
            encode_rela(self, ncl_id, machine)?,
        );
        align(&mut out, 8);
        ranges[4] = (out.len() as u64, rela_text.len() as u64);
        out.extend_from_slice(&rela_text);
        align(&mut out, 8);
        ranges[5] = (out.len() as u64, rela_ncl.len() as u64);
        out.extend_from_slice(&rela_ncl);
        let mut symtab = vec![0; 24];
        for (index, symbol) in self.symbols.iter().enumerate() {
            let mut entry = [0; 24];
            entry[0..4].copy_from_slice(&symbol_names[index].to_le_bytes());
            entry[4] = if symbol.global { 0x10 } else { 0 };
            entry[5] = symbol_section(self, symbol);
            entry[6..8].copy_from_slice(&0u16.to_le_bytes());
            entry[8..16].copy_from_slice(&symbol.value.to_le_bytes());
            symtab.extend_from_slice(&entry);
        }
        align(&mut out, 8);
        ranges[6] = (out.len() as u64, symtab.len() as u64);
        out.extend_from_slice(&symtab);
        ranges[7] = (out.len() as u64, strtab.len() as u64);
        out.extend_from_slice(&strtab);
        ranges[8] = (out.len() as u64, shstr.len() as u64);
        out.extend_from_slice(&shstr);
        align(&mut out, 8);
        let shoff = out.len() as u64;
        out.extend_from_slice(&[0; 64]);
        for index in 1..9 {
            let mut sh = [0; 64];
            sh[0..4].copy_from_slice(&name_offsets[index - 1].to_le_bytes());
            let (ty, flags, link, info, align_value, entsize): (u32, u64, u32, u32, u64, u64) =
                match index {
                    1 => (1, 6, 0, 0, 16, 0),
                    2 => (1, 2, 0, 0, 1, 0),
                    3 => (1, 0, 0, 0, 1, 0),
                    4 | 5 => (4, 0, 6, if index == 4 { 1 } else { 3 }, 8, 24),
                    6 => (
                        2,
                        0,
                        7,
                        u32::try_from(self.symbols.len() + 1).map_err(|_| {
                            ObjectError::InvalidField {
                                field: "symbol count",
                                value: u64::MAX,
                            }
                        })?,
                        8,
                        24,
                    ),
                    7 | 8 => (3, 0, 0, 0, 1, 0),
                    _ => (0, 0, 0, 0, 1, 0),
                };
            sh[4..8].copy_from_slice(&ty.to_le_bytes());
            sh[8..16].copy_from_slice(&flags.to_le_bytes());
            sh[24..32].copy_from_slice(&ranges[index].0.to_le_bytes());
            sh[32..40].copy_from_slice(&ranges[index].1.to_le_bytes());
            sh[40..44].copy_from_slice(&link.to_le_bytes());
            sh[44..48].copy_from_slice(&info.to_le_bytes());
            sh[48..56].copy_from_slice(&align_value.to_le_bytes());
            sh[56..64].copy_from_slice(&entsize.to_le_bytes());
            out.extend_from_slice(&sh);
        }
        out[0..4].copy_from_slice(b"\x7fELF");
        out[4] = 2;
        out[5] = 1;
        out[6] = 1;
        out[16..18].copy_from_slice(&1u16.to_le_bytes());
        out[18..20].copy_from_slice(&machine.to_le_bytes());
        out[20..24].copy_from_slice(&1u32.to_le_bytes());
        out[40..48].copy_from_slice(&shoff.to_le_bytes());
        out[58..60].copy_from_slice(&64u16.to_le_bytes());
        out[60..62].copy_from_slice(&9u16.to_le_bytes());
        out[62..64].copy_from_slice(&8u16.to_le_bytes());
        Ok(out)
    }
}

/// Validates an ELF64 relocatable object header and section table.
///
/// # Errors
///
/// Returns an error when the header, machine, or section table is malformed.
pub fn validate_elf(bytes: &[u8], architecture: ElfArchitecture) -> Result<(), ObjectError> {
    if bytes.len() < 64 {
        return Err(ObjectError::Truncated {
            offset: bytes.len(),
            needed: 64,
        });
    }
    if &bytes[..4] != b"\x7fELF" || bytes[4] != 2 || bytes[5] != 1 || bytes[6] != 1 {
        return Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 file",
        ));
    }
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    let expected = match architecture {
        ElfArchitecture::X86_64 => 62,
        ElfArchitecture::Aarch64 => 183,
    };
    if machine != expected {
        return Err(ObjectError::InvalidField {
            field: "ELF machine",
            value: u64::from(machine),
        });
    }
    if u16::from_le_bytes([bytes[58], bytes[59]]) != 64
        || u16::from_le_bytes([bytes[60], bytes[61]]) != 9
    {
        return Err(ObjectError::InvalidStructure("invalid ELF section table"));
    }
    let shoff = usize::try_from(u64::from_le_bytes(bytes[40..48].try_into().map_err(
        |_| ObjectError::Truncated {
            offset: 40,
            needed: 8,
        },
    )?))
    .map_err(|_| ObjectError::InvalidField {
        field: "section offset",
        value: u64::MAX,
    })?;
    let table_size = 9usize
        .checked_mul(64)
        .ok_or(ObjectError::InvalidStructure("section table overflow"))?;
    if shoff
        .checked_add(table_size)
        .as_ref()
        .is_none_or(|end| *end > bytes.len())
    {
        return Err(ObjectError::OutOfBounds {
            section: "ELF section table",
            offset: shoff as u64,
            size: table_size as u64,
        });
    }
    Ok(())
}

/// Stateless ELF validator.
#[derive(Clone, Copy, Debug, Default)]
pub struct ElfReader;

impl ElfReader {
    /// Validates an ELF object for the requested architecture.
    ///
    /// # Errors
    ///
    /// Returns an error when the object is malformed or targets another machine.
    pub fn validate(bytes: &[u8], architecture: ElfArchitecture) -> Result<(), ObjectError> {
        validate_elf(bytes, architecture)
    }
}

fn validate_input(object: &ElfObject) -> Result<(), ObjectError> {
    for section in &object.sections {
        if section.bytes.len() > u32::MAX as usize {
            return Err(ObjectError::InvalidField {
                field: "section size",
                value: section.bytes.len() as u64,
            });
        }
    }
    for relocation in &object.relocations {
        let section = object
            .sections
            .iter()
            .find(|section| section.id == relocation.section)
            .ok_or(ObjectError::InvalidReference {
                kind: "section",
                index: relocation.section.0 as usize,
            })?;
        if usize::try_from(relocation.offset).map_or(true, |offset| offset >= section.bytes.len())
            && !section.bytes.is_empty()
        {
            return Err(ObjectError::OutOfBounds {
                section: "relocation",
                offset: u64::from(relocation.offset),
                size: 1,
            });
        }
        if let SymbolRef::Local(index) = relocation.symbol
            && usize::try_from(index).map_or(true, |i| i >= object.symbols.len())
        {
            return Err(ObjectError::InvalidReference {
                kind: "symbol",
                index: index as usize,
            });
        }
    }
    Ok(())
}

fn encode_rela(
    object: &ElfObject,
    target: Option<SectionId>,
    machine: u16,
) -> Result<Vec<u8>, ObjectError> {
    let mut out = Vec::new();
    for relocation in object
        .relocations
        .iter()
        .filter(|r| Some(r.section) == target)
    {
        let typ = elf_type(relocation.kind, machine)?;
        let symbol = match relocation.symbol {
            SymbolRef::Local(index) => index + 1,
            SymbolRef::External(_) => 0,
        };
        let info = (u64::from(symbol) << 32) | u64::from(typ);
        out.extend_from_slice(&u64::from(relocation.offset).to_le_bytes());
        out.extend_from_slice(&info.to_le_bytes());
        out.extend_from_slice(&relocation.addend.to_le_bytes());
    }
    Ok(out)
}
const fn elf_type(kind: RelocKind, machine: u16) -> Result<u32, ObjectError> {
    match (machine, kind) {
        (62, RelocKind::Abs64 | RelocKind::CodeEntry | RelocKind::ExternalSymbol) => Ok(1),
        (62, RelocKind::PcRel32) => Ok(2),
        (62, RelocKind::Plt32) => Ok(4),
        (183, RelocKind::Abs64 | RelocKind::CodeEntry | RelocKind::ExternalSymbol) => Ok(257),
        (183, RelocKind::Branch26) => Ok(283),
        (183, RelocKind::Adrp21) => Ok(275),
        (183, RelocKind::Add12) => Ok(277),
        (183, RelocKind::CondBranch19) => Ok(280),
        _ => Err(ObjectError::UnsupportedRelocation(kind)),
    }
}
fn symbol_section(object: &ElfObject, symbol: &ElfSymbol) -> u8 {
    symbol
        .section
        .and_then(|id| object.sections.iter().position(|s| s.id == id))
        .map_or(0, |index| u8::try_from(index + 1).unwrap_or(0))
}
fn align(bytes: &mut Vec<u8>, alignment: usize) {
    let padding = (alignment - bytes.len() % alignment) % alignment;
    bytes.resize(bytes.len() + padding, 0);
}

/// Converts generic sections into ELF section descriptions.
#[must_use]
pub fn sections_from_generic(sections: &[Section]) -> Vec<ElfSection> {
    sections
        .iter()
        .map(|section| ElfSection {
            id: section.id,
            kind: match section.name.as_str() {
                ".text" => ElfSectionKind::Text,
                ".ncl" => ElfSectionKind::Metadata,
                _ => ElfSectionKind::Rodata,
            },
            bytes: section.bytes.clone(),
        })
        .collect()
}
