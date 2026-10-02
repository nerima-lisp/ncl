use super::{emit, load_value, store_value};
use crate::{Allocation, CodegenError};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Prim, ValueId};

pub(super) fn load_callable_address(
    assembler: &mut Assembler,
    source: Reg,
    destination: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(destination),
            rn: RegOrSp::Reg(source),
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: destination,
            rn: destination,
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )
}

pub(super) fn decode_function_entry(
    assembler: &mut Assembler,
    register: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::AsrImm {
            rd: register,
            rn: register,
            amount: u8::try_from(ncl_sys::FIXNUM_TAG_BITS)
                .map_err(|_| CodegenError::FrameOverflow)?,
        },
    )
}

/// Loads a lexical capture from the closure object retained in the generated
/// frame.
pub(super) fn lower_load_capture(
    assembler: &mut Assembler,
    index: u32,
    result: Option<ValueId>,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let offset = ncl_object::function_offset::CAPTURES
        .checked_add(usize::try_from(index).map_err(|_| CodegenError::FrameOverflow)?)
        .and_then(|slot| slot.checked_add(1))
        .and_then(|slot| slot.checked_mul(8))
        .and_then(|offset| i16::try_from(offset).ok())
        .ok_or(CodegenError::FrameOverflow)?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(29)),
                offset: 16,
            },
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(16),
            rn: Reg(16),
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(16)),
                offset,
            },
        },
    )?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

fn emit_lisp_boolean(assembler: &mut Assembler, condition: Cond) -> Result<(), CodegenError> {
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), ncl_sys::Word::TRUE.bits()) {
        emit(assembler, instruction)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), ncl_sys::Word::NIL.bits()) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Csel {
            rd: Reg(16),
            rn: Reg(16),
            rm: Reg(17),
            cond: condition,
        },
    )
}

// `argc`/`args` mirror the calling convention's own argument-count/argument-
// list naming; that pairing is clearer here than any alternative spelling.
#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub(super) fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    named_symbol: Option<ValueId>,
    abi: &dyn crate::RuntimeAbi,
) -> Result<(), CodegenError> {
    super::lower_closure_call(assembler, closure, args, allocation, named_symbol, abi)
}

#[allow(clippy::too_many_lines)]
pub(super) fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "primitive has no operands".into(),
        ));
    };
    load_value(assembler, allocation, *first, Reg(16))?;
    if let Some(second) = args.get(1) {
        load_value(assembler, allocation, *second, Reg(17))?;
    }
    match prim {
        Prim::FixnumAdd => emit(
            assembler,
            Inst::Add {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?,
        Prim::FixnumSub => emit(
            assembler,
            Inst::Sub {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?,
        Prim::FixnumMul => emit(
            assembler,
            Inst::Mul {
                rd: Reg(16),
                rn: Reg(16),
                rm: Reg(17),
            },
        )?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit_lisp_boolean(assembler, Cond::Eq)?;
        }
        Prim::FixnumLt => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit_lisp_boolean(assembler, Cond::Lt)?;
        }
        Prim::FixnumLe => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit_lisp_boolean(assembler, Cond::Le)?;
        }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => {
            let offset = if matches!(prim, Prim::Cdr) { 8 } else { 0 };
            emit(
                assembler,
                Inst::AndImm {
                    rd: Reg(16),
                    rn: Reg(16),
                    imm: !ncl_sys::LOWTAG_MASK,
                },
            )?;
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => {
            let offset = if matches!(prim, Prim::Rplacd) { 8 } else { 0 };
            emit(
                assembler,
                Inst::AndImm {
                    rd: Reg(16),
                    rn: Reg(16),
                    imm: !ncl_sys::LOWTAG_MASK,
                },
            )?;
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(17),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
            // check-added-lines: allow(unsupported) existing codegen error variant
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        assert!(!encoded(assembler).is_empty());
    }
}
