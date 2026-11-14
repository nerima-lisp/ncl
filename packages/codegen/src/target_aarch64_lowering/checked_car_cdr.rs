use super::{
    Allocation, CallCheckContext, CheckedCallMode, CodegenError, RuntimeAbi, RuntimeFunction, emit,
    emit_safepoint_call, load_value, lower_checked_call, store_value,
};
use crate::ConstantName;
use crate::machine_verify::SafepointSite;
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{DebugLocationId, Prim, ValueId};

fn move_immediate(
    assembler: &mut Assembler,
    register: Reg,
    value: u64,
) -> Result<(), CodegenError> {
    for instruction in ncl_asm_aarch64::mov_imm64(register, value) {
        emit(assembler, instruction)?;
    }
    Ok(())
}

fn branch(
    assembler: &mut Assembler,
    condition: Cond,
    label: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::BCond {
            cond: condition,
            label,
        },
    )
}

fn bind(assembler: &mut Assembler, label: ncl_asm_aarch64::Label) -> Result<(), CodegenError> {
    assembler
        .bind(label)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

#[derive(Clone, Copy)]
pub(super) struct CarCdrOperation<'a> {
    pub(super) prim: &'a Prim,
    pub(super) args: &'a [ValueId],
    pub(super) result: Option<ValueId>,
    pub(super) source_origin: Option<DebugLocationId>,
}

pub(super) fn lower(
    assembler: &mut Assembler,
    operation: CarCdrOperation<'_>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let [datum] = operation.args else {
        // check-added-lines: allow(unsupported) reject malformed primitive IR arity
        return Err(CodegenError::Unsupported(
            "CAR or CDR requires one operand".into(),
        ));
    };
    let slow = assembler.new_label();
    let nil = assembler.new_label();
    let done = assembler.new_label();

    emit_fast_path(assembler, &operation, *datum, allocation, nil, slow)?;
    emit(assembler, Inst::B { label: done })?;

    bind(assembler, nil)?;
    move_immediate(assembler, Reg(16), ncl_sys::Word::NIL.bits())?;
    if let Some(result) = operation.result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    emit(assembler, Inst::B { label: done })?;

    bind(assembler, slow)?;
    let checked = emit_type_error_signal(assembler, &operation, *datum, allocation, abi, context)?;
    if let Some(result) = operation.result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    bind(assembler, done)?;
    Ok(checked.site())
}

fn emit_fast_path(
    assembler: &mut Assembler,
    operation: &CarCdrOperation<'_>,
    datum: ValueId,
    allocation: &Allocation,
    nil: ncl_asm_aarch64::Label,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    load_value(assembler, allocation, datum, Reg(16))?;
    move_immediate(assembler, Reg(17), ncl_sys::Word::NIL.bits())?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(16),
            rm: Reg(17),
            shift: Shift::Lsl(0),
        },
    )?;
    branch(assembler, Cond::Eq, nil)?;

    emit_cons_tag_guard(assembler, slow)?;

    let offset = if matches!(operation.prim, Prim::Cdr) {
        8
    } else {
        0
    };
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
    if let Some(result) = operation.result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

fn emit_cons_tag_guard(
    assembler: &mut Assembler,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    // Character immediates share the cons lowtag, so combine both predicates before branching.
    emit(
        assembler,
        Inst::LsrImm {
            rd: Reg(17),
            rn: Reg(16),
            amount: 32,
        },
    )?;
    // Subtracting one makes a zero upper word negative for the combined sign check.
    emit(
        assembler,
        Inst::SubImm {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(17)),
            imm: 1,
            shift: false,
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(15),
            rn: Reg(16),
            imm: ncl_sys::LOWTAG_MASK,
        },
    )?;
    emit(
        assembler,
        Inst::EorImm {
            rd: Reg(15),
            rn: Reg(15),
            imm: 1,
        },
    )?;
    emit(
        assembler,
        Inst::Neg {
            rd: Reg(15),
            rn: Reg(15),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::Orr {
            rd: Reg(17),
            rn: Reg(17),
            rm: Reg(15),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(17),
            rm: Reg(31),
            shift: Shift::Lsl(0),
        },
    )?;
    branch(assembler, Cond::Mi, slow)?;
    Ok(())
}

fn emit_type_error_signal(
    assembler: &mut Assembler,
    operation: &CarCdrOperation<'_>,
    datum: ValueId,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<super::CheckedCall, CodegenError> {
    let origin = operation
        .source_origin
        .map_or(-1, |location| i64::from(location.0));
    let address = abi
        .runtime_address(RuntimeFunction::SignalTypeError)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) report a missing signal entry in the runtime ABI
    let expected_type_index = abi
        .constant_word_named(ConstantName::new("type-error-expected:LIST"))
        .and_then(|index| u32::try_from(index).ok())
        .map(ncl_ir::ConstantIndex)
        .ok_or_else(|| {
            CodegenError::Unsupported("LIST type-error constant is unavailable".into()) // check-added-lines: allow(unsupported) the code object must carry LIST for TYPE-ERROR
        })?;
    lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            emit(
                assembler,
                Inst::Mov {
                    rd: RegOrSp::Reg(Reg(0)),
                    rn: RegOrSp::Reg(Reg(21)),
                },
            )?;
            load_value(assembler, allocation, datum, Reg(1))?;
            super::ops::load_heap_constant(assembler, expected_type_index)?;
            emit(
                assembler,
                Inst::Mov {
                    rd: RegOrSp::Reg(Reg(2)),
                    rn: RegOrSp::Reg(Reg(16)),
                },
            )?;
            move_immediate(assembler, Reg(3), u64::from_ne_bytes(origin.to_ne_bytes()))?;
            move_immediate(assembler, Reg(17), address)?;
            emit_safepoint_call(assembler)
        },
    )
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Lowering tests use named setup failures and assert emitted instructions or errors."
)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::{AbiError, AllocationTarget, ContextField, RuntimeAbi};
    use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Param, Terminator, Ty};

    #[derive(Clone, Copy, Default)]
    struct TestAbi {
        missing_signal_entry: bool,
        missing_list_constant: bool,
    }

    impl RuntimeAbi for TestAbi {
        fn builtin_address(
            &self,
            _identifier: ncl_object::BuiltinIdentifier,
        ) -> Result<u64, AbiError> {
            Ok(0x1000)
        }

        fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
            let layout = ncl_sys::thread_layout();
            let offset = match field {
                ContextField::TlabBump => layout.tlab_bump,
                ContextField::TlabLimit => layout.tlab_limit,
                ContextField::SafepointRequest => layout.safepoint_request,
                ContextField::Pending => layout.pending,
                ContextField::MultipleValueCount => layout.mv_count,
                ContextField::MultipleValueArea => layout.mv,
                ContextField::Handler => layout.handler,
                ContextField::Cleanup => layout.cleanup,
                ContextField::Catch => layout.catch,
                ContextField::RuntimeErrorFlags => layout.runtime_error_flags,
            };
            i32::try_from(offset).map_err(|_| AbiError::UnsupportedContextField(field))
        }

        fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
            if self.missing_signal_entry && function == RuntimeFunction::SignalTypeError {
                return Err(AbiError::UnsupportedRuntimeFunction(function));
            }
            Ok(0x1000)
        }

        fn constant_word(&self, name: &str) -> Option<i64> {
            (name == "type-error-expected:LIST" && !self.missing_list_constant).then_some(0)
        }
    }

    fn lower_for_test(
        prim: &Prim,
        args: &[ValueId],
        with_result: bool,
        source_origin: Option<DebugLocationId>,
        abi: &dyn RuntimeAbi,
    ) -> Result<(Vec<u8>, SafepointSite), CodegenError> {
        let params = if args.is_empty() {
            Vec::new()
        } else {
            vec![Param {
                name: "datum".into(),
                ty: Ty::Word,
            }]
        };
        let mut builder =
            FunctionBuilder::new(FunctionId(1200), "checked-car-cdr-test", params, Vec::new());
        let result = with_result.then(|| {
            builder
                .push_op(
                    OpKind::Prim {
                        op: prim.clone(),
                        args: vec![ValueId(0)],
                        condition: None,
                    },
                    &[Ty::Word],
                )
                .expect("allocate a result value")[0]
        });
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .expect("terminate test function");
        let function = builder.finish();
        let allocation = crate::allocate(&function, AllocationTarget::AArch64);
        let labels = HashMap::new();
        let context = CallCheckContext {
            function: &function,
            block: ncl_ir::BlockId(0),
            body_bytes: 0,
            labels: &labels,
        };
        let mut assembler = Assembler::new();
        let site = lower(
            &mut assembler,
            CarCdrOperation {
                prim,
                args,
                result,
                source_origin,
            },
            &allocation,
            abi,
            &context,
        )?;
        let bytes = assembler
            .finish()
            .map_err(|error| CodegenError::Encode(error.to_string()))?
            .bytes;
        Ok((bytes, site))
    }

    fn encode(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
        let mut assembler = Assembler::new();
        for instruction in instructions {
            assembler
                .emit(&instruction)
                .expect("encode expected instruction");
        }
        assembler
            .finish()
            .expect("finish expected instructions")
            .bytes
    }

    fn assert_contains(code: &[u8], expected: &[u8], message: &str) {
        assert!(
            code.windows(expected.len())
                .any(|window| window == expected),
            "{message}: expected {expected:02x?} in {code:02x?}"
        );
    }

    #[test]
    fn car_and_cdr_lowering_load_their_cons_fields() {
        for (prim, offset) in [(Prim::Car, 0), (Prim::Cdr, 8)] {
            let (code, _) = lower_for_test(&prim, &[ValueId(0)], true, None, &TestAbi::default())
                .expect("lower checked cons access");
            let expected = encode([Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(16)),
                    offset,
                },
            }]);
            assert_contains(
                &code,
                &expected,
                "lowering emits the selected cons field load",
            );
        }
    }

    #[test]
    fn lowering_handles_no_result_and_encodes_source_origin() {
        for (source_origin, encoded_origin) in
            [(Some(DebugLocationId(73)), 73_u64), (None, u64::MAX)]
        {
            let (code, _) = lower_for_test(
                &Prim::Car,
                &[ValueId(0)],
                false,
                source_origin,
                &TestAbi::default(),
            )
            .expect("lower checked cons access without a result");
            let expected = encode(ncl_asm_aarch64::mov_imm64(Reg(3), encoded_origin));
            assert_contains(&code, &expected, "signal call carries the source origin");
        }
    }

    #[test]
    fn malformed_operand_arity_returns_the_specific_error() {
        let error = lower_for_test(&Prim::Car, &[], false, None, &TestAbi::default())
            .expect_err("CAR requires one operand");
        assert_eq!(
            error,
            CodegenError::Unsupported("CAR or CDR requires one operand".into())
        );
    }

    #[test]
    fn missing_type_error_signal_entry_is_reported() {
        let error = lower_for_test(
            &Prim::Car,
            &[ValueId(0)],
            false,
            None,
            &TestAbi {
                missing_signal_entry: true,
                missing_list_constant: false,
            },
        )
        .expect_err("missing signal entry must fail lowering");
        assert_eq!(
            error,
            CodegenError::Unsupported(
                AbiError::UnsupportedRuntimeFunction(RuntimeFunction::SignalTypeError).to_string()
            )
        );
    }

    #[test]
    fn missing_list_constant_is_reported() {
        let error = lower_for_test(
            &Prim::Cdr,
            &[ValueId(0)],
            false,
            None,
            &TestAbi {
                missing_signal_entry: false,
                missing_list_constant: true,
            },
        )
        .expect_err("missing LIST type constant must fail lowering");
        assert_eq!(
            error,
            CodegenError::Unsupported("LIST type-error constant is unavailable".into())
        );
    }
}
