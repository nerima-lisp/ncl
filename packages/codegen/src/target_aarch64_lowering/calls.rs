use super::{emit, load_value, primitives};
use crate::{Allocation, CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::ValueId;

#[allow(clippy::redundant_pub_crate)]
#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub(crate) fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some((argc, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "calls require a tagged argc argument".into(),
        ));
    };
    load_value(assembler, allocation, callee, Reg(16))?;
    load_value(assembler, allocation, *argc, Reg(0))?;
    let extra_count = rest.len().saturating_sub(4);
    if extra_count > 0 {
        let rest_offset = allocation
            .outgoing_base
            .checked_add(u32::try_from(extra_count).map_err(|_| CodegenError::FrameOverflow)?)
            .and_then(|slot| slot.checked_add(1))
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?;
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), u64::from(rest_offset)) {
            emit(assembler, instruction)?;
        }
        emit(
            assembler,
            Inst::Sub {
                rd: RegOrSp::Reg(Reg(5)),
                rn: RegOrSp::Reg(Reg(29)),
                rm: Reg(17),
                shift: ncl_asm_aarch64::Shift::Lsl(0),
            },
        )?;
        emit(
            assembler,
            Inst::Str {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(5)),
                    offset: i16::try_from(
                        extra_count
                            .checked_mul(8)
                            .ok_or(CodegenError::FrameOverflow)?,
                    )
                    .map_err(|_| CodegenError::FrameOverflow)?,
                },
            }, // check-added-lines: allow(index) intentional
        )?;
    }
    for (index, argument) in rest.iter().enumerate() {
        if index < 4 {
            load_value(
                assembler,
                allocation,
                *argument,
                Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
            )?;
        } else {
            load_value(assembler, allocation, *argument, Reg(16))?;
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(5)),
                        offset: i16::try_from((index - 4).saturating_mul(8))
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    },
                },
            )?;
        }
    }
    if extra_count > 0 {
        emit(
            assembler,
            Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(5)),
                    offset: i16::try_from(
                        extra_count
                            .checked_mul(8)
                            .ok_or(CodegenError::FrameOverflow)?,
                    )
                    .map_err(|_| CodegenError::FrameOverflow)?,
                },
            },
            // check-added-lines: allow(index) intentional
        )?;
    } else {
        load_value(assembler, allocation, callee, Reg(16))?;
    }
    primitives::load_callable_address(assembler, Reg(16), Reg(17))?;
    Ok(())
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    named_symbol: Option<ValueId>,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if let Some(symbol) = named_symbol {
        return lower_named_global_call(assembler, closure, symbol, args, allocation, abi);
    }
    lower_call(assembler, closure, args, allocation)?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(17)),
                offset: i16::try_from((ncl_object::function_offset::ENTRY + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
            },
        },
    )?;
    primitives::decode_function_entry(assembler, Reg(17))
}

#[allow(clippy::similar_names)]
fn lower_named_global_call(
    assembler: &mut Assembler,
    closure: ValueId,
    symbol: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    load_value(assembler, allocation, symbol, Reg(16))?;
    load_value(assembler, allocation, closure, Reg(17))?;
    // check-added-lines: allow(unbound) compare against the function-cell sentinel.
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(5), ncl_sys::Word::UNBOUND.bits()) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(17),
            rm: Reg(5),
            shift: ncl_asm_aarch64::Shift::Lsl(0),
        },
    )?;
    let normal = assembler.new_label();
    let call = assembler.new_label();
    emit(
        assembler,
        Inst::BCond {
            cond: ncl_asm_aarch64::Cond::Ne,
            label: normal,
        },
    )?;
    let undefined_address = abi
        .runtime_address(RuntimeFunction::UndefinedFunction)
        .map_err(|error| CodegenError::Abi(error.to_string()))?;
    // check-added-lines: allow(unsupported) emit the undefined-function stub address.
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), undefined_address) {
        emit(assembler, instruction)?; // check-added-lines: allow(unsupported)
    }
    emit(assembler, Inst::B { label: call })?;
    assembler
        .bind(normal)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    lower_call(assembler, closure, args, allocation)?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(17)),
                offset: i16::try_from((ncl_object::function_offset::ENTRY + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
            },
        },
    )?;
    primitives::decode_function_entry(assembler, Reg(17))?;
    assembler
        .bind(call)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

#[cfg(test)]
#[allow(clippy::len_zero, missing_docs)]
mod tests {
    use super::*;
    use crate::{AbiError, Allocation, Location};
    use std::collections::BTreeMap;

    #[derive(Clone, Copy)]
    struct TestAbi {
        fail_undefined: bool,
    }

    impl RuntimeAbi for TestAbi {
        fn builtin_address(
            &self,
            identifier: ncl_object::BuiltinIdentifier,
        ) -> Result<u64, AbiError> {
            Err(AbiError::MissingBuiltin(identifier))
        }

        fn field_offset(&self, field: crate::ContextField) -> Result<i32, AbiError> {
            crate::Aarch64Abi.field_offset(field)
        }

        fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
            if self.fail_undefined && function == RuntimeFunction::UndefinedFunction {
                Err(AbiError::UnsupportedRuntimeFunction(function))
            } else {
                Ok(0x4000)
            }
        }
    }

    fn allocation() -> Allocation {
        Allocation {
            intervals: Vec::new(),
            locations: vec![
                (ValueId(0), Location::Register(1)),
                (ValueId(1), Location::Register(2)),
                (ValueId(2), Location::Register(3)),
                (ValueId(3), Location::Register(4)),
                (ValueId(4), Location::Register(5)),
                (ValueId(5), Location::Register(6)),
            ],
            spill_words: 0,
            safepoint_registers: BTreeMap::new(),
            outgoing_base: 0,
            incoming_args_base: None,
        }
    }

    fn assert_encodes(assembler: Assembler) {
        let bytes = assembler
            .finish()
            .unwrap_or_else(|error| panic!("AArch64 call encoding: {error:?}"))
            .bytes;
        assert!(!bytes.is_empty());
        assert_eq!(bytes.len() % 4, 0);
    }

    fn assert_first_instruction(bytes: &[u8], instruction: &Inst) {
        let actual = u32::from_le_bytes(
            bytes[..4]
                .try_into()
                .unwrap_or_else(|_| panic!("complete AArch64 instruction")),
        );
        let expected = ncl_asm_aarch64::encode(instruction, 0)
            .unwrap_or_else(|error| panic!("expected AArch64 encoding: {error:?}"));
        assert_eq!(actual, expected);
    }

    #[test]
    fn call_lowering_covers_register_and_outgoing_argument_abis() {
        let allocation = allocation();
        let mut assembler = Assembler::new();
        assert_eq!(
            lower_call(&mut assembler, ValueId(0), &[], &allocation),
            Err(CodegenError::Unsupported(
                "calls require a tagged argc argument".into()
            ))
        );

        let mut assembler = Assembler::new();
        lower_call(
            &mut assembler,
            ValueId(0),
            &[ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
            &allocation,
        )
        .unwrap_or_else(|error| panic!("register call: {error:?}"));
        let bytes = assembler
            .finish()
            .unwrap_or_else(|error| panic!("register call encoding: {error:?}"))
            .bytes;
        assert_first_instruction(
            &bytes,
            &Inst::Mov {
                rd: RegOrSp::Reg(Reg(16)),
                rn: RegOrSp::Reg(Reg(1)),
            },
        );

        let mut assembler = Assembler::new();
        lower_call(
            &mut assembler,
            ValueId(0),
            &[ValueId(1), ValueId(2), ValueId(3), ValueId(4), ValueId(5)],
            &allocation,
        )
        .unwrap_or_else(|error| panic!("outgoing call: {error:?}"));
        let bytes = assembler
            .finish()
            .unwrap_or_else(|error| panic!("outgoing call encoding: {error:?}"))
            .bytes;
        assert_eq!(bytes.len() % 4, 0);
        assert_first_instruction(
            &bytes,
            &Inst::Mov {
                rd: RegOrSp::Reg(Reg(16)),
                rn: RegOrSp::Reg(Reg(1)),
            },
        );
    }

    #[test]
    fn closure_calls_cover_direct_and_named_global_error_boundaries() {
        let allocation = allocation();
        let abi = TestAbi {
            fail_undefined: false,
        };
        let mut assembler = Assembler::new();
        lower_closure_call(
            &mut assembler,
            ValueId(0),
            &[ValueId(1), ValueId(2)],
            &allocation,
            None,
            &abi,
        )
        .unwrap_or_else(|error| panic!("direct closure call: {error:?}"));
        assert_encodes(assembler);

        let mut assembler = Assembler::new();
        lower_closure_call(
            &mut assembler,
            ValueId(0),
            &[ValueId(1), ValueId(2)],
            &allocation,
            Some(ValueId(1)),
            &abi,
        )
        .unwrap_or_else(|error| panic!("named closure call: {error:?}"));
        assert_encodes(assembler);

        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_closure_call(
                &mut assembler,
                ValueId(0),
                &[ValueId(1)],
                &allocation,
                Some(ValueId(1)),
                &TestAbi {
                    fail_undefined: true,
                },
            ),
            Err(CodegenError::Abi(_))
        ));
    }
}
