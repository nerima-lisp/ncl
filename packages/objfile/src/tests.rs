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
    let validation = validate_macho(&macho, MachArchitecture::Arm64);
    assert!(validation.is_ok(), "{validation:?}");
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
    assert!(validate_elf_executable(&elf, Architecture::X86_64).is_ok());
    let macho = write_mach_executable(&image, MachArchitecture::X86_64).unwrap_or_default();
    assert!(validate_mach_executable(&macho, MachArchitecture::X86_64).is_ok());
    let without_metadata = ExecutableImage {
        architecture: image.architecture,
        code: vec![0xc3],
        metadata: Vec::new(),
    };
    let no_metadata = write_elf_executable(&without_metadata).unwrap_or_default();
    assert!(validate_elf_executable(&no_metadata, Architecture::X86_64).is_err());
    let no_metadata =
        write_mach_executable(&without_metadata, MachArchitecture::X86_64).unwrap_or_default();
    assert!(validate_mach_executable(&no_metadata, MachArchitecture::X86_64).is_err());
}

#[test]
fn seeded_fasl_round_trip() {
    let mut seed = 0x9e37_79b9_u32;
    for iteration in 0..64 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let code_len = usize::try_from(seed % 31 + 1).unwrap_or(1);
        let mut code = Vec::with_capacity(code_len);
        for _ in 0..code_len {
            seed = seed.rotate_left(5).wrapping_add(0x6d2b_79f5);
            code.push(u8::try_from(seed & 0xff).unwrap_or_default());
        }
        let fasl = Fasl {
            header: FaslHeader {
                architecture: if iteration % 2 == 0 {
                    Architecture::X86_64
                } else {
                    Architecture::Aarch64
                },
                features: u64::from(seed & 3),
            },
            sections: FaslSection {
                code,
                relocations: vec![Relocation {
                    section: SectionId(0),
                    offset: 0,
                    kind: RelocKind::Abs64,
                    symbol: SymbolRef::Local(0),
                    addend: i64::from(i32::from_ne_bytes(seed.to_ne_bytes())),
                }],
                constants: seed.to_le_bytes().to_vec(),
                symbols: vec![u8::try_from(iteration).unwrap_or_default()],
                stack_maps: vec![u8::try_from(seed & 0xff).unwrap_or_default()],
                debug: vec![
                    u8::try_from(iteration).unwrap_or_default(),
                    u8::try_from((seed >> 8) & 0xff).unwrap_or_default(),
                ],
            },
        };
        let bytes = FaslWriter::write(&fasl).unwrap_or_default();
        let decoded = FaslReader::read(&bytes, fasl.header.architecture, fasl.header.features)
            .unwrap_or_else(|_| Fasl {
                header: fasl.header,
                sections: FaslSection {
                    code: Vec::new(),
                    relocations: Vec::new(),
                    constants: Vec::new(),
                    symbols: Vec::new(),
                    stack_maps: Vec::new(),
                    debug: Vec::new(),
                },
            });
        assert_eq!(decoded, fasl);
    }
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

    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[11] = 4;
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());
    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[12] = 2;
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());
    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[16] = 1;
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());
}

#[test]
fn fasl_rejects_overlapping_sections_and_out_of_range_relocations() {
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: vec![0xc3],
            relocations: vec![Relocation {
                section: SectionId(0),
                offset: 0,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::Local(0),
                addend: 0,
            }],
            constants: vec![],
            symbols: vec![],
            stack_maps: vec![],
            debug: vec![],
        },
    };
    let mut bytes = FaslWriter::write(&value).unwrap_or_default();
    bytes[32..36].copy_from_slice(&64u32.to_le_bytes());
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());

    let value = Fasl {
        sections: FaslSection {
            code: vec![],
            relocations: vec![Relocation {
                section: SectionId(0),
                offset: 0,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::Local(0),
                addend: 0,
            }],
            constants: vec![],
            symbols: vec![],
            stack_maps: vec![],
            debug: vec![],
        },
        ..value
    };
    let bytes = FaslWriter::write(&value).unwrap_or_default();
    assert!(FaslReader::read(&bytes, Architecture::X86_64, 0).is_err());
}

#[test]
#[ignore = "requires macOS codesign and execution"]
fn signed_minimal_macho_executes() {
    let image = ExecutableImage {
        architecture: Architecture::Aarch64,
        code: vec![0x20, 0x00, 0x80, 0xd2, 0xc0, 0x03, 0x5f, 0xd6],
        metadata: b"NCL\0".to_vec(),
    };
    let path = std::env::temp_dir().join("ncl-objfile-minimal-macho");
    let Ok(bytes) = write_mach_executable(&image, MachArchitecture::Arm64) else {
        unreachable!("writer failed");
    };
    let write_result = std::fs::write(&path, bytes);
    assert!(write_result.is_ok(), "write failed");
    let Ok(signed) = std::process::Command::new("codesign")
        .args(["--sign", "-", path.to_str().unwrap_or_default()])
        .status()
    else {
        unreachable!("codesign failed to start");
    };
    assert!(signed.success());
    let Ok(result) = std::process::Command::new(&path).status() else {
        unreachable!("execution failed to start");
    };
    assert_eq!(result.code(), Some(0));
    let _ = std::fs::remove_file(path);
}
