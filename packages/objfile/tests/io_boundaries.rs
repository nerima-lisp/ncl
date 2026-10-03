#![allow(missing_docs, clippy::expect_used)]

use ncl_objfile::{
    Architecture, ElfArchitecture, ElfObject, Fasl, FaslHeader, FaslReader, FaslSection,
    FaslWriter, MachArchitecture, MachObject, MachReader, ObjectError, RelocKind, Relocation,
    SectionId, SymbolRef,
};

#[test]
fn empty_fasl_sections_round_trip_without_losing_header_state() {
    let input = Fasl {
        header: FaslHeader {
            architecture: Architecture::Aarch64,
            features: u64::MAX,
        },
        sections: FaslSection {
            code: Vec::new(),
            relocations: Vec::new(),
            constants: Vec::new(),
            symbols: Vec::new(),
            stack_maps: Vec::new(),
            debug: Vec::new(),
        },
    };

    let bytes = FaslWriter::write(&input).expect("empty FASL is representable");
    assert_eq!(bytes.len(), 64);
    let output = FaslReader::read(&bytes, Architecture::Aarch64, u64::MAX)
        .expect("empty FASL remains readable");
    assert_eq!(output, input);
}

#[test]
fn fasl_round_trip_preserves_wire_edge_values_and_debug_tail() {
    let input = Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: vec![0; 2],
            relocations: vec![
                Relocation {
                    section: SectionId(0),
                    offset: 0,
                    kind: RelocKind::Abs64,
                    symbol: SymbolRef::Local(0),
                    addend: i64::from(i32::MIN),
                },
                Relocation {
                    section: SectionId(0),
                    offset: 1,
                    kind: RelocKind::PcRel32,
                    symbol: SymbolRef::Local(0),
                    addend: i64::from(i32::MAX),
                },
            ],
            constants: vec![0xff],
            symbols: vec![0],
            stack_maps: vec![0xaa, 0x55],
            debug: vec![0xde, 0xad, 0xbe, 0xef],
        },
    };

    let bytes = FaslWriter::write(&input).expect("edge values are representable");
    let output = FaslReader::read(&bytes, Architecture::X86_64, 0)
        .expect("encoded edge values remain readable");
    assert_eq!(output, input);
    assert_eq!(&bytes[bytes.len() - 4..], &[0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn native_empty_objects_are_valid_but_empty_executable_payloads_are_rejected() {
    let elf = ElfObject {
        architecture: ElfArchitecture::X86_64,
        sections: Vec::new(),
        relocations: Vec::new(),
        symbols: Vec::new(),
    }
    .write()
    .expect("empty ELF object is representable");
    assert_eq!(
        ncl_objfile::validate_elf(&elf, ElfArchitecture::X86_64),
        Ok(())
    );

    let macho = MachObject {
        architecture: MachArchitecture::Arm64,
        sections: Vec::new(),
        relocations: Vec::new(),
    }
    .write()
    .expect("empty Mach-O object is representable");
    assert_eq!(
        MachReader::validate(&macho, MachArchitecture::Arm64),
        Ok(())
    );

    let image = ncl_objfile::ExecutableImage {
        architecture: Architecture::X86_64,
        code: Vec::new(),
        metadata: Vec::new(),
    };
    let elf_executable = ncl_objfile::write_elf_executable(&image)
        .expect("zero-length executable payloads are encodable");
    assert_eq!(
        ncl_objfile::validate_elf_executable(&elf_executable, Architecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "ELF entry is outside executable segment"
        ))
    );
    let macho_executable = ncl_objfile::write_mach_executable(&image, MachArchitecture::X86_64)
        .expect("zero-length Mach-O payloads are encodable");
    assert_eq!(
        ncl_objfile::validate_mach_executable(&macho_executable, MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "missing NCL executable metadata"
        ))
    );
}
