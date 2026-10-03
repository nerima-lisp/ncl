#![allow(missing_docs, clippy::expect_used)]

use ncl_disasm::{Architecture, DecodedInstruction, decode};

#[test]
fn public_decode_returns_instruction_bytes_text_and_targets() {
    let decoded = decode(Architecture::X86_64, &[0xe9, 0, 0, 0, 0, 0xc3], 0x1000).expect("decode");

    assert_eq!(
        decoded,
        vec![
            DecodedInstruction {
                address: 0x1000,
                size: 5,
                bytes: vec![0xe9, 0, 0, 0, 0],
                text: "jmp".into(),
                branch_target: Some(0x1005),
            },
            DecodedInstruction {
                address: 0x1005,
                size: 1,
                bytes: vec![0xc3],
                text: "ret".into(),
                branch_target: None,
            },
        ]
    );
}
