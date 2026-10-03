use super::*;
use crate::{AbiError, Allocation, Location};
use ncl_ir::{BasicBlock, Constant, FunctionId, HandlerRegionId, Param};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy)]
struct TestAbi {
    fail_context: bool,
}

impl RuntimeAbi for TestAbi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        if self.fail_context {
            Err(AbiError::UnsupportedContextField(field))
        } else {
            Ok(8)
        }
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        Ok(0x1000 + u64::from(function as u8))
    }
}

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

fn function(regions: Vec<HandlerRegion>, blocks: Vec<BasicBlock>) -> Function {
    Function {
        id: FunctionId(1),
        name: "dispatch-test".into(),
        params: Vec::<Param>::new(),
        return_types: Vec::new(),
        blocks,
        locals: Vec::new(),
        constants: vec![Constant::Nil],
        handler_regions: regions,
        debug: Vec::new(),
    }
}

#[test]
fn multiple_values_and_epilogues_cover_empty_nonempty_and_overflow_forms() {
    let allocation = allocation();
    let abi = TestAbi {
        fail_context: false,
    };
    let mut assembler = Assembler::new();
    lower_set_multiple_values(
        &mut assembler,
        &[ValueId(0), ValueId(1)],
        Some(ValueId(2)),
        &allocation,
        &abi,
    )
    .unwrap_or_else(|error| panic!("multiple values: {error:?}"));
    let instructions = instruction_texts(assembler);
    assert_in_order(
        &instructions,
        &[
            "movz x1, #0x2, lsl #0",
            "orr x16, x31, x1, lsl #0",
            "orr x3, x31, x16, lsl #0",
            "str x16, [x21, #8]",
            "orr x16, x31, x2, lsl #0",
            "str x16, [x21, #16]",
            "str x1, [x21, #8]",
        ],
    );

    let mut assembler = Assembler::new();
    lower_set_multiple_values(&mut assembler, &[], None, &allocation, &abi)
        .unwrap_or_else(|error| panic!("empty multiple values: {error:?}"));
    let instructions = instruction_texts(assembler);
    assert_eq!(instructions, ["movz x1, #0x0, lsl #0", "str x1, [x21, #8]"]);
    let mut assembler = Assembler::new();
    assert!(matches!(
        lower_set_multiple_values(
            &mut assembler,
            &vec![ValueId(0); ncl_sys::MULTIPLE_VALUE_AREA_WORDS + 1],
            None,
            &allocation,
            &abi,
        ),
        Err(CodegenError::MultipleValueAreaOverflow { .. })
    ));

    let mut assembler = Assembler::new();
    emit_epilogue(&mut assembler, &allocation, &abi, 0, &[ValueId(0)], false)
        .unwrap_or_else(|error| panic!("ordinary epilogue: {error:?}"));
    let instructions = instruction_texts(assembler);
    assert_in_order(
        &instructions,
        &[
            "orr x16, x31, x1, lsl #0",
            "str x16, [x21, #8]",
            "orr x0, x31, x1, lsl #0",
            "movz x1, #0x1, lsl #0",
            "str x1, [x21, #8]",
            "ldp x29, x30, [sp], #32",
            "ret x30",
        ],
    );
    let mut assembler = Assembler::new();
    emit_epilogue(&mut assembler, &allocation, &abi, 16, &[], true)
        .unwrap_or_else(|error| panic!("propagating epilogue: {error:?}"));
    let instructions = instruction_texts(assembler);
    assert_eq!(
        instructions,
        [
            "movz x0, #0x0, lsl #0",
            "movz x1, #0x0, lsl #0",
            "add sp, sp, #16",
            "ldp x29, x30, [sp], #32",
            "ret x30",
        ]
    );
    let mut assembler = Assembler::new();
    assert!(matches!(
        emit_epilogue(
            &mut assembler,
            &allocation,
            &abi,
            0,
            &vec![ValueId(0); ncl_sys::MULTIPLE_VALUE_AREA_WORDS + 1],
            false,
        ),
        Err(CodegenError::MultipleValueAreaOverflow { .. })
    ));
    let mut assembler = Assembler::new();
    assert_eq!(
        emit_epilogue(&mut assembler, &allocation, &abi, 65_536, &[], false),
        Err(CodegenError::FrameOverflow)
    );
}

#[test]
fn dispatch_order_and_handler_error_boundaries_are_explicit() {
    let nested = HandlerRegion {
        id: HandlerRegionId(1),
        kind: HandlerKind::Catch,
        protected: vec![BlockId(0)],
        handler: BlockId(1),
        cleanup: None,
        catch_tag: Some(ValueId(0)),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    };
    let outer = HandlerRegion {
        id: HandlerRegionId(2),
        kind: HandlerKind::UnwindProtect,
        protected: vec![BlockId(0), BlockId(1)],
        handler: BlockId(2),
        cleanup: Some(BlockId(2)),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    };
    let dispatch_function = function(vec![outer.clone(), nested.clone()], Vec::new());
    let regions = covering_regions(&dispatch_function, BlockId(0));
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].id, HandlerRegionId(1));
    assert_eq!(regions[1].id, HandlerRegionId(2));

    let mut assembler = Assembler::new();
    let handler_label = assembler.new_label();
    let mut labels = HashMap::new();
    labels.insert(BlockId(1), handler_label);
    assert!(
        try_dispatch_candidates(
            &mut assembler,
            &dispatch_function,
            BlockId(0),
            &allocation(),
            &TestAbi {
                fail_context: false,
            },
            &labels,
        )
        .is_err()
    );

    let no_tag = HandlerRegion {
        id: HandlerRegionId(3),
        kind: HandlerKind::Catch,
        protected: vec![BlockId(0)],
        handler: BlockId(1),
        cleanup: None,
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    };
    let no_tag_function = function(
        vec![no_tag],
        vec![BasicBlock {
            id: BlockId(1),
            params: Vec::new(),
            ops: Vec::new(),
            terminator: ncl_ir::Terminator::Unreachable,
        }],
    );
    let mut assembler = Assembler::new();
    let handler_label = assembler.new_label();
    let mut labels = HashMap::new();
    labels.insert(BlockId(1), handler_label);
    try_dispatch_candidates(
        &mut assembler,
        &no_tag_function,
        BlockId(0),
        &allocation(),
        &TestAbi {
            fail_context: false,
        },
        &labels,
    )
    .unwrap_or_else(|error| panic!("catch without tag: {error:?}"));

    let empty_function = function(Vec::new(), Vec::new());
    let labels = HashMap::new();
    let mut assembler = Assembler::new();
    assert_eq!(
        lower_return_or_throw(
            &mut assembler,
            &empty_function,
            BlockId(0),
            Some(&[]),
            &allocation(),
            &TestAbi {
                fail_context: false,
            },
            65_536,
            &labels,
        ),
        Err(CodegenError::FrameOverflow)
    );
    let mut assembler = Assembler::new();
    assert!(matches!(
        lower_pending_check(
            &mut assembler,
            &empty_function,
            BlockId(0),
            &allocation(),
            &TestAbi { fail_context: true },
            0,
            &labels,
        ),
        Err(CodegenError::Unsupported(_))
    ));
    assert!(matches!(
        mv_area_mem(&TestAbi { fail_context: true }, 0,),
        Err(CodegenError::Abi(_))
    ));
    assert!(matches!(
        mv_area_mem(
            &TestAbi {
                fail_context: false
            },
            -2,
        ),
        Err(CodegenError::FrameOverflow)
    ));
}
