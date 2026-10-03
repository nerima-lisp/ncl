#![allow(missing_docs, clippy::expect_used)]

use ncl_disasm::{Architecture, DecodeError, decode, resolve_labels};

fn words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn public_x86_decode_reports_exact_operands_and_branch_labels() {
    let decoded = decode(
        Architecture::X86_64,
        &[
            0x48, 0x8b, 0x44, 0x8d, 0xf0, // mov -16(%rbp,%rcx,4), %rax
            0xe9, 0xf8, 0xff, 0xff, 0xff, // jmp eight bytes backwards
        ],
        0x4000,
    )
    .expect("supported x86 stream");
    assert_eq!(
        decoded
            .iter()
            .map(|instruction| (
                instruction.address,
                instruction.size,
                instruction.bytes.clone(),
                instruction.text.as_str(),
                instruction.branch_target,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                0x4000,
                5,
                vec![0x48, 0x8b, 0x44, 0x8d, 0xf0],
                "mov -16(%rbp,%rcx,4), %rax",
                None,
            ),
            (
                0x4005,
                5,
                vec![0xe9, 0xf8, 0xff, 0xff, 0xff],
                "jmp",
                Some(0x4002)
            ),
        ]
    );
    assert_eq!(resolve_labels(&decoded), vec![None, None]);
}

#[test]
fn public_x86_decode_rejects_branch_target_overflow_with_exact_error() {
    assert_eq!(
        decode(
            Architecture::X86_64,
            &[0xe9, 0xff, 0xff, 0xff, 0x7f],
            u64::MAX - 3
        ),
        Err(DecodeError::Invalid {
            address: u64::MAX - 3,
            reason: "branch target overflow".into(),
        })
    );
}

#[test]
fn public_aarch64_decode_covers_negative_branch_and_invalid_logical_immediate() {
    let decoded = decode(
        Architecture::Aarch64,
        &words(&[0x17ff_ffff, 0xd503_201f]),
        0x8000,
    )
    .expect("supported AArch64 stream");
    assert_eq!(
        decoded
            .iter()
            .map(|instruction| (
                instruction.address,
                instruction.size,
                instruction.text.as_str(),
                instruction.branch_target,
            ))
            .collect::<Vec<_>>(),
        vec![(0x8000, 4, "b #-4", Some(0x7ffc)), (0x8004, 4, "nop", None),]
    );
    assert_eq!(resolve_labels(&decoded), vec![None, None]);

    assert_eq!(
        decode(Architecture::Aarch64, &words(&[0x9200_fc00]), 0x8000),
        Err(DecodeError::Invalid {
            address: 0x8000,
            reason: "invalid logical immediate".into(),
        })
    );
}
