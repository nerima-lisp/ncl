#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use ncl_asm_aarch64::{Assembler, Inst, Label, Reg, RegOrSp, Shift, decode, encode, mov_imm64};

#[allow(
    clippy::option_if_let_else,
    reason = "The helper is const so test register literals remain compile-time values."
)]
const fn x(n: u8) -> Reg {
    match Reg::new(n) {
        Ok(register) => register,
        Err(_) => Reg(0),
    }
}

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
    let lines = include_str!("golden/coverage.txt")
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
fn immediate_sequence_and_decode() {
    assert_eq!(mov_imm64(x(0), 1).len(), 1);
    assert_eq!(decode(0xD503_201F), Ok(Inst::Nop));
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
