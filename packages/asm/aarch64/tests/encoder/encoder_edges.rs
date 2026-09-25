#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn addressing_logical_shift_and_float_edges_are_observed() {
    use ncl_asm_aarch64::{EncodeError, Extend, MemOperand, VReg};

    let r0 = x(0);
    let r1 = x(1);
    let scaled = [
        (
            Inst::Ldr {
                rt: r0,
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
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 8,
                    scale: 8,
                },
            },
            0xF900_07E0,
        ),
        (
            Inst::LdrW {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 4,
                    scale: 4,
                },
            },
            0xB940_07E0,
        ),
        (
            Inst::StrW {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 4,
                    scale: 4,
                },
            },
            0xB900_07E0,
        ),
        (
            Inst::Ldrb {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 1,
                    scale: 1,
                },
            },
            0x3940_07E0,
        ),
        (
            Inst::Strb {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 1,
                    scale: 1,
                },
            },
            0x3900_07E0,
        ),
        (
            Inst::Ldrh {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 2,
                    scale: 2,
                },
            },
            0x7940_07E0,
        ),
        (
            Inst::Strh {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 2,
                    scale: 2,
                },
            },
            0x7900_07E0,
        ),
        (
            Inst::Ldrsw {
                rt: r0,
                mem: MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 4,
                    scale: 4,
                },
            },
            0xB940_07E0,
        ),
    ];
    for (index, (instruction, expected)) in scaled.into_iter().enumerate() {
        assert_eq!(
            encode(&instruction, 0),
            Ok(expected),
            "index {index}: {instruction:?}"
        );
    }

    for mem in [
        MemOperand::Unscaled {
            base: RegOrSp::Sp,
            offset: -8,
        },
        MemOperand::PreIndex {
            base: RegOrSp::Sp,
            offset: 8,
        },
        MemOperand::PostIndex {
            base: RegOrSp::Sp,
            offset: 8,
        },
        MemOperand::Register {
            base: RegOrSp::Sp,
            index: r1,
            extend: None,
            shift: 0,
        },
        MemOperand::Register {
            base: RegOrSp::Sp,
            index: r1,
            extend: Some(Extend::Uxtw),
            shift: 8,
        },
        MemOperand::Register {
            base: RegOrSp::Sp,
            index: r1,
            extend: Some(Extend::Sxtw),
            shift: 0,
        },
        MemOperand::Register {
            base: RegOrSp::Sp,
            index: r1,
            extend: Some(Extend::Sxtx),
            shift: 8,
        },
    ] {
        assert!(encode(&Inst::Ldr { rt: r0, mem }, 0).is_ok());
        assert!(encode(&Inst::Str { rt: r0, mem }, 0).is_ok());
    }
    for mem in [
        MemOperand::Unsigned {
            base: RegOrSp::Sp,
            offset: 8,
            scale: 4,
        },
        MemOperand::Register {
            base: RegOrSp::Sp,
            index: r1,
            extend: Some(Extend::Sxtb),
            shift: 1,
        },
    ] {
        assert!(matches!(
            encode(&Inst::Ldr { rt: r0, mem }, 0),
            Err(EncodeError::ImmediateOutOfRange { .. })
        ));
    }

    for (index, instruction) in [
        Inst::AndImm {
            rd: r0,
            rn: r1,
            imm: 1,
        },
        Inst::OrrImm {
            rd: r0,
            rn: r1,
            imm: 0x00FF_00FF_00FF_00FF,
        },
        Inst::EorImm {
            rd: r0,
            rn: r1,
            imm: 0x0000_0000_0000_00FF,
        },
        Inst::TstImm { rn: r1, imm: 0xF0 },
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            encode(&instruction, 0).is_ok(),
            "logical index {index}: {instruction:?}"
        );
    }
    for amount in [0, 1, 31, 63] {
        for instruction in [
            Inst::LslImm {
                rd: r0,
                rn: r1,
                amount,
            },
            Inst::LsrImm {
                rd: r0,
                rn: r1,
                amount,
            },
            Inst::AsrImm {
                rd: r0,
                rn: r1,
                amount,
            },
        ] {
            assert!(encode(&instruction, 0).is_ok());
        }
    }
    assert!(matches!(
        encode(
            &Inst::LslImm {
                rd: r0,
                rn: r1,
                amount: 64
            },
            0
        ),
        Err(EncodeError::ImmediateOutOfRange { .. })
    ));
    assert!(matches!(
        encode(
            &Inst::Tbz {
                rt: r0,
                bit: 64,
                label: Label(0)
            },
            0
        ),
        Err(EncodeError::ImmediateOutOfRange { .. })
    ));

    let v0 = VReg {
        number: 0,
        double: true,
    };
    let v1 = VReg {
        number: 1,
        double: true,
    };
    for instruction in [
        Inst::Fmov { rd: v0, rn: v1 },
        Inst::FmovGeneral {
            v: v0,
            r: r0,
            to_float: false,
        },
        Inst::Fadd {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fsub {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fmul {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fdiv {
            rd: v0,
            rn: v1,
            rm: v0,
        },
        Inst::Fsqrt { rd: v0, rn: v1 },
        Inst::Fneg { rd: v0, rn: v1 },
        Inst::Fcmp { rn: v0, rm: v1 },
        Inst::Scvtf { rd: v0, rn: r0 },
        Inst::Fcvtzs { rd: r0, rn: v0 },
        Inst::LdrD {
            rt: v0,
            mem: MemOperand::Unsigned {
                base: RegOrSp::Sp,
                offset: 8,
                scale: 8,
            },
        },
        Inst::StrD {
            rt: v0,
            mem: MemOperand::Unsigned {
                base: RegOrSp::Sp,
                offset: 8,
                scale: 8,
            },
        },
    ] {
        assert!(encode(&instruction, 0).is_ok());
    }
    assert!(
        encode(
            &Inst::Fmov {
                rd: VReg {
                    number: 32,
                    double: true
                },
                rn: v1
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::Fadd {
                rd: v0,
                rn: VReg {
                    number: 1,
                    double: false
                },
                rm: v0
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::LdrD {
                rt: v0,
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Sp,
                    offset: 0
                }
            },
            0
        )
        .is_err()
    );
}

