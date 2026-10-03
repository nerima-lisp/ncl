use super::*;
use crate::{Allocation, Location};
use std::collections::BTreeMap;

fn allocation() -> Allocation {
    Allocation {
        intervals: Vec::new(),
        locations: vec![
            (ValueId(0), Location::Register(1)),
            (ValueId(1), Location::Register(2)),
            (ValueId(2), Location::Register(3)),
        ],
        spill_words: 0,
        safepoint_registers: BTreeMap::new(),
        outgoing_base: 0,
        incoming_args_base: None,
    }
}

fn encoded(assembler: Assembler) -> Vec<u8> {
    assembler
        .finish()
        .unwrap_or_else(|error| panic!("AArch64 instruction encoding: {error:?}"))
        .bytes
}

fn assert_instruction(bytes: &[u8], offset: usize, instruction: &Inst) {
    let actual = u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .unwrap_or_else(|_| panic!("complete AArch64 instruction")),
    );
    let expected = ncl_asm_aarch64::encode(instruction, offset)
        .unwrap_or_else(|error| panic!("expected encoding: {error:?}"));
    assert_eq!(actual, expected, "instruction at byte offset {offset}");
}
#[test]
#[allow(clippy::too_many_lines)]
fn aarch64_primitives_emit_their_operation_kind() {
    let allocation = allocation();
    let cases = [
        (
            Prim::FixnumAdd,
            8,
            Inst::Add {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
        (
            Prim::FixnumSub,
            8,
            Inst::Sub {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
        (
            Prim::FixnumMul,
            8,
            Inst::Mul {
                rd: Reg(16),
                rn: Reg(16),
                rm: Reg(17),
            },
        ),
        (
            Prim::FixnumEq,
            8,
            Inst::Cmp {
                rn: Reg(16),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
        (
            Prim::Eq,
            8,
            Inst::Cmp {
                rn: Reg(16),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
        (
            Prim::Eql,
            8,
            Inst::Cmp {
                rn: Reg(16),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
        (
            Prim::FixnumLt,
            8,
            Inst::Cmp {
                rn: Reg(16),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
        (
            Prim::FixnumLe,
            8,
            Inst::Cmp {
                rn: Reg(16),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        ),
    ];
    for (primitive, offset, expected_instruction) in cases {
        let mut assembler = Assembler::new();
        lower_prim(
            &mut assembler,
            &primitive,
            &[ValueId(0), ValueId(1)],
            Some(ValueId(2)),
            &allocation,
        )
        .unwrap_or_else(|error| panic!("{primitive:?}: {error:?}"));
        assert_instruction(&encoded(assembler), offset, &expected_instruction);
    }

    for (primitive, offset, expected_instruction) in [
        (
            Prim::Car,
            12,
            Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 0,
                },
            },
        ),
        (
            Prim::Cdr,
            12,
            Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 8,
                },
            },
        ),
        (
            Prim::Svref,
            12,
            Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 0,
                },
            },
        ),
        (
            Prim::Aref,
            12,
            Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 0,
                },
            },
        ),
        (
            Prim::Rplaca,
            12,
            Inst::Str {
                rt: Reg(17),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 0,
                },
            },
        ),
        (
            Prim::Rplacd,
            12,
            Inst::Str {
                rt: Reg(17),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 8,
                },
            },
        ),
        (
            Prim::Aset,
            12,
            Inst::Str {
                rt: Reg(17),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset: 0,
                },
            },
        ),
    ] {
        let mut assembler = Assembler::new();
        lower_prim(
            &mut assembler,
            &primitive,
            &[ValueId(0), ValueId(1)],
            Some(ValueId(2)),
            &allocation,
        )
        .unwrap_or_else(|error| panic!("{primitive:?}: {error:?}"));
        assert_instruction(&encoded(assembler), offset, &expected_instruction);
    }
}
#[test]
fn aarch64_primitives_reject_malformed_and_unimplemented_forms() {
    let allocation = allocation();
    let mut assembler = Assembler::new();
    assert!(matches!(
        lower_prim(&mut assembler, &Prim::FixnumAdd, &[], None, &allocation),
        Err(CodegenError::Unsupported(message)) if message == "primitive has no operands"
    ));
    for primitive in [
        Prim::FixnumDiv,
        Prim::Typep,
        Prim::CharacterPredicate("characterp".into()),
        Prim::StructureSlot("car".into()),
    ] {
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_prim(&mut assembler, &primitive, &[ValueId(0)], None, &allocation),
            Err(CodegenError::Unsupported(_))
        ));
    }
}

#[test]
fn primitive_helpers_cover_capture_offset_and_resultless_success_boundaries() {
    let allocation = allocation();
    let mut assembler = Assembler::new();
    assert_eq!(
        lower_load_capture(&mut assembler, u32::MAX, None, &allocation,),
        Err(CodegenError::FrameOverflow)
    );

    let mut assembler = Assembler::new();
    lower_prim(
        &mut assembler,
        &Prim::FixnumAdd,
        &[ValueId(0), ValueId(1)],
        None,
        &allocation,
    )
    .unwrap_or_else(|error| panic!("resultless primitive: {error:?}"));
    let actual = encoded(assembler);
    let mut expected = Assembler::new();
    for instruction in [
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(16)),
            rn: RegOrSp::Reg(Reg(1)),
        },
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(2)),
        },
        Inst::Add {
            rd: RegOrSp::Reg(Reg(16)),
            rn: RegOrSp::Reg(Reg(16)),
            rm: Reg(17),
            shift: Shift::Lsl(0),
        },
    ] {
        expected
            .emit(&instruction)
            .unwrap_or_else(|error| panic!("expected instruction encoding: {error:?}"));
    }
    assert_eq!(actual, encoded(expected));
}
