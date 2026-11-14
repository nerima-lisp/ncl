use std::collections::HashMap;

use ncl_asm_x86_64::{Assembler, BinOp, Inst, Mem, Reg};
use ncl_ir::{Constant, DebugLocationId, FunctionBuilder, FunctionId, OpKind, Param, Prim, Terminator, Ty};

use super::tests::empty_slots;
use super::{CheckedCarCdr, lower_checked_car_cdr};
use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AbiError, CodegenError, RuntimeAbi, RuntimeFunction, compile_function_x86_64};

#[derive(Clone, Copy)]
struct CheckedCarCdrAbi {
    missing_signal: bool,
    missing_expected_type: bool,
}

impl RuntimeAbi for CheckedCarCdrAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, AbiError> {
        X86_64FixtureAbi.builtin_address(identifier)
    }

    fn field_offset(&self, field: crate::ContextField) -> Result<i32, AbiError> {
        X86_64FixtureAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        if self.missing_signal && function == RuntimeFunction::SignalTypeError {
            return Err(AbiError::UnsupportedRuntimeFunction(function));
        }
        X86_64FixtureAbi.runtime_address(function)
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        if self.missing_expected_type && name == "type-error-expected:LIST" {
            return None;
        }
        X86_64FixtureAbi.constant_word(name)
    }
}

fn checked_car_cdr_function(
    id: u32,
    prim: Prim,
    argument_count: usize,
    source_origin: Option<DebugLocationId>,
) -> ncl_ir::Function {
    let params = (0..argument_count)
        .map(|index| Param {
            name: format!("arg{index}"),
            ty: Ty::Word,
        })
        .collect();
    let mut builder = FunctionBuilder::new(
        FunctionId(id),
        "checked-car-cdr",
        params,
        vec![Ty::Word],
    );
    builder.add_constant(Constant::Symbol {
        package: "COMMON-LISP".into(),
        name: "LIST".into(),
    });
    let args = (0..argument_count)
        .map(|index| {
            builder
                .push_op(
                    OpKind::LoadArg {
                        index: u8::try_from(index).expect("test argument index fits"),
                    },
                    &[Ty::Word],
                )
                .expect("load argument")[0]
        })
        .collect();
    let result = builder
        .push_op(
            OpKind::Prim {
                op: prim,
                args,
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("checked CAR/CDR result")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return checked CAR/CDR result");

    let mut function = builder.finish();
    let operation = function.blocks[0]
        .ops
        .iter_mut()
        .find(|op| matches!(&op.kind, OpKind::Prim { .. }))
        .expect("primitive operation");
    operation.loc = source_origin;
    function
}

fn encode(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler
            .emit(&instruction)
            .expect("expected x86-64 instruction encoding");
    }
    assembler.bytes().to_vec()
}

fn assert_contains(code: &[u8], expected: &[u8], behavior: &str) {
    assert!(
        code.windows(expected.len())
            .any(|window| window == expected),
        "{behavior}: expected {expected:02x?} in {code:02x?}"
    );
}

#[test]
fn checked_car_and_cdr_emit_nil_and_their_cons_field_loads() {
    let abi = CheckedCarCdrAbi {
        missing_signal: false,
        missing_expected_type: false,
    };
    for (id, prim, field_offset) in [(121, Prim::Car, 0), (122, Prim::Cdr, 8)] {
        let function = checked_car_cdr_function(id, prim, 1, None);
        let compiled = compile_function_x86_64(&function, &abi).expect("lowering");

        assert_contains(
            &compiled.code,
            &encode([Inst::CmpRI(Reg::R10, ncl_sys::Word::NIL.bits() as i32)]),
            "CAR/CDR must branch on the canonical NIL word",
        );
        assert_contains(
            &compiled.code,
            &encode([Inst::MovRI(
                Reg::R10,
                ncl_asm_x86_64::Imm::I32(ncl_sys::Word::NIL.bits() as i32),
            )]),
            "CAR/CDR must return NIL on the NIL branch",
        );
        assert_contains(
            &compiled.code,
            &encode([
                Inst::BinRI(BinOp::And, Reg::R10, -8),
                Inst::MovRM(Reg::R10, Mem::base(Reg::R10, field_offset)),
            ]),
            "CAR/CDR must load the selected cons field after removing its lowtag",
        );
        assert_contains(
            &compiled.code,
            &encode([Inst::MovRI(Reg::Rcx, ncl_asm_x86_64::Imm::I32(-1))]),
            "CAR/CDR without an IR location must pass the absent-origin sentinel",
        );
    }
}

#[test]
fn checked_car_cdr_signal_call_receives_context_type_and_source_origin() {
    let abi = CheckedCarCdrAbi {
        missing_signal: false,
        missing_expected_type: false,
    };
    let function = checked_car_cdr_function(123, Prim::Car, 1, Some(DebugLocationId(37)));
    let compiled = compile_function_x86_64(&function, &abi).expect("lowering");

    assert_contains(
        &compiled.code,
        &encode([Inst::MovRR(Reg::Rdi, crate::isa_x86_64::THREAD_CONTEXT)]),
        "type-error call must pass the pinned thread context",
    );
    assert_contains(
        &compiled.code,
        &encode([Inst::MovRR(Reg::Rdx, Reg::R10)]),
        "type-error call must pass the full expected type word",
    );
    assert_contains(
        &compiled.code,
        &encode([Inst::MovRI(Reg::Rcx, ncl_asm_x86_64::Imm::I32(37))]),
        "type-error call must pass the operation's debug location id",
    );
    let signal_address = encode([Inst::MovRI(
        Reg::R11,
        ncl_asm_x86_64::Imm::I32(0x1000),
    )]);
    let signal_call = encode([Inst::CallReg(Reg::R11)]);
    let address_position = compiled
        .code
        .windows(signal_address.len())
        .position(|window| window == signal_address);
    let call_position = compiled
        .code
        .windows(signal_call.len())
        .position(|window| window == signal_call);
    assert!(
        matches!((address_position, call_position), (Some(address), Some(call)) if address < call),
        "type-error signal must be called through its runtime address"
    );
}

#[test]
fn checked_car_cdr_reject_bad_arity_and_missing_signal_abi() {
    for (id, prim) in [(124, Prim::Car), (125, Prim::Cdr)] {
        let function = checked_car_cdr_function(id, prim.clone(), 1, None);
        let slots = empty_slots();
        let abi = CheckedCarCdrAbi {
            missing_signal: false,
            missing_expected_type: false,
        };
        let labels = HashMap::new();
        let context = super::super::dispatch::CallCheckContext::new(
            &function,
            function.blocks[0].id,
            &slots,
            &abi,
            &labels,
        );
        let malformed_result = lower_checked_car_cdr(
            &mut Assembler::new(),
            CheckedCarCdr {
                prim: &prim,
                args: &[],
                result: None,
                source_origin: None,
            },
            &slots,
            &abi,
            &context,
        );
        assert!(
            matches!(
                &malformed_result,
                Err(CodegenError::Unsupported(message))
                    if message == "CAR or CDR requires one operand"
            ),
            "unexpected malformed CAR/CDR result: {malformed_result:?}"
        );

        let function = checked_car_cdr_function(id + 10, prim, 1, None);
        assert!(matches!(
            compile_function_x86_64(&function, &CheckedCarCdrAbi {
                missing_signal: false,
                missing_expected_type: true,
            }),
            Err(CodegenError::Unsupported(message))
                if message == "LIST type-error constant is unavailable"
        ));
        assert!(matches!(
            compile_function_x86_64(&function, &CheckedCarCdrAbi {
                missing_signal: true,
                missing_expected_type: false,
            }),
            Err(CodegenError::Unsupported(message))
                if message == "runtime address is unavailable: SignalTypeError"
        ));
    }
}
