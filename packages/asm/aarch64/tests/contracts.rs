#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn public_value_and_error_contracts_are_exercised() {
    use ncl_asm_aarch64::EncodeError;

    assert_eq!(Reg::new(30).unwrap().number(), 30);
    assert_eq!(Reg::new(31), Err(EncodeError::InvalidRegister(31)));
    assert_eq!(disassemble(0xD65F_03C0), Ok("ret x30".to_owned()));
    assert_eq!(mov_imm64(x(0), u64::MAX).len(), 1);
    assert_eq!(mov_imm64(x(0), 0x0001_0002_0003_0004).len(), 4);
    assert_eq!(encode(&Inst::Bl { label: Label(0) }, 0), Ok(0x9400_0000));

    for instruction in [
        Inst::Sub {
            rd: RegOrSp::Reg(x(0)),
            rn: RegOrSp::Reg(x(1)),
            rm: x(2),
            shift: Shift::Lsl(0),
        },
        Inst::Add {
            rd: RegOrSp::Reg(x(0)),
            rn: RegOrSp::Reg(x(1)),
            rm: x(2),
            shift: Shift::Asr(1),
        },
    ] {
        assert!(encode(&instruction, 0).is_ok());
    }
    assert!(
        encode(
            &Inst::AddExt {
                rd: RegOrSp::Reg(x(0)),
                rn: RegOrSp::Reg(x(1)),
                rm: x(2),
                extend: ncl_asm_aarch64::Extend::Uxtw,
                shift: 5,
            },
            0
        )
        .is_err()
    );
    assert!(
        encode(
            &Inst::Ldp {
                rt: x(0),
                rt2: x(1),
                mem: ncl_asm_aarch64::MemOperand::Unsigned {
                    base: RegOrSp::Sp,
                    offset: 4,
                    scale: 4,
                },
            },
            0
        )
        .is_err()
    );

    let errors = [
        EncodeError::InvalidRegister(31),
        EncodeError::ImmediateOutOfRange { value: -1, bits: 6 },
        EncodeError::InvalidBitmaskImmediate(0),
        EncodeError::UnboundLabel(Label(1)),
        EncodeError::DuplicateLabel(Label(2)),
        EncodeError::RelocationOutOfRange {
            offset: 4,
            target: Label(3),
        },
        EncodeError::InvalidLength,
        EncodeError::UnsupportedInstruction(0),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
}
