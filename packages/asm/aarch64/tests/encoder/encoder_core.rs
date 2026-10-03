#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn golden_core_words() {
    assert_eq!(encode(&Inst::Nop, 0), Ok(0xD503_201F));
    assert_eq!(encode(&Inst::Ret { rn: x(30) }, 0), Ok(0xD65F_03C0));
    assert_eq!(
        encode(
            &Inst::MovZ {
                rd: x(0),
                imm: 1,
                shift: 0
            },
            0
        ),
        Ok(0xD280_0020)
    );
    assert_eq!(
        encode(
            &Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: 1,
                shift: false
            },
            0
        ),
        Ok(0x9100_07FF)
    );
    assert_eq!(
        encode(
            &Inst::Ldp {
                rt: x(29),
                rt2: x(30),
                mem: ncl_asm_aarch64::MemOperand::PostIndex {
                    base: RegOrSp::Sp,
                    offset: 16,
                },
            },
            0,
        ),
        Ok(0xA8C1_7BFD)
    );
    assert_eq!(
        encode(
            &Inst::Mov {
                rd: RegOrSp::Reg(x(29)),
                rn: RegOrSp::Sp,
            },
            0,
        ),
        Ok(0x9100_03FD)
    );
}

#[test]
fn golden_coverage_corpus_is_present() {
    let lines = include_str!("../golden/coverage.txt")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(lines >= 200);
}

#[test]
fn labels_resolve_and_are_retained() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert!(assembler.emit(&Inst::B { label }).is_ok());
    assert!(assembler.bind(label).is_ok());
    let result = assembler.finish();
    assert!(result.is_ok());
    if let Ok(blob) = result {
        assert_eq!(blob.bytes, [1, 0, 0, 20]);
        assert_eq!(blob.fixups[0].target, label);
    }
}

#[test]
fn conditional_literal_and_test_branch_fixups_patch_exact_bytes() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler
        .emit(&Inst::BCond {
            cond: Cond::Eq,
            label,
        })
        .unwrap();
    assembler.emit(&Inst::Cbz { rt: x(0), label }).unwrap();
    assembler
        .emit(&Inst::LdrLiteral { rt: x(1), label })
        .unwrap();
    assembler
        .emit(&Inst::Tbz {
            rt: x(2),
            bit: 1,
            label,
        })
        .unwrap();
    assembler
        .emit(&Inst::Tbnz {
            rt: x(3),
            bit: 33,
            label,
        })
        .unwrap();
    assembler.bind(label).unwrap();
    let blob = assembler.finish().unwrap();
    assert_eq!(
        blob.bytes,
        [
            0xa0, 0x00, 0x00, 0x54, 0x80, 0x00, 0x00, 0xb4, 0x61, 0x00, 0x00, 0x58, 0x42, 0x00,
            0x08, 0x36, 0x23, 0x00, 0x08, 0xb7,
        ]
    );
    assert_eq!(blob.fixups.len(), 5);
}

#[test]
fn immediate_sequence_and_encoding() {
    assert_eq!(mov_imm64(x(0), 1).len(), 1);
    assert_eq!(encode(&Inst::Nop, 0), Ok(0xD503_201F));
    assert!(
        encode(
            &Inst::Add {
                rd: RegOrSp::Reg(x(0)),
                rn: RegOrSp::Reg(x(1)),
                rm: x(2),
                shift: Shift::Lsl(0)
            },
            0
        )
        .is_ok()
    );
}

#[test]
fn mov_imm64_returns_encodable_instruction_sequence() {
    let instructions = mov_imm64(x(3), 0x1234_0000_ffff_0001);
    assert_eq!(instructions.len(), 3);
    let mut assembler = Assembler::new();
    for instruction in &instructions {
        assembler.emit(instruction).unwrap();
    }
    assert_eq!(
        assembler.finish().unwrap().bytes,
        [
            0x23, 0x00, 0x80, 0xd2, 0xe3, 0xff, 0xbf, 0xf2, 0x83, 0x46, 0xe2, 0xf2,
        ]
    );
}

#[test]
fn unbound_label_is_an_error() {
    let mut assembler = Assembler::new();
    assert!(assembler.emit(&Inst::B { label: Label(7) }).is_ok());
    assert!(assembler.finish().is_err());
}

#[test]
fn immediate_and_memory_boundaries_are_checked() {
    assert!(
        encode(
            &Inst::MovZ {
                rd: x(0),
                imm: 1,
                shift: 64
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: 4096,
                shift: false
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::Ldr {
                rt: x(0),
                mem: ncl_asm_aarch64::MemOperand::Unscaled {
                    base: RegOrSp::Sp,
                    offset: 256
                }
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::AndImm {
                rd: x(0),
                rn: x(1),
                imm: 0
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::AndImm {
                rd: x(0),
                rn: x(1),
                imm: u64::MAX
            },
            0
        )
        .is_err()
    );
}

#[test]
fn branch_kinds_are_retained() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert!(
        assembler
            .emit(&Inst::Tbz {
                rt: x(0),
                bit: 3,
                label
            })
            .is_ok()
    );
    assert!(
        assembler
            .emit(&Inst::LdrLiteral { rt: x(1), label })
            .is_ok()
    );
    assert!(assembler.bind(label).is_ok());
    let blob = assembler.finish();
    assert!(blob.is_ok());
    if let Ok(blob) = blob {
        assert_eq!(blob.fixups.len(), 2);
    }
}

#[test]
fn pc_relative_fixups_use_signed_aarch64_layout() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Adr { rd: x(2), label }).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.bind(label).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[..4],
        [0x42, 0x00, 0x00, 0x10]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.emit(&Inst::Adr { rd: x(2), label }).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[4..8],
        [0xe2, 0xff, 0xff, 0x10]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    assembler.emit(&Inst::Nop).unwrap();
    assembler.emit(&Inst::B { label }).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[4..8],
        [0xff, 0xff, 0xff, 0x17]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Adrp { rd: x(2), label }).unwrap();
    for _ in 0..1024 {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[..4],
        [0x02, 0x00, 0x00, 0xb0]
    );

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.bind(label).unwrap();
    for _ in 0..1024 {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.emit(&Inst::Adrp { rd: x(2), label }).unwrap();
    assert_eq!(
        assembler.finish().unwrap().bytes[4096..4100],
        [0xe2, 0xff, 0xff, 0xf0]
    );
}

#[test]
fn pc_relative_adr_rejects_out_of_range_target() {
    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assembler.emit(&Inst::Adr { rd: x(2), label }).unwrap();
    for _ in 0..262_144 {
        assembler.emit(&Inst::Nop).unwrap();
    }
    assembler.bind(label).unwrap();
    assert!(matches!(
        assembler.finish(),
        Err(ncl_asm_aarch64::EncodeError::RelocationOutOfRange { .. })
    ));
}
