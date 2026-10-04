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
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
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
    lower_runtime_builtin(
        &mut assembler,
        RuntimeFunction::MakeValueCell,
        &[1, 2, 3, 4],
        &[ValueId(0)],
        &allocation,
        &abi,
    )
    .unwrap_or_else(|error| panic!("runtime builtin overflow: {error:?}"));
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
    lower_builtin(
        &mut assembler,
        "identity",
        &[ValueId(0), ValueId(0), ValueId(0), ValueId(0), ValueId(0)],
        &allocation,
        &abi,
    )
    .unwrap_or_else(|error| panic!("builtin overflow: {error:?}"));
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
