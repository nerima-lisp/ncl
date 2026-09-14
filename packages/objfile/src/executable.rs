use crate::{Architecture, MachArchitecture, ObjectError};

/// Input for a minimal NCL executable image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableImage {
    /// Target architecture.
    pub architecture: Architecture,
    /// Entry-point machine code.
    pub code: Vec<u8>,
    /// NCL loader metadata.
    pub metadata: Vec<u8>,
}

/// Writes a minimal ELF executable image with RX and RW load segments.
pub fn write_elf_executable(image: &ExecutableImage) -> Result<Vec<u8>, ObjectError> {
    if image.architecture != Architecture::X86_64 && image.architecture != Architecture::Aarch64 {
        return Err(ObjectError::InvalidField {
            field: "architecture",
            value: 0,
        });
    }
    let phoff = 64usize;
    let text_offset = 0x1000usize;
    let code_end = text_offset
        .checked_add(image.code.len())
        .ok_or(ObjectError::InvalidStructure("executable size overflow"))?;
    let data_offset = (code_end + 0x0fff) & !0x0fff;
    let mut out = vec![
        0;
        data_offset
            .checked_add(image.metadata.len())
            .ok_or(ObjectError::InvalidStructure("executable size overflow"))?
    ];
    out[0..4].copy_from_slice(b"\x7fELF");
    out[4] = 2;
    out[5] = 1;
    out[6] = 1;
    out[16..18].copy_from_slice(&2u16.to_le_bytes());
    out[18..20].copy_from_slice(
        &(if image.architecture == Architecture::X86_64 {
            62u16
        } else {
            183u16
        })
        .to_le_bytes(),
    );
    out[20..24].copy_from_slice(&1u32.to_le_bytes());
    out[24..32].copy_from_slice(&(0x400000u64 + text_offset as u64).to_le_bytes());
    out[32..40].copy_from_slice(&(phoff as u64).to_le_bytes());
    out[52..54].copy_from_slice(&64u16.to_le_bytes());
    out[54..56].copy_from_slice(&56u16.to_le_bytes());
    out[56..58].copy_from_slice(&2u16.to_le_bytes());
    let text = phoff;
    write_phdr(
        &mut out[text..text + 56],
        1,
        5,
        text_offset as u64,
        0x400000 + text_offset as u64,
        image.code.len() as u64,
    );
    let data = phoff + 56;
    write_phdr(
        &mut out[data..data + 56],
        1,
        6,
        data_offset as u64,
        0x400000 + data_offset as u64,
        image.metadata.len() as u64,
    );
    out[text_offset..text_offset + image.code.len()].copy_from_slice(&image.code);
    out[data_offset..data_offset + image.metadata.len()].copy_from_slice(&image.metadata);
    Ok(out)
}

fn write_phdr(out: &mut [u8], kind: u32, flags: u32, offset: u64, address: u64, size: u64) {
    out[0..4].copy_from_slice(&kind.to_le_bytes());
    out[4..8].copy_from_slice(&flags.to_le_bytes());
    out[8..16].copy_from_slice(&offset.to_le_bytes());
    out[16..24].copy_from_slice(&address.to_le_bytes());
    out[24..32].copy_from_slice(&address.to_le_bytes());
    out[32..40].copy_from_slice(&size.to_le_bytes());
    out[40..48].copy_from_slice(&size.to_le_bytes());
    out[48..56].copy_from_slice(&0x1000u64.to_le_bytes());
}

/// Writes a Mach-O executable envelope containing NCL metadata.
pub fn write_mach_executable(
    image: &ExecutableImage,
    architecture: MachArchitecture,
) -> Result<Vec<u8>, ObjectError> {
    let expected = match architecture {
        MachArchitecture::X86_64 => Architecture::X86_64,
        MachArchitecture::Arm64 => Architecture::Aarch64,
    };
    if image.architecture != expected {
        return Err(ObjectError::InvalidField {
            field: "architecture",
            value: image.architecture as u8 as u64,
        });
    }
    let header = 32usize;
    let segment = 72usize + 2 * 80;
    let main = 24usize;
    let commands = segment + main;
    let code_offset = header + commands;
    let data_offset = code_offset
        .checked_add(image.code.len())
        .ok_or(ObjectError::InvalidStructure("executable size overflow"))?;
    let mut out = vec![
        0;
        data_offset
            .checked_add(image.metadata.len())
            .ok_or(ObjectError::InvalidStructure("executable size overflow"))?
    ];
    let cpu = match architecture {
        MachArchitecture::X86_64 => 0x01000007u32,
        MachArchitecture::Arm64 => 0x0100000cu32,
    };
    out[0..4].copy_from_slice(&0xfeedfacfu32.to_le_bytes());
    out[4..8].copy_from_slice(&cpu.to_le_bytes());
    out[12..16].copy_from_slice(&2u32.to_le_bytes());
    out[16..20].copy_from_slice(&2u32.to_le_bytes());
    out[20..24].copy_from_slice(&(commands as u32).to_le_bytes());
    out[32..36].copy_from_slice(&0x19u32.to_le_bytes());
    out[36..40].copy_from_slice(&(segment as u32).to_le_bytes());
    write_name(&mut out, 40, "__TEXT");
    out[88..92].copy_from_slice(&7u32.to_le_bytes());
    out[92..96].copy_from_slice(&5u32.to_le_bytes());
    out[96..100].copy_from_slice(&2u32.to_le_bytes());
    write_section(
        &mut out,
        104,
        "__text",
        "__TEXT",
        code_offset as u32,
        image.code.len() as u64,
    );
    write_section(
        &mut out,
        184,
        "__ncl",
        "__TEXT",
        data_offset as u32,
        image.metadata.len() as u64,
    );
    let main_at = 32 + segment;
    out[main_at..main_at + 4].copy_from_slice(&0x80000028u32.to_le_bytes());
    out[main_at + 4..main_at + 8].copy_from_slice(&24u32.to_le_bytes());
    out[main_at + 8..main_at + 16].copy_from_slice(&(code_offset as u64).to_le_bytes());
    out[code_offset..data_offset].copy_from_slice(&image.code);
    out[data_offset..].copy_from_slice(&image.metadata);
    Ok(out)
}

fn write_name(out: &mut [u8], at: usize, name: &str) {
    out[at..at + name.len()].copy_from_slice(name.as_bytes());
}
fn write_section(out: &mut [u8], at: usize, name: &str, segment: &str, offset: u32, size: u64) {
    write_name(out, at, name);
    write_name(out, at + 16, segment);
    out[at + 40..at + 48].copy_from_slice(&size.to_le_bytes());
    out[at + 48..at + 52].copy_from_slice(&offset.to_le_bytes());
}
