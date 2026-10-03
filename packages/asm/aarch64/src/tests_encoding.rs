use super::*;
use crate::{MemOperand, Shift};

#[test]
fn encoding_helpers_reject_and_accept_boundary_values() {
    assert_eq!(
        encoding_helpers::wide(0, Reg(0), 0, 1, 0),
        Err(EncodeError::ImmediateOutOfRange { value: 1, bits: 6 })
    );
    assert!(encoding_helpers::wide(0, Reg(3), 0xab, 48, 0).is_ok());
    assert_eq!(
        encoding_helpers::addsub(RegOrSp::Sp, RegOrSp::Sp, 4096, false, false),
        Err(EncodeError::ImmediateOutOfRange {
            value: 4096,
            bits: 12
        })
    );
    assert!(encoding_helpers::reg3(0, Reg(0), RegOrSp::Sp, Reg(1), Shift::Asr(63)).is_ok());
    assert_eq!(
        encoding_helpers::reg3(0, Reg(0), RegOrSp::Sp, Reg(1), Shift::Lsl(64)),
        Err(EncodeError::ImmediateOutOfRange { value: 64, bits: 6 })
    );
}

#[test]
fn encoding_boundary_forms_keep_asserted_error_and_sequence_behavior() {
    let invalid_register = VReg {
        number: 32,
        double: true,
    };
    assert_eq!(
        encode(
            &Inst::Fmov {
                rd: invalid_register,
                rn: invalid_register,
            },
            0
        ),
        Err(EncodeError::InvalidRegister(32))
    );
    assert_eq!(
        encode(
            &Inst::Ldr {
                rt: Reg(0),
                mem: MemOperand::Register {
                    base: RegOrSp::Sp,
                    index: Reg(1),
                    extend: None,
                    shift: 16,
                },
            },
            0
        ),
        Err(EncodeError::ImmediateOutOfRange { value: 16, bits: 1 })
    );
}
