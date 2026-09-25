#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn current_api_encodes_instruction_families_to_exact_words() {
    let label = Label(0);
    let cases = [
        (
            Inst::MovZ {
                rd: x(0),
                imm: 7,
                shift: 16,
            },
            0xD2A0_00E0,
        ),
        (
            Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: 7,
                shift: true,
            },
            0x9140_1FFF,
        ),
        (
            Inst::Ldr {
                rt: x(0),
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 8,
                    scale: 8,
                },
            },
            0xF940_07E0,
        ),
        (
            Inst::Str {
                rt: x(0),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Sp,
                    offset: -8,
                },
            },
            0xF81F_83E0,
        ),
        (Inst::B { label }, 0x1400_0000),
        (
            Inst::BCond {
                cond: Cond::Al,
                label,
            },
            0x5400_000E,
        ),
        (Inst::Ret { rn: x(30) }, 0xD65F_03C0),
        (Inst::Nop, 0xD503_201F),
    ];

    for (instruction, expected) in cases {
        assert_eq!(encode(&instruction, 0), Ok(expected), "{instruction:?}");
    }
}

#[test]
fn current_api_rejects_invalid_operands() {
    assert_eq!(
        encode(
            &Inst::MovZ {
                rd: x(0),
                imm: 1,
                shift: 64,
            },
            0
        ),
        Err(EncodeError::ImmediateOutOfRange { value: 64, bits: 6 })
    );
    assert_eq!(
        encode(
            &Inst::AddExt {
                rd: RegOrSp::Reg(x(0)),
                rn: RegOrSp::Reg(x(1)),
                rm: x(2),
                extend: Extend::Uxtw,
                shift: 5,
            },
            0
        ),
        Err(EncodeError::ImmediateOutOfRange { value: 5, bits: 3 })
    );
    assert_eq!(encode(&Inst::Udf { imm: 1 }, 0), Ok(0x20));
}

#[test]
fn current_api_covers_movn_and_condition_encoding() {
    assert_eq!(
        mov_imm64(x(0), 0xFFFF_FFFF_FFFF_0000),
        vec![Inst::MovN {
            rd: x(0),
            imm: u16::MAX,
            shift: 0,
        }]
    );
    let conditions = [
        (Cond::Eq, 0),
        (Cond::Ne, 1),
        (Cond::Cs, 2),
        (Cond::Cc, 3),
        (Cond::Mi, 4),
        (Cond::Pl, 5),
        (Cond::Vs, 6),
        (Cond::Vc, 7),
        (Cond::Hi, 8),
        (Cond::Ls, 9),
        (Cond::Ge, 10),
        (Cond::Lt, 11),
        (Cond::Gt, 12),
        (Cond::Le, 13),
        (Cond::Al, 14),
    ];
    for (condition, bits) in conditions {
        assert_eq!(
            encode(
                &Inst::BCond {
                    cond: condition,
                    label: Label(1)
                },
                0
            ),
            Ok(0x5400_0000 | bits)
        );
    }
}
