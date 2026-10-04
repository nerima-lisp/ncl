use super::*;
use crate::{AbiError, Allocation, Location, RuntimeAbi};
use ncl_ir::{Compare, Constant, Function, FunctionId, Op, OpKind, Ty, ValueId};
use std::collections::BTreeMap;

struct TestAbi;

impl RuntimeAbi for TestAbi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: crate::ContextField) -> Result<i32, AbiError> {
        Err(AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: crate::RuntimeFunction) -> Result<u64, AbiError> {
        Err(AbiError::UnsupportedRuntimeFunction(function))
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:7").then_some(0x2000)
    }
}

fn allocation() -> Allocation {
    Allocation {
        intervals: Vec::new(),
        locations: vec![
            (ValueId(0), Location::Register(1)),
            (ValueId(1), Location::Register(2)),
        ],
        spill_words: 0,
        safepoint_registers: BTreeMap::new(),
        outgoing_base: 0,
        incoming_args_base: None,
    }
}

fn instruction_texts(assembler: Assembler) -> Vec<String> {
    let bytes = assembler
        .finish()
        .unwrap_or_else(|error| panic!("AArch64 instruction encoding: {error:?}"))
        .bytes;
    ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &bytes, 0)
        .unwrap_or_else(|error| panic!("AArch64 disassembly: {error:?}"))
        .into_iter()
        .map(|instruction| instruction.text)
        .collect()
}

fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler
            .emit(&instruction)
            .unwrap_or_else(|error| panic!("expected AArch64 encoding: {error:?}"));
    }
    assembler
        .finish()
        .unwrap_or_else(|error| panic!("AArch64 instruction encoding: {error:?}"))
        .bytes
}

fn operation(kind: OpKind, result: Option<ValueId>) -> Op {
    Op {
        results: result.map_or_else(Vec::new, |value| vec![(value, Ty::Word)]),
        kind,
        loc: None,
    }
}

fn function() -> Function {
    Function {
        id: FunctionId(9),
        name: "aarch64-lowering-op-templates".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: vec![Constant::Fixnum(7)],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

#[test]
fn constant_word_covers_immediate_function_and_heap_boundaries() {
    let abi = TestAbi;
    for constant in [
        Constant::Fixnum(-3),
        Constant::Character('x' as u32),
        Constant::Nil,
        Constant::Unbound,
        Constant::T,
    ] {
        assert!(constant_word(&constant, &abi).is_ok());
    }
    assert_eq!(
        constant_word(&Constant::FunctionEntry(FunctionId(7)), &abi),
        Ok(0x2000)
    );
    assert_eq!(
        constant_word(&Constant::FunctionEntry(FunctionId(8)), &abi),
        Err(CodegenError::Unsupported(
            "function entry constant is unavailable".into()
        ))
    );
    assert_eq!(
        constant_word(&Constant::SingleFloat(1.0), &abi),
        Err(CodegenError::Unsupported(
            "constant requires a runtime table".into()
        ))
    );
}

#[test]
fn heap_address_helpers_reject_unrepresentable_offsets() {
    let mut assembler = Assembler::new();
    assert_eq!(
        load_heap_constant(&mut assembler, ncl_ir::ConstantIndex(u32::MAX)),
        Err(CodegenError::FrameOverflow)
    );
    let allocation = allocation();
    let mut assembler = Assembler::new();
    assert_eq!(
        store_closure_capture(
            &mut assembler,
            ValueId(0),
            usize::MAX,
            ValueId(1),
            &allocation,
        ),
        Err(CodegenError::FrameOverflow)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn lower_op_emits_exact_aarch64_memory_and_compare_templates() {
    let function = function();
    let abi = TestAbi;
    let lower = |kind, result| {
        let mut assembler = Assembler::new();
        lower_op(
            &mut assembler,
            &operation(kind, result),
            &function,
            &allocation(),
            &abi,
        )
        .unwrap_or_else(|error| panic!("AArch64 operation lowering: {error:?}"));
        instruction_texts(assembler)
    };

    assert_eq!(
        lower(OpKind::Move { value: ValueId(0) }, Some(ValueId(1))),
        ["orr x16, x31, x1, lsl #0", "orr x2, x31, x16, lsl #0"]
    );
    assert_eq!(
        lower(
            OpKind::Convert {
                op: ncl_ir::Convert::WordToI64,
                value: ValueId(0),
            },
            Some(ValueId(1)),
        ),
        ["orr x16, x31, x1, lsl #0", "orr x2, x31, x16, lsl #0"]
    );
    assert_eq!(
        lower(OpKind::LoadArg { index: 0 }, Some(ValueId(1))),
        ["orr x16, x31, x1, lsl #0", "orr x2, x31, x16, lsl #0"]
    );
    assert_eq!(
        lower(OpKind::LoadFunctionObject, Some(ValueId(1))),
        ["ldr x16, [x29, #16]", "orr x2, x31, x16, lsl #0"]
    );
    assert_eq!(
        lower(
            OpKind::Compare {
                op: Compare::Eq,
                left: ValueId(0),
                right: ValueId(1),
            },
            Some(ValueId(0)),
        ),
        [
            "orr x16, x31, x1, lsl #0",
            "orr x17, x31, x2, lsl #0",
            "subs sp, x16, x17, lsl #0",
            "csel x16, x31, x31, ne",
            "orr x1, x31, x16, lsl #0",
        ]
    );
    let load = lower(
        OpKind::Load {
            address: ValueId(0),
        },
        Some(ValueId(1)),
    );
    assert_eq!(
        load,
        [
            "orr x16, x31, x1, lsl #0",
            "and x16, x16, #0xfffffffffffffff8",
            "ldr x16, [x16, #0]",
            "orr x2, x31, x16, lsl #0",
        ]
    );
    let store = lower(
        OpKind::Store {
            address: ValueId(0),
            value: ValueId(1),
        },
        None,
    );
    assert_eq!(
        store,
        [
            "orr x16, x31, x1, lsl #0",
            "orr x17, x31, x2, lsl #0",
            "and x16, x16, #0xfffffffffffffff8",
            "str x17, [x16, #0]",
        ]
    );
    assert_eq!(
        lower(
            OpKind::LoadField {
                object: ValueId(0),
                field: 2,
            },
            Some(ValueId(1)),
        ),
        [
            "orr x16, x31, x1, lsl #0",
            "and x16, x16, #0xfffffffffffffff8",
            "ldr x16, [x16, #24]",
            "orr x2, x31, x16, lsl #0",
        ]
    );
    assert_eq!(
        lower(
            OpKind::StoreField {
                object: ValueId(0),
                field: 3,
                value: ValueId(1),
            },
            None,
        ),
        [
            "orr x16, x31, x1, lsl #0",
            "orr x17, x31, x2, lsl #0",
            "and x16, x16, #0xfffffffffffffff8",
            "str x17, [x16, #32]",
        ]
    );
}

#[test]
fn lower_op_move_matches_the_exact_encoded_register_copy() {
    let mut assembler = Assembler::new();
    lower_op(
        &mut assembler,
        &operation(OpKind::Move { value: ValueId(0) }, Some(ValueId(1))),
        &function(),
        &allocation(),
        &TestAbi,
    )
    .unwrap_or_else(|error| panic!("AArch64 move lowering: {error:?}"));

    assert_eq!(
        assembler
            .finish()
            .unwrap_or_else(|error| panic!("AArch64 instruction encoding: {error:?}"))
            .bytes,
        encoded([
            Inst::Mov {
                rd: RegOrSp::Reg(Reg(16)),
                rn: RegOrSp::Reg(Reg(1)),
            },
            Inst::Mov {
                rd: RegOrSp::Reg(Reg(2)),
                rn: RegOrSp::Reg(Reg(16)),
            },
        ])
    );
}
