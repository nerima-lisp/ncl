#![allow(missing_docs)]
#![allow(clippy::unwrap_used)]

use super::common::*;
use ncl_asm_aarch64::*;

#[test]
fn control_and_alias_encodings_are_exact() {
    assert_eq!(encode(&Inst::Br { rn: x(1) }, 0), Ok(0xD61F_0020));
    assert_eq!(encode(&Inst::Blr { rn: x(1) }, 0), Ok(0xD63F_0020));
    assert_eq!(
        encode(
            &Inst::Csel {
                rd: x(0),
                rn: x(1),
                rm: x(2),
                cond: Cond::Ne,
            },
            0
        ),
        Ok(0x9A82_1020)
    );
    assert_eq!(
        encode(
            &Inst::Cset {
                rd: x(0),
                cond: Cond::Eq
            },
            0
        ),
        Ok(0x9A9F_17E0)
    );
    assert_eq!(
        encode(
            &Inst::Cinc {
                rd: x(0),
                rn: x(1),
                cond: Cond::Gt,
            },
            0
        ),
        Ok(0x9A80_D420)
    );
    assert_eq!(
        encode(
            &Inst::FmovGeneral {
                v: VReg {
                    number: 2,
                    double: true,
                },
                r: x(3),
                to_float: true,
            },
            0
        ),
        Ok(0x9E67_0043)
    );
    assert_eq!(
        encode(
            &Inst::FmovGeneral {
                v: VReg {
                    number: 2,
                    double: true,
                },
                r: x(3),
                to_float: false,
            },
            0
        ),
        Ok(0x9E66_0043)
    );
}

#[test]
fn assembler_rejects_invalid_emission_without_advancing() {
    let mut assembler = Assembler::new();
    assert_eq!(assembler.offset(), 0);
    assert_eq!(
        assembler.emit(&Inst::MovZ {
            rd: x(0),
            imm: 1,
            shift: 64,
        }),
        Err(EncodeError::ImmediateOutOfRange { value: 64, bits: 6 })
    );
    assert_eq!(assembler.offset(), 0);
    assembler.emit(&Inst::Nop).unwrap();
    assert_eq!(assembler.offset(), 4);
}

#[test]
fn register_and_label_boundaries_are_typed_errors() {
    assert_eq!(Reg::new(30).map(Reg::number), Ok(30));
    assert_eq!(Reg::new(31), Err(EncodeError::InvalidRegister(31)));

    let mut assembler = Assembler::new();
    let label = assembler.new_label();
    assert_eq!(assembler.bind(label), Ok(()));
    assert_eq!(
        assembler.bind(label),
        Err(EncodeError::DuplicateLabel(label))
    );
    assert_eq!(assembler.emit(&Inst::B { label }), Ok(()));
    assert_eq!(assembler.finish().unwrap().bytes, [0, 0, 0, 20]);
}

#[test]
fn encode_errors_have_stable_actionable_display() {
    assert_eq!(
        EncodeError::InvalidRegister(31).to_string(),
        "invalid register x31"
    );
    assert_eq!(
        EncodeError::ImmediateOutOfRange { value: -1, bits: 6 }.to_string(),
        "immediate -1 does not fit 6 bits"
    );
    assert_eq!(
        EncodeError::InvalidBitmaskImmediate(0x100).to_string(),
        "invalid logical immediate 0x100"
    );
    assert_eq!(
        EncodeError::UnboundLabel(Label(3)).to_string(),
        "unbound label Label(3)"
    );
    assert_eq!(
        EncodeError::DuplicateLabel(Label(4)).to_string(),
        "duplicate label Label(4)"
    );
    assert_eq!(
        EncodeError::RelocationOutOfRange {
            offset: 8,
            target: Label(5),
        }
        .to_string(),
        "relocation at 8 to Label(5) is out of range"
    );
    assert_eq!(
        EncodeError::InvalidLength.to_string(),
        "instruction bytes are not four-byte aligned"
    );
    assert_eq!(
        EncodeError::UnsupportedInstruction(0xD503_201F).to_string(),
        "unsupported instruction 0xd503201f"
    );
}
