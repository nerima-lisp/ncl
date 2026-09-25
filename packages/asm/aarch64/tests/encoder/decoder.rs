#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn decoder_covers_supported_instruction_families() {
    let r0 = x(0);
    let r1 = x(1);
    let label = Label(0);
    let words = [
        encode(
            &Inst::MovZ {
                rd: r0,
                imm: 7,
                shift: 16,
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: 7,
                shift: true,
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::SubImm {
                rd: RegOrSp::Reg(r0),
                rn: RegOrSp::Reg(r1),
                imm: 7,
                shift: false,
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Ldr {
                rt: r0,
                mem: ncl_asm_aarch64::MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 8,
                    scale: 8,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Str {
                rt: r0,
                mem: ncl_asm_aarch64::MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 8,
                    scale: 8,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Ldr {
                rt: r0,
                mem: ncl_asm_aarch64::MemOperand::Unscaled {
                    base: RegOrSp::Sp,
                    offset: -8,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Str {
                rt: r0,
                mem: ncl_asm_aarch64::MemOperand::PreIndex {
                    base: RegOrSp::Sp,
                    offset: 8,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Ldr {
                rt: r0,
                mem: ncl_asm_aarch64::MemOperand::PostIndex {
                    base: RegOrSp::Sp,
                    offset: 8,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Cmp {
                rn: r0,
                rm: r1,
                shift: Shift::Lsl(0),
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Mov {
                rd: RegOrSp::Reg(r0),
                rn: RegOrSp::Reg(r1),
            },
            0,
        )
        .unwrap(),
        encode(&Inst::Blr { rn: r1 }, 0).unwrap(),
        encode(&Inst::Adr { rd: r0, label }, 0).unwrap(),
        encode(&Inst::Adrp { rd: r0, label }, 0).unwrap(),
        encode(&Inst::Cbz { rt: r0, label }, 0).unwrap(),
        encode(&Inst::Cbnz { rt: r0, label }, 0).unwrap(),
        encode(&Inst::Brk { imm: 7 }, 0).unwrap(),
        encode(&Inst::B { label }, 0).unwrap(),
        encode(
            &Inst::BCond {
                cond: Cond::Al,
                label,
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Ldp {
                rt: r0,
                rt2: r1,
                mem: ncl_asm_aarch64::MemOperand::Unscaled {
                    base: RegOrSp::Sp,
                    offset: 0,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Ldp {
                rt: r0,
                rt2: r1,
                mem: ncl_asm_aarch64::MemOperand::PreIndex {
                    base: RegOrSp::Sp,
                    offset: 8,
                },
            },
            0,
        )
        .unwrap(),
        encode(
            &Inst::Stp {
                rt: r0,
                rt2: r1,
                mem: ncl_asm_aarch64::MemOperand::PostIndex {
                    base: RegOrSp::Sp,
                    offset: 8,
                },
            },
            0,
        )
        .unwrap(),
    ];
    for word in words {
        assert!(decode(word).is_ok(), "decoder rejected 0x{word:08x}");
    }
    for value in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
        let word = 0x5400_0000 | value;
        assert!(decode(word).is_ok());
    }
    assert!(matches!(
        decode(0),
        Err(ncl_asm_aarch64::EncodeError::UnsupportedInstruction(0))
    ));
}

