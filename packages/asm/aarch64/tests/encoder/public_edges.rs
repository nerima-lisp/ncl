#![allow(clippy::expect_used)]

use ncl_asm_aarch64::{EncodeError, Inst, Reg, encode, mov_imm64};

#[test]
fn public_mov_imm64_selects_exact_zero_and_all_ones_sequences() {
    assert_eq!(
        mov_imm64(Reg(3), 0),
        vec![Inst::MovZ {
            rd: Reg(3),
            imm: 0,
            shift: 0,
        }]
    );
    assert_eq!(
        mov_imm64(Reg(3), u64::MAX),
        vec![Inst::MovN {
            rd: Reg(3),
            imm: 0,
            shift: 0,
        }]
    );
}

#[test]
fn public_aarch64_logical_immediate_reports_exact_error_and_bytes() {
    assert_eq!(
        encode(
            &Inst::AndImm {
                rd: Reg(0),
                rn: Reg(1),
                imm: 1,
            },
            0,
        ),
        Ok(0x9240_0020)
    );
    assert_eq!(
        encode(
            &Inst::AndImm {
                rd: Reg(0),
                rn: Reg(1),
                imm: 0,
            },
            0,
        ),
        Err(EncodeError::InvalidBitmaskImmediate(0))
    );
}
