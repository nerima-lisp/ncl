use super::*;
use crate::{RelocKind, SectionId, SymbolRef};

#[test]
fn legacy_v2_writer_and_reader_round_trip_nonempty_sections_and_debug_tail() {
    let features = 0x0102_0304_0506_0708;
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::Aarch64,
            features,
        },
        sections: FaslSection {
            code: vec![0; 16],
            relocations: vec![Relocation {
                section: SectionId(0),
                offset: 0,
                kind: RelocKind::Abs64,
                symbol: SymbolRef::Local(0),
                addend: -3,
            }],
            constants: vec![1, 2, 3],
            symbols: vec![4, 5],
            stack_maps: vec![6, 7, 8],
            debug: vec![9, 10],
        },
    };
    let bytes =
        FaslWriter::write(&value).unwrap_or_else(|error| panic!("valid FASL rejected: {error}"));
    assert_eq!(
        &bytes[10..14],
        &[Architecture::Aarch64.wire_tag(), 8, 1, 64]
    );
    assert_eq!(&bytes[16..24], &features.to_le_bytes());
    let debug_start = bytes.len() - value.sections.debug.len();
    assert_eq!(
        FaslReader::read(&bytes, Architecture::Aarch64, features),
        Ok(value)
    );
    for end in 0..debug_start {
        assert!(FaslReader::read(&bytes[..end], Architecture::Aarch64, features).is_err());
    }
}

#[test]
fn legacy_range_and_fixed_field_helpers_report_boundaries() {
    assert_eq!(
        range(&[], "code", u32::MAX, 1),
        Err(ObjectError::OutOfBounds {
            section: "code",
            offset: u64::from(u32::MAX),
            size: 1,
        })
    );
    assert_eq!(
        range(&[], "code", 0, 1),
        Err(ObjectError::OutOfBounds {
            section: "code",
            offset: 0,
            size: 1,
        })
    );
    assert_eq!(range(&[1, 2], "stack", 2, 0), Ok(&[][..]));
    assert_eq!(
        fixed_array::<2>(&[1], 0),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 2,
        })
    );
    assert_eq!(
        byte_at(&[], 0),
        Err(ObjectError::Truncated {
            offset: 0,
            needed: 1,
        })
    );
    assert_eq!(
        write_at(&mut [0], 1, &[2], "header"),
        Err(ObjectError::OutOfBounds {
            section: "header",
            offset: 1,
            size: 1,
        })
    );
}

#[test]
fn legacy_section_reader_reports_relocation_size_failures() {
    let value = Fasl {
        header: FaslHeader {
            architecture: Architecture::Aarch64,
            features: 0,
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
    let valid = FaslWriter::write(&value).unwrap_or_else(|error| panic!("valid FASL: {error}"));
    let mut overflow = valid.clone();
    overflow[36..40].copy_from_slice(&0x1000_0000_u32.to_le_bytes());
    assert_eq!(
        read_fasl_ranges(&overflow, 1).map(|_| ()),
        Err(ObjectError::InvalidField {
            field: "relocation size",
            value: u64::MAX,
        })
    );

    let mut underflow = valid;
    underflow[40..44].copy_from_slice(&63_u32.to_le_bytes());
    assert_eq!(
        read_fasl_ranges(&underflow, 2).map(|_| ()),
        Err(ObjectError::Overlap {
            first: "relocation",
            second: "constant",
        })
    );
}
