use super::*;

#[test]
fn private_elf_segment_parser_skips_non_load_segments() {
    let mut bytes = vec![0; 56];
    bytes[0..4].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        validate_elf_segments(&bytes, 0, 56, 1, 0),
        Ok((false, false))
    );
}

#[test]
fn private_elf_header_parser_rejects_truncated_and_wrong_class_inputs() {
    assert_eq!(
        validate_elf_executable_header(&[0; 63], Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "not a little-endian ELF64 executable",
        ))
    );
    let mut bytes = vec![0; 64];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable_header(&bytes, Architecture::X86_64),
        Ok(())
    );
    bytes[16..18].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        validate_elf_executable_header(&bytes, Architecture::X86_64),
        Err(ObjectError::InvalidStructure("not an ELF executable"))
    );
}

#[test]
fn private_mach_writer_reports_unrepresentable_fields() {
    let mut header = [0; 24];
    assert_eq!(
        write_mach_header(&mut header, MachArchitecture::X86_64, usize::MAX),
        Err(ObjectError::InvalidField {
            field: "Mach-O command size",
            value: u64::MAX,
        })
    );

    let mut out = vec![0; 300];
    let segment = ExecSegment {
        at: 0,
        segment: "__TEXT",
        vmaddr: 0,
        fileoff: 1,
        vmsize: 0,
        filesize: 0,
        maxprot: 0,
        initprot: 0,
        section_offset: (u32::MAX as usize) + 1,
        section_size: 0,
        section_name: "__text",
        section_segment: "__TEXT",
    };
    assert_eq!(
        write_exec_segment(&mut out, &segment),
        Err(ObjectError::InvalidField {
            field: "Mach-O section offset",
            value: u64::MAX,
        })
    );
}

#[test]
fn private_executable_helpers_accept_valid_layouts() {
    let image = ExecutableImage {
        architecture: Architecture::X86_64,
        code: vec![0xc3],
        metadata: vec![1, 2],
    };
    let layout_result = executable_layout(&image);
    assert!(layout_result.is_ok());
    let Ok((mut output, layout)) = layout_result else {
        return;
    };
    let header_result = write_mach_header(&mut output, MachArchitecture::X86_64, layout.commands);
    assert!(header_result.is_ok());
    let segment = ExecSegment {
        at: 32,
        segment: "__TEXT",
        vmaddr: 0x1_0000_0000,
        fileoff: 0,
        vmsize: layout.data_offset as u64,
        filesize: layout.code_end as u64,
        maxprot: 7,
        initprot: 5,
        section_offset: layout.code_offset,
        section_size: image.code.len(),
        section_name: "__text",
        section_segment: "__TEXT",
    };
    let segment_result = write_exec_segment(&mut output, &segment);
    assert!(segment_result.is_ok());
    assert_eq!(output[0..4], 0xfeed_facf_u32.to_le_bytes());
    let elf_result = write_elf_executable(&image);
    assert!(elf_result.is_ok());
    let Ok(elf) = elf_result else { return };
    let phoff = 64;
    assert_eq!(
        validate_elf_segments(&elf, phoff, 56, 2, 0x0040_1000),
        Ok((true, true))
    );
}
