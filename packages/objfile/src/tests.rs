use crate::*;

#[test]
fn fasl_round_trip_and_header_validation() {
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 3,
        },
        sections: FaslSection {
            code: vec![0xc3],
            relocations: vec![Relocation {
                section: SectionId(0),
                offset: 0,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::Local(0),
                addend: 4,
            }],
            constants: vec![1, 2],
            symbols: vec![3],
            stack_maps: vec![4],
            debug: vec![5],
        },
    };
    let bytes = FaslWriter::write(&value);
    assert!(bytes.is_ok());
    let bytes = bytes.unwrap_or_default();
    let read = FaslReader::read(&bytes, Architecture::X86_64, 3);
    assert!(read.is_ok());
    let read = read.unwrap_or_else(|_| Fasl {
        header: value.header,
        sections: value.sections.clone(),
    });
    assert_eq!(read.sections.code, value.sections.code);
    assert_eq!(read.sections.relocations, value.sections.relocations);
    assert_eq!(read.sections.debug, value.sections.debug);
    assert!(FaslReader::read(&bytes, Architecture::Aarch64, 3).is_err());
    assert!(FaslReader::read(&bytes[..63], Architecture::X86_64, 3).is_err());
}

#[test]
fn native_object_magic_and_relocations() {
    let sections = vec![
        ElfSection {
            id: SectionId(1),
            kind: ElfSectionKind::Text,
            bytes: vec![0xc3],
        },
        ElfSection {
            id: SectionId(2),
            kind: ElfSectionKind::Metadata,
            bytes: vec![1],
        },
    ];
    let elf = (ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections,
        relocations: vec![],
        symbols: vec![],
    })
    .write();
    assert!(elf.is_ok());
    let elf = elf.unwrap_or_else(|_| Vec::new());
    assert_eq!(&elf[..4], b"\x7fELF");
    let macho = (MachObject {
        architecture: MachArchitecture::Arm64,
        sections: vec![MachSection {
            id: SectionId(1),
            segment: "__TEXT".into(),
            name: "__text".into(),
            bytes: vec![0xc0, 0x03, 0x5f, 0xd6],
        }],
        relocations: vec![],
    })
    .write();
    assert!(macho.is_ok());
    let macho = macho.unwrap_or_else(|_| Vec::new());
    assert!(validate_macho(&macho, MachArchitecture::Arm64).is_ok());
}

#[test]
fn executable_envelopes_have_expected_headers() {
    let image = ExecutableImage {
        architecture: Architecture::X86_64,
        code: vec![0xc3],
        metadata: b"NCL\0".to_vec(),
    };
    let elf = write_elf_executable(&image).unwrap_or_default();
    assert_eq!(&elf[..4], b"\x7fELF");
    let macho = write_mach_executable(&image, MachArchitecture::X86_64).unwrap_or_default();
    assert!(validate_macho(&macho, MachArchitecture::X86_64).is_ok());
}

#[test]
fn fasl_rejects_bad_offsets_and_reserved_bits() {
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: vec![1],
            relocations: vec![],
            constants: vec![],
            symbols: vec![],
            stack_maps: vec![],
            debug: vec![],
        },
    };
    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[14] = 1;
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());
    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());
}
