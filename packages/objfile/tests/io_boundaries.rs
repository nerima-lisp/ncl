#![allow(missing_docs, clippy::expect_used)]

use ncl_objfile::{Architecture, Fasl, FaslHeader, FaslReader, FaslSection, FaslWriter};

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
