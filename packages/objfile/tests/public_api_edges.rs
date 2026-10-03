#![allow(missing_docs, clippy::expect_used)]

use ncl_objfile::*;

fn empty_fasl() -> Fasl {
    Fasl {
        header: FaslHeader {
            architecture: Architecture::X86_64,
            features: 0,
        },
        sections: FaslSection {
            code: vec![0xc3],
            relocations: vec![],
            constants: vec![],
            symbols: vec![],
            stack_maps: vec![],
            debug: vec![],
        },
    }
}

#[test]
fn public_fasl_reader_reports_relocation_size_overflow() {
    let mut bytes = FaslWriter::write(&empty_fasl()).expect("valid FASL");
    bytes[36..40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        FaslReader::read(&bytes, Architecture::X86_64, 0),
        Err(ObjectError::InvalidField {
            field: "relocation size",
            value: u64::MAX,
        })
    );
}

#[test]
fn public_mach_executable_round_trip_preserves_aarch64_envelope() {
    let image = ExecutableImage {
        architecture: Architecture::Aarch64,
        code: vec![0x20, 0x00, 0x80, 0xd2],
        metadata: vec![0x4e, 0x43, 0x4c, 0x00],
    };
    let bytes =
        write_mach_executable(&image, MachArchitecture::Arm64).expect("AArch64 Mach executable");
    assert_eq!(&bytes[..4], &[0xcf, 0xfa, 0xed, 0xfe]);
    assert_eq!(
        validate_mach_executable(&bytes, MachArchitecture::Arm64),
        Ok(())
    );
    assert_eq!(
        validate_mach_executable(&bytes, MachArchitecture::X86_64),
        Err(ObjectError::InvalidField {
            field: "Mach-O CPU",
            value: 0x0100_000c,
        })
    );
}

#[test]
fn public_mach_executable_reports_truncated_load_command() {
    let image = ExecutableImage {
        architecture: Architecture::X86_64,
        code: vec![0xc3],
        metadata: vec![1],
    };
    let bytes =
        write_mach_executable(&image, MachArchitecture::X86_64).expect("x86-64 Mach executable");
    assert_eq!(
        validate_mach_executable(&bytes[..63], MachArchitecture::X86_64),
        Err(ObjectError::InvalidStructure(
            "invalid Mach-O load commands"
        ))
    );
}
