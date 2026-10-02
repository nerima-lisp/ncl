use crate::{
    Allocation, CodegenError, ContextField, Location, RuntimeAbi, RuntimeFunction,
    common_lisp_builtin,
};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Constant, ConstantIndex, ValueId};

pub(super) fn constant_table_entry(
    constants: &[Constant],
    index: ConstantIndex,
) -> Result<&Constant, CodegenError> {
    let raw_index = index.0;
    let index = usize::try_from(raw_index).map_err(|_| CodegenError::InvalidConstantIndex {
        index: raw_index,
        length: constants.len(),
    })?;
    constants
        .get(index)
        .ok_or(CodegenError::InvalidConstantIndex {
            index: raw_index,
            length: constants.len(),
        })
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

#[path = "target_aarch64_lowering/offsets.rs"]
mod offsets;
pub(super) use offsets::{spill_offset, spill_slot_offset};

pub(super) fn load_value(
    assembler: &mut Assembler,
    allocation: &Allocation,
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    match allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    {
        Location::Register(source) => emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(register),
                rn: RegOrSp::Reg(Reg(
                    u8::try_from(source).map_err(|_| CodegenError::FrameOverflow)?
                )),
            },
        ),
        Location::Spill(_) => {
            let offset = spill_slot_offset(allocation, value)?;
            if offset <= 4095 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(register),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: offset,
                        shift: false,
                    },
                )?;
            } else {
                let offset_register = if register == Reg(17) {
                    Reg(16)
                } else {
                    Reg(17)
                };
                for instruction in ncl_asm_aarch64::mov_imm64(offset_register, u64::from(offset)) {
                    emit(assembler, instruction)?;
                }
                emit(
                    assembler,
                    Inst::Sub {
                        rd: RegOrSp::Reg(register),
                        rn: RegOrSp::Reg(Reg(29)),
                        rm: offset_register,
                        shift: ncl_asm_aarch64::Shift::Lsl(0),
                    },
                )?;
            }
            emit(
                assembler,
                Inst::Ldr {
                    rt: register,
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(register),
                        offset: 0,
                    },
                },
            )
        }
    }
}

pub(super) fn store_value(
    assembler: &mut Assembler,
    allocation: &Allocation,
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    match allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    {
        Location::Register(destination) => emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(Reg(
                    u8::try_from(destination).map_err(|_| CodegenError::FrameOverflow)?
                )),
                rn: RegOrSp::Reg(register),
            },
        ),
        Location::Spill(_) => {
            let offset = spill_slot_offset(allocation, value)?;
            let source = if register == Reg(16) {
                emit(
                    assembler,
                    Inst::Mov {
                        rd: RegOrSp::Reg(Reg(17)),
                        rn: RegOrSp::Reg(Reg(16)),
                    },
                )?;
                Reg(17)
            } else {
                register
            };
            if offset <= 4095 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(Reg(16)),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: offset,
                        shift: false,
                    },
                )?;
            } else {
                for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(offset)) {
                    emit(assembler, instruction)?;
                }
                emit(
                    assembler,
                    Inst::Sub {
                        rd: RegOrSp::Reg(Reg(16)),
                        rn: RegOrSp::Reg(Reg(29)),
                        rm: Reg(16),
                        shift: ncl_asm_aarch64::Shift::Lsl(0),
                    },
                )?;
            }
            emit(
                assembler,
                Inst::Str {
                    rt: source,
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset: 0,
                    },
                },
            )
        }
    }
}

pub(super) fn lower_runtime_builtin(
    assembler: &mut Assembler,
    function: RuntimeFunction,
    immediate_args: &[u64],
    value_args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if immediate_args.len() + value_args.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 runtime calls support at most four arguments".into(),
        ));
    }
    let address = abi
        .runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), address) {
        emit(assembler, instruction)?;
    }
    for (index, value) in immediate_args.iter().copied().enumerate() {
        for instruction in ncl_asm_aarch64::mov_imm64(
            Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
            value,
        ) {
            emit(assembler, instruction)?;
        }
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        load_value(
            assembler,
            allocation,
            value,
            Reg(u8::try_from(immediate_args.len() + index + 1)
                .map_err(|_| CodegenError::FrameOverflow)?),
        )?;
    }
    Ok(())
}

fn context_mem(abi: &dyn RuntimeAbi, field: ContextField) -> Result<MemOperand, CodegenError> {
    let offset = abi
        .field_offset(field)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    let offset = u16::try_from(offset).map_err(|_| CodegenError::FrameOverflow)?;
    Ok(MemOperand::Unsigned {
        base: RegOrSp::Reg(Reg(21)),
        offset,
        scale: 8,
    })
}

fn runtime_address(abi: &dyn RuntimeAbi, function: RuntimeFunction) -> Result<u64, CodegenError> {
    abi.runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))
}

fn lower_alloc(
    assembler: &mut Assembler,
    words: u32,
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<u32, CodegenError> {
    let bytes = words
        .checked_mul(8)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or(CodegenError::FrameOverflow)?;
    let slow = assembler.new_label();
    let done = assembler.new_label();
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: context_mem(abi, ContextField::TlabBump)?,
        },
    )?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: context_mem(abi, ContextField::TlabLimit)?,
        },
    )?;
    emit(
        assembler,
        Inst::AddImm {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(16)),
            imm: bytes,
            shift: false,
        },
    )?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(0),
            rm: Reg(17),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::BCond {
            cond: Cond::Hi,
            label: slow,
        },
    )?;
    emit(
        assembler,
        Inst::Str {
            rt: Reg(0),
            mem: context_mem(abi, ContextField::TlabBump)?,
        },
    )?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    emit(assembler, Inst::B { label: done })?;
    assembler
        .bind(slow)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), u64::from(words)) {
        emit(assembler, instruction)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(
        Reg(17),
        runtime_address(abi, RuntimeFunction::AllocateSlow)?,
    ) {
        emit(assembler, instruction)?;
    }
    emit(assembler, Inst::Blr { rn: Reg(17) })?;
    let call_pc = u32::try_from(assembler.offset()).map_err(|_| CodegenError::FrameOverflow)?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    assembler
        .bind(done)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(call_pc)
}

fn lower_safepoint(assembler: &mut Assembler, abi: &dyn RuntimeAbi) -> Result<u32, CodegenError> {
    let done = assembler.new_label();
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: context_mem(abi, ContextField::SafepointRequest)?,
        },
    )?;
    emit(
        assembler,
        Inst::Cbz {
            rt: Reg(16),
            label: done,
        },
    )?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(1)),
            rn: RegOrSp::Reg(Reg(29)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(
        Reg(17),
        runtime_address(abi, RuntimeFunction::SafepointSlow)?,
    ) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Adr {
            rd: Reg(2),
            label: done,
        },
    )?;
    emit(assembler, Inst::Blr { rn: Reg(17) })?;
    let call_pc = u32::try_from(assembler.offset()).map_err(|_| CodegenError::FrameOverflow)?;
    assembler
        .bind(done)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(call_pc)
}

fn lower_builtin(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if args.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 builtins support at most four arguments".into(),
        ));
    }
    if name == "make-rest-list" {
        // check-added-lines: allow(index) intentional
        let [argc_value, start_value] = args else {
            // check-added-lines: allow(unsupported) intentional
            return Err(CodegenError::Unsupported(
                "make-rest-list requires argc and start".into(), // check-added-lines: allow(unsupported) intentional
            ));
        };
        emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(Reg(0)),
                rn: RegOrSp::Reg(Reg(21)),
            },
        )?;
        if let Some(base) = allocation.incoming_args_base {
            let offset = base
                .checked_add(1)
                .and_then(|slot| slot.checked_mul(8))
                .ok_or(CodegenError::FrameOverflow)?;
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(offset)) {
                emit(assembler, instruction)?;
            }
            emit(
                assembler,
                Inst::Sub {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(29)),
                    rm: Reg(16),
                    shift: Shift::Lsl(0),
                },
            )?;
            // check-added-lines: allow(index) fixed five-register ABI table.
            for (index, register) in [Reg(1), Reg(2), Reg(3), Reg(4), Reg(5)]
                .into_iter()
                .enumerate()
            {
                emit(
                    assembler,
                    Inst::Ldr {
                        rt: register,
                        mem: MemOperand::Unscaled {
                            base: RegOrSp::Reg(Reg(16)),
                            offset: spill_offset(index)?,
                        },
                    },
                )?;
            }
        }
        load_value(assembler, allocation, *argc_value, Reg(6))?;
        load_value(assembler, allocation, *start_value, Reg(7))?;
        for instruction in ncl_asm_aarch64::mov_imm64(
            Reg(17),
            abi.builtin_address(common_lisp_builtin(name))
                .map_err(|error| CodegenError::Unsupported(error.to_string()))?, // check-added-lines: allow(unsupported) ABI address lookup failure.
        ) {
            emit(assembler, instruction)?;
        }
        return Ok(());
    }
    let address = abi
        .builtin_address(common_lisp_builtin(name))
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for (index, argument) in args.iter().enumerate() {
        let register = Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?);
        load_value(assembler, allocation, *argument, register)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), address) {
        emit(assembler, instruction)?;
    }
    Ok(())
}

#[path = "target_aarch64_lowering/calls.rs"]
pub(super) mod calls;
#[path = "target_aarch64_lowering/dispatch.rs"]
pub(super) mod dispatch;
#[path = "target_aarch64_lowering/moves.rs"]
mod moves;
#[path = "target_aarch64_lowering/ops.rs"]
pub(super) mod ops;
pub(super) use moves::move_args;
#[path = "target_aarch64_lowering/primitives.rs"]
pub(super) mod primitives;
pub(super) use calls::{lower_call, lower_closure_call};
pub(super) use dispatch::{lower_pending_check, lower_return_or_throw};
pub(super) use ops::lower_op;

#[cfg(test)]
#[allow(clippy::too_many_lines, missing_docs)]
mod tests {
    use super::*;
    use crate::{AbiError, Allocation, Location};
    use std::collections::BTreeMap;

    #[derive(Clone, Copy)]
    struct TestAbi {
        fail_builtin: bool,
        fail_context: bool,
        fail_runtime: bool,
        context_offset: i32,
    }

    impl TestAbi {
        const fn working() -> Self {
            Self {
                fail_builtin: false,
                fail_context: false,
                fail_runtime: false,
                context_offset: 8,
            }
        }
    }

    impl RuntimeAbi for TestAbi {
        fn builtin_address(
            &self,
            identifier: ncl_object::BuiltinIdentifier,
        ) -> Result<u64, AbiError> {
            if self.fail_builtin {
                Err(AbiError::MissingBuiltin(identifier))
            } else {
                Ok(0x1000)
            }
        }

        fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
            if self.fail_context {
                Err(AbiError::UnsupportedContextField(field))
            } else {
                Ok(self.context_offset)
            }
        }

        fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
            if self.fail_runtime {
                Err(AbiError::UnsupportedRuntimeFunction(function))
            } else {
                Ok(0x2000)
            }
        }
    }

    fn allocation() -> Allocation {
        Allocation {
            intervals: Vec::new(),
            locations: vec![
                (ValueId(0), Location::Register(1)),
                (ValueId(1), Location::Spill(0)),
                (ValueId(2), Location::Spill(600)),
            ],
            spill_words: 601,
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

    fn instruction_texts(assembler: Assembler) -> Vec<String> {
        let bytes = encoded(assembler);
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &bytes, 0)
            .unwrap_or_else(|error| panic!("AArch64 disassembly: {error:?}"))
            .into_iter()
            .map(|instruction| instruction.text)
            .collect()
    }

    fn assert_in_order(actual: &[String], expected: &[&str]) {
        let mut position = 0;
        for expected_instruction in expected {
            let Some(found) = actual[position..]
                .iter()
                .position(|instruction| instruction == expected_instruction)
            else {
                panic!("missing {expected_instruction:?} in {actual:?}");
            };
            position += found + 1;
        }
    }

    #[test]
    fn lowering_helpers_cover_spills_runtime_calls_and_abi_boundaries() {
        let allocation = allocation();
        let abi = TestAbi::working();

        for register in [Reg(16), Reg(17)] {
            let mut assembler = Assembler::new();
            load_value(&mut assembler, &allocation, ValueId(2), register)
                .unwrap_or_else(|error| panic!("long spill load: {error:?}"));
            let instructions = instruction_texts(assembler);
            let expected = if register == Reg(16) {
                [
                    "movz x17, #0x12c8, lsl #0",
                    "sub x16, x29, x17, lsl #0",
                    "ldr x16, [x16, #0]",
                ]
            } else {
                [
                    "movz x16, #0x12c8, lsl #0",
                    "sub x17, x29, x16, lsl #0",
                    "ldr x17, [x17, #0]",
                ]
            };
            assert_in_order(&instructions, &expected);
        }
        let mut assembler = Assembler::new();
        store_value(&mut assembler, &allocation, ValueId(2), Reg(16))
            .unwrap_or_else(|error| panic!("long spill store: {error:?}"));
        let instructions = instruction_texts(assembler);
        assert_in_order(
            &instructions,
            &[
                "orr x17, x31, x16, lsl #0",
                "movz x16, #0x12c8, lsl #0",
                "sub x16, x29, x16, lsl #0",
                "str x17, [x16, #0]",
            ],
        );

        let mut assembler = Assembler::new();
        lower_runtime_builtin(
            &mut assembler,
            RuntimeFunction::MakeValueCell,
            &[1, 2],
            &[ValueId(0), ValueId(1)],
            &allocation,
            &abi,
        )
        .unwrap_or_else(|error| panic!("runtime builtin: {error:?}"));
        let instructions = instruction_texts(assembler);
        assert_in_order(
            &instructions,
            &[
                "orr x0, x31, x21, lsl #0",
                "movz x17, #0x2000, lsl #0",
                "movz x1, #0x1, lsl #0",
                "movz x2, #0x2, lsl #0",
                "orr x3, x31, x1, lsl #0",
                "sub x4, x29, #8",
                "ldr x4, [x4, #0]",
            ],
        );
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_runtime_builtin(
                &mut assembler,
                RuntimeFunction::MakeValueCell,
                &[1, 2, 3, 4],
                &[ValueId(0)],
                &allocation,
                &abi,
            ),
            Err(CodegenError::Unsupported(message))
                if message == "AArch64 runtime calls support at most four arguments"
        ));
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_runtime_builtin(
                &mut assembler,
                RuntimeFunction::MakeValueCell,
                &[],
                &[],
                &allocation,
                &TestAbi {
                    fail_runtime: true,
                    ..abi
                },
            ),
            Err(CodegenError::Unsupported(_))
        ));

        assert!(context_mem(&abi, ContextField::Pending).is_ok());
        assert!(matches!(
            context_mem(
                &TestAbi {
                    fail_context: true,
                    ..abi
                },
                ContextField::Pending
            ),
            Err(CodegenError::Unsupported(_))
        ));
        assert!(matches!(
            context_mem(
                &TestAbi {
                    context_offset: -1,
                    ..abi
                },
                ContextField::Pending
            ),
            Err(CodegenError::FrameOverflow)
        ));
        assert!(runtime_address(&abi, RuntimeFunction::Unwind).is_ok());
        assert!(matches!(
            runtime_address(
                &TestAbi {
                    fail_runtime: true,
                    ..abi
                },
                RuntimeFunction::Unwind
            ),
            Err(CodegenError::Unsupported(_))
        ));

        let mut assembler = Assembler::new();
        let call_pc = lower_alloc(&mut assembler, 2, Some(ValueId(0)), &allocation, &abi)
            .unwrap_or_else(|error| panic!("allocation lowering: {error:?}"));
        assert!(call_pc > 0);
        let instructions = instruction_texts(assembler);
        assert_in_order(
            &instructions,
            &[
                "ldr x16, [x21, #8]",
                "ldr x17, [x21, #8]",
                "add x0, x16, #16",
                "subs sp, x0, x17, lsl #0",
                "orr x0, x31, x21, lsl #0",
                "movz x1, #0x2, lsl #0",
                "movz x17, #0x2000, lsl #0",
                "blr x17",
            ],
        );
        let mut assembler = Assembler::new();
        assert_eq!(
            lower_alloc(&mut assembler, u32::MAX, None, &allocation, &abi),
            Err(CodegenError::FrameOverflow)
        );

        let mut assembler = Assembler::new();
        let call_pc = lower_safepoint(&mut assembler, &abi)
            .unwrap_or_else(|error| panic!("safepoint lowering: {error:?}"));
        assert!(call_pc > 0);
        let instructions = instruction_texts(assembler);
        assert_in_order(
            &instructions,
            &[
                "ldr x16, [x21, #8]",
                "orr x0, x31, x21, lsl #0",
                "orr x1, x31, x29, lsl #0",
                "movz x17, #0x2000, lsl #0",
                "blr x17",
            ],
        );
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_safepoint(
                &mut assembler,
                &TestAbi {
                    fail_runtime: true,
                    ..abi
                },
            ),
            Err(CodegenError::Unsupported(_))
        ));

        let mut assembler = Assembler::new();
        lower_builtin(&mut assembler, "identity", &[ValueId(0)], &allocation, &abi)
            .unwrap_or_else(|error| panic!("builtin lowering: {error:?}"));
        let instructions = instruction_texts(assembler);
        assert_in_order(
            &instructions,
            &[
                "orr x0, x31, x21, lsl #0",
                "orr x1, x31, x1, lsl #0",
                "movz x17, #0x1000, lsl #0",
            ],
        );
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_builtin(
                &mut assembler,
                "identity",
                &[ValueId(0), ValueId(0), ValueId(0), ValueId(0), ValueId(0)],
                &allocation,
                &abi,
            ),
            Err(CodegenError::Unsupported(_))
        ));
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_builtin(&mut assembler, "make-rest-list", &[], &allocation, &abi),
            Err(CodegenError::Unsupported(message))
                if message == "make-rest-list requires argc and start"
        ));
        let mut assembler = Assembler::new();
        let mut generated_lambda = allocation.clone();
        generated_lambda.incoming_args_base = Some(0);
        lower_builtin(
            &mut assembler,
            "make-rest-list",
            &[ValueId(0), ValueId(1)],
            &generated_lambda,
            &abi,
        )
        .unwrap_or_else(|error| panic!("rest builtin lowering: {error:?}"));
        let instructions = instruction_texts(assembler);
        assert_in_order(
            &instructions,
            &[
                "orr x0, x31, x21, lsl #0",
                "movz x16, #0x8, lsl #0",
                "sub x16, x29, x16, lsl #0",
                "ldr x1, [x16, #0]",
                "ldr x2, [x16, #-8]",
                "ldr x3, [x16, #-16]",
                "ldr x4, [x16, #-24]",
                "ldr x5, [x16, #-32]",
                "orr x6, x31, x1, lsl #0",
                "sub x7, x29, #8",
                "ldr x7, [x7, #0]",
                "movz x17, #0x1000, lsl #0",
            ],
        );
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_builtin(
                &mut assembler,
                "identity",
                &[ValueId(0)],
                &allocation,
                &TestAbi {
                    fail_builtin: true,
                    ..abi
                },
            ),
            Err(CodegenError::Unsupported(_))
        ));
    }

    #[test]
    fn constant_table_entry_distinguishes_valid_and_invalid_indices() {
        let constants = vec![Constant::Nil];
        assert!(matches!(
            constant_table_entry(&constants, ConstantIndex(0)),
            Ok(Constant::Nil)
        ));
        assert_eq!(
            constant_table_entry(&constants, ConstantIndex(1)),
            Err(CodegenError::InvalidConstantIndex {
                index: 1,
                length: 1,
            })
        );
    }
}
