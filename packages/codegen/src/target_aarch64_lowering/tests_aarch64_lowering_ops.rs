use super::*;
use crate::{AbiError, Allocation, Location, RuntimeAbi};
use ncl_ir::{Constant, FunctionId};
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
