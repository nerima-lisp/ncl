use crate::{Architecture, MachArchitecture, ObjectError};

/// Validates an ELF executable envelope and requires an NCL metadata load segment.
///
/// # Errors
///
/// Returns an error when the input is not an executable for the requested
/// architecture, lacks RX and RW load segments, or has no metadata payload.
pub fn validate_elf_executable(
    bytes: &[u8],
    architecture: Architecture,
) -> Result<(), ObjectError> {
    validate_elf_executable_header(bytes, architecture)?;
    let entry =
        u64::from_le_bytes(
            bytes[24..32]
                .try_into()
                .map_err(|_| ObjectError::Truncated {
                    offset: 24,
                    needed: 8,
                })?,
        );
    let phoff = usize::try_from(u64::from_le_bytes(bytes[32..40].try_into().map_err(
        |_| ObjectError::Truncated {
            offset: 32,
            needed: 8,
        },
    )?))
    .map_err(|_| ObjectError::InvalidField {
        field: "program header offset",
        value: u64::MAX,
    })?;
    let phentsize = usize::from(u16::from_le_bytes([bytes[54], bytes[55]]));
    let phnum = usize::from(u16::from_le_bytes([bytes[56], bytes[57]]));
    if phentsize != 56 || phnum < 2 {
        return Err(ObjectError::InvalidStructure("invalid ELF program headers"));
    }
    let table_size = phentsize
        .checked_mul(phnum)
        .ok_or(ObjectError::InvalidStructure(
            "program header table overflow",
        ))?;
    if phoff
        .checked_add(table_size)
        .is_none_or(|end| end > bytes.len())
    {
        return Err(ObjectError::OutOfBounds {
            section: "ELF program headers",
            offset: u64::try_from(phoff).unwrap_or(u64::MAX),
            size: u64::try_from(table_size).unwrap_or(u64::MAX),
        });
    }
    let (has_text, has_metadata) = validate_elf_segments(bytes, phoff, phentsize, phnum, entry)?;
    if !has_text || !has_metadata {
        return Err(ObjectError::InvalidStructure(
            "missing NCL executable segments",
        ));
    }
    Ok(())
}

fn validate_elf_executable_header(
    bytes: &[u8],
    architecture: Architecture,
) -> Result<(), ObjectError> {
    if bytes.len() < 64 || &bytes[..4] != b"\x7fELF" || bytes[4] != 2 || bytes[5] != 1 {
        return Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 executable",
        ));
    }
    if u16::from_le_bytes([bytes[16], bytes[17]]) != 2 {
        return Err(ObjectError::InvalidStructure("not an ELF executable"));
    }
    let expected = match architecture {
        Architecture::X86_64 => 62,
        Architecture::Aarch64 => 183,
    };
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    if machine != expected {
        return Err(ObjectError::InvalidField {
            field: "ELF machine",
            value: u64::from(machine),
        });
    }
    Ok(())
}

fn validate_elf_segments(
    bytes: &[u8],
    phoff: usize,
    phentsize: usize,
    phnum: usize,
    entry: u64,
) -> Result<(bool, bool), ObjectError> {
    let mut has_text = false;
    let mut has_metadata = false;
    for index in 0..phnum {
        let at = phoff + index * phentsize;
        let kind = u32::from_le_bytes(bytes[at..at + 4].try_into().map_err(|_| {
            ObjectError::Truncated {
                offset: at,
                needed: 4,
            }
        })?);
        if kind != 1 {
            continue;
        }
        let flags = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().map_err(|_| {
            ObjectError::Truncated {
                offset: at + 4,
                needed: 4,
            }
        })?);
        let offset = usize::try_from(u64::from_le_bytes(
            bytes[at + 8..at + 16]
                .try_into()
                .map_err(|_| ObjectError::Truncated {
                    offset: at + 8,
                    needed: 8,
                })?,
        ))
        .map_err(|_| ObjectError::InvalidField {
            field: "ELF segment offset",
            value: u64::MAX,
        })?;
        let file_size = usize::try_from(u64::from_le_bytes(
            bytes[at + 32..at + 40]
                .try_into()
                .map_err(|_| ObjectError::Truncated {
                    offset: at + 32,
                    needed: 8,
                })?,
        ))
        .map_err(|_| ObjectError::InvalidField {
            field: "ELF segment size",
            value: u64::MAX,
        })?;
        if offset
            .checked_add(file_size)
            .is_none_or(|end| end > bytes.len())
        {
            return Err(ObjectError::OutOfBounds {
                section: "ELF load segment",
                offset: u64::try_from(offset).unwrap_or(u64::MAX),
                size: u64::try_from(file_size).unwrap_or(u64::MAX),
            });
        }
        let address = u64::from_le_bytes(bytes[at + 16..at + 24].try_into().map_err(|_| {
            ObjectError::Truncated {
                offset: at + 16,
                needed: 8,
            }
        })?);
        if flags & 1 != 0 && flags & 4 != 0 {
            has_text = true;
            if entry < address || entry - address >= u64::try_from(file_size).unwrap_or(u64::MAX) {
                return Err(ObjectError::InvalidStructure(
                    "ELF entry is outside executable segment",
                ));
            }
        }
        if flags & 2 != 0 && flags & 4 != 0 && file_size != 0 {
            has_metadata = true;
        }
    }
    Ok((has_text, has_metadata))
}

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
///
/// # Errors
///
/// Returns an error when the image size cannot be represented in the ELF envelope.
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
    out[24..32].copy_from_slice(&(0x0040_0000_u64 + text_offset as u64).to_le_bytes());
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
        0x0040_0000 + text_offset as u64,
        image.code.len() as u64,
    );
    let data = phoff + 56;
    write_phdr(
        &mut out[data..data + 56],
        1,
        6,
        data_offset as u64,
        0x0040_0000 + data_offset as u64,
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
///
/// # Errors
///
/// Returns an error when the image architecture or size is incompatible with Mach-O.
pub fn write_mach_executable(
    image: &ExecutableImage,
    architecture: MachArchitecture,
) -> Result<Vec<u8>, ObjectError> {
    validate_executable_image(image, architecture)?;
    let (mut out, layout) = executable_layout(image)?;
    write_mach_header(&mut out, architecture, layout.commands)?;
    write_exec_segment(
        &mut out,
        &ExecSegment {
            at: 32,
            segment: "__TEXT",
            vmaddr: 0x1_0000_0000,
            fileoff: 0,
            vmsize: 0x1000,
            filesize: u64::try_from(layout.code_end).unwrap_or(u64::MAX),
            maxprot: 7,
            initprot: 5,
            section_offset: layout.code_offset,
            section_size: image.code.len(),
            section_name: "__text",
            section_segment: "__TEXT",
        },
    )?;
    write_exec_segment(
        &mut out,
        &ExecSegment {
            at: 32 + layout.segment_size,
            segment: "__DATA",
            vmaddr: 0x1_0000_0000 + u64::try_from(layout.data_offset).unwrap_or(u64::MAX),
            fileoff: u64::try_from(layout.data_offset).unwrap_or(u64::MAX),
            vmsize: 0x1000,
            filesize: layout.metadata_size,
            maxprot: 3,
            initprot: 1,
            section_offset: layout.data_offset,
            section_size: image.metadata.len(),
            section_name: "__ncl",
            section_segment: "__DATA",
        },
    )?;
    write_mach_tail(&mut out, &layout, image);
    Ok(out)
}

fn validate_executable_image(
    image: &ExecutableImage,
    architecture: MachArchitecture,
) -> Result<(), ObjectError> {
    let expected = match architecture {
        MachArchitecture::X86_64 => Architecture::X86_64,
        MachArchitecture::Arm64 => Architecture::Aarch64,
    };
    if image.architecture != expected {
        return Err(ObjectError::InvalidField {
            field: "architecture",
            value: u64::from(image.architecture as u8),
        });
    }
    Ok(())
}

struct ExecutableLayout {
    commands: usize,
    segment_size: usize,
    code_offset: usize,
    code_end: usize,
    data_offset: usize,
    metadata_size: u64,
}

fn executable_layout(image: &ExecutableImage) -> Result<(Vec<u8>, ExecutableLayout), ObjectError> {
    let header = 32usize;
    let segment_size = 72usize + 80;
    let dylinker_size = 32usize;
    let build_size = 24usize;
    let main_size = 24usize;
    let commands = segment_size * 2 + dylinker_size + build_size + main_size;
    let code_offset = header
        .checked_add(commands)
        .ok_or(ObjectError::InvalidStructure("Mach-O command overflow"))?;
    let code_end = code_offset
        .checked_add(image.code.len())
        .ok_or(ObjectError::InvalidStructure("executable size overflow"))?;
    let data_offset = (code_end + 0x0fff) & !0x0fff;
    let out = vec![
        0;
        data_offset
            .checked_add(image.metadata.len())
            .ok_or(ObjectError::InvalidStructure("executable size overflow"))?
    ];
    Ok((
        out,
        ExecutableLayout {
            commands,
            segment_size,
            code_offset,
            code_end,
            data_offset,
            metadata_size: u64::try_from(image.metadata.len()).map_err(|_| {
                ObjectError::InvalidField {
                    field: "Mach-O metadata size",
                    value: u64::MAX,
                }
            })?,
        },
    ))
}

fn write_mach_header(
    out: &mut [u8],
    architecture: MachArchitecture,
    commands: usize,
) -> Result<(), ObjectError> {
    let cpu = match architecture {
        MachArchitecture::X86_64 => 0x0100_0007_u32,
        MachArchitecture::Arm64 => 0x0100_000c_u32,
    };
    out[0..4].copy_from_slice(&0xfeed_facf_u32.to_le_bytes());
    out[4..8].copy_from_slice(&cpu.to_le_bytes());
    out[12..16].copy_from_slice(&2u32.to_le_bytes());
    out[16..20].copy_from_slice(&5u32.to_le_bytes());
    out[20..24].copy_from_slice(
        &u32::try_from(commands)
            .map_err(|_| ObjectError::InvalidField {
                field: "Mach-O command size",
                value: u64::MAX,
            })?
            .to_le_bytes(),
    );
    Ok(())
}

fn write_mach_tail(out: &mut [u8], layout: &ExecutableLayout, image: &ExecutableImage) {
    let dylinker_at = 32 + layout.segment_size * 2;
    write_dylinker(out, dylinker_at);
    let build_at = dylinker_at + 32;
    write_build_version(out, build_at);
    let main_at = build_at + 24;
    out[main_at..main_at + 4].copy_from_slice(&0x8000_0028_u32.to_le_bytes());
    out[main_at + 4..main_at + 8].copy_from_slice(&24u32.to_le_bytes());
    out[main_at + 8..main_at + 16].copy_from_slice(
        &u64::try_from(layout.code_offset)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    out[layout.code_offset..layout.code_end].copy_from_slice(&image.code);
    out[layout.data_offset..].copy_from_slice(&image.metadata);
}

fn write_build_version(out: &mut [u8], at: usize) {
    out[at..at + 4].copy_from_slice(&0x32u32.to_le_bytes());
    out[at + 4..at + 8].copy_from_slice(&24u32.to_le_bytes());
    out[at + 8..at + 12].copy_from_slice(&1u32.to_le_bytes());
    out[at + 12..at + 16].copy_from_slice(&0x000d_0000u32.to_le_bytes());
    out[at + 16..at + 20].copy_from_slice(&0x000d_0000u32.to_le_bytes());
}

fn write_dylinker(out: &mut [u8], at: usize) {
    out[at..at + 4].copy_from_slice(&0x0eu32.to_le_bytes());
    out[at + 4..at + 8].copy_from_slice(&32u32.to_le_bytes());
    out[at + 8..at + 12].copy_from_slice(&12u32.to_le_bytes());
    out[at + 12..at + 12 + 14].copy_from_slice(b"/usr/lib/dyld\0");
}

struct ExecSegment<'a> {
    at: usize,
    segment: &'a str,
    vmaddr: u64,
    fileoff: u64,
    vmsize: u64,
    filesize: u64,
    maxprot: u32,
    initprot: u32,
    section_offset: usize,
    section_size: usize,
    section_name: &'a str,
    section_segment: &'a str,
}

fn write_exec_segment(out: &mut [u8], segment: &ExecSegment<'_>) -> Result<(), ObjectError> {
    let command_size = 152u32;
    let at = segment.at;
    out[at..at + 4].copy_from_slice(&0x19u32.to_le_bytes());
    out[at + 4..at + 8].copy_from_slice(&command_size.to_le_bytes());
    write_name(out, at + 8, segment.segment);
    out[at + 24..at + 32].copy_from_slice(&segment.vmaddr.to_le_bytes());
    out[at + 32..at + 40].copy_from_slice(&segment.vmsize.to_le_bytes());
    out[at + 40..at + 48].copy_from_slice(&segment.fileoff.to_le_bytes());
    out[at + 48..at + 56].copy_from_slice(&segment.filesize.to_le_bytes());
    out[at + 56..at + 60].copy_from_slice(&segment.maxprot.to_le_bytes());
    out[at + 60..at + 64].copy_from_slice(&segment.initprot.to_le_bytes());
    out[at + 64..at + 68].copy_from_slice(&1u32.to_le_bytes());
    let section_at = at + 72;
    write_section(
        out,
        section_at,
        segment.section_name,
        segment.section_segment,
        u32::try_from(segment.section_offset).map_err(|_| ObjectError::InvalidField {
            field: "Mach-O section offset",
            value: u64::MAX,
        })?,
        u64::try_from(segment.section_size).map_err(|_| ObjectError::InvalidField {
            field: "Mach-O section size",
            value: u64::MAX,
        })?,
        segment.vmaddr
            + if segment.fileoff == 0 {
                u64::try_from(segment.section_offset).map_err(|_| ObjectError::InvalidField {
                    field: "Mach-O section address",
                    value: u64::MAX,
                })?
            } else {
                0
            },
    );
    Ok(())
}

fn write_name(out: &mut [u8], at: usize, name: &str) {
    out[at..at + name.len()].copy_from_slice(name.as_bytes());
}
fn write_section(
    out: &mut [u8],
    at: usize,
    name: &str,
    segment: &str,
    offset: u32,
    size: u64,
    address: u64,
) {
    write_name(out, at, name);
    write_name(out, at + 16, segment);
    out[at + 32..at + 40].copy_from_slice(&address.to_le_bytes());
    out[at + 40..at + 48].copy_from_slice(&size.to_le_bytes());
    out[at + 48..at + 52].copy_from_slice(&offset.to_le_bytes());
}
