use super::{lower_op, lower_prim, move_args};
use crate::{Allocation, AllocationTarget, CodegenError, Location, X86_64Abi, allocate};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Mem, Reg};
use ncl_ir::{
    BlockParam, Compare, Constant, Convert, Function, FunctionBuilder, Op, OpKind, Param, Prim,
    Terminator, Ty, ValueId,
};
use std::collections::BTreeMap;

fn one_word_slots() -> super::super::ValueSlots {
    let function = FunctionBuilder::new(
        ncl_ir::FunctionId(213),
        "one-word-slots",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
    )
    .finish();
    super::super::slots(
        &function,
        1,
        allocate(&function, AllocationTarget::X86_64),
        0,
    )
    .0
}

fn slots_with_locations(
    locations: &[(ValueId, Location)],
    spill_base: u32,
) -> super::super::ValueSlots {
    super::super::ValueSlots {
        values: Vec::new(),
        allocation: Allocation {
            intervals: Vec::new(),
            locations: locations.to_vec(),
            spill_words: 2,
            safepoint_registers: BTreeMap::new(),
            outgoing_base: 0,
            incoming_args_base: None,
        },
        spill_base,
        outgoing_base: 0,
        incoming_args_base: None,
    }
}

fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler
            .emit(&instruction)
            .expect("expected instruction encoding");
    }
    assembler.bytes().to_vec()
}

fn lower_exact(
    kind: OpKind,
    result: Option<ValueId>,
    function: &Function,
    slots: &super::super::ValueSlots,
) -> Vec<u8> {
    let op = Op {
        results: result.into_iter().map(|value| (value, Ty::Word)).collect(),
        kind,
        loc: None,
    };
    let mut assembler = Assembler::new();
    lower_op(&mut assembler, &op, function, slots, &X86_64Abi).expect("operation lowering");
    assembler.bytes().to_vec()
}

#[test]
fn unsupported_primitives_return_typed_errors_after_loading_operands() {
    let slots = one_word_slots();
    for prim in [
        Prim::FixnumDiv,
        Prim::Typep,
        Prim::CharacterPredicate("characterp".into()),
        Prim::StructureSlot("slot".into()),
    ] {
        let mut assembler = Assembler::new();
        let result = lower_prim(
            &mut assembler,
            &prim,
            &[ValueId(0), ValueId(0)],
            None,
            &slots,
        );
        assert!(matches!(
            result,
            Err(CodegenError::Unsupported(message))
                if message.contains("primitive is not available")
        ));
    }
}

#[test]
fn primitive_without_operands_reports_the_operand_error_without_emitting_bytes() {
    let slots = one_word_slots();
    let mut assembler = Assembler::new();

    assert_eq!(
        lower_prim(&mut assembler, &Prim::FixnumAdd, &[], None, &slots,),
        Err(CodegenError::Unsupported(
            "primitive has no operands".into()
        ))
    );
    assert_eq!(assembler.bytes(), &[]);
}

#[test]
fn supported_memory_primitives_emit_exact_operand_offsets() {
    let slots = one_word_slots();
    for (prim, expected_instruction) in [
        (
            Prim::Car,
            Inst::MovRM(super::FUNCTION_OBJECT, Mem::base(super::FUNCTION_OBJECT, 0)),
        ),
        (
            Prim::Cdr,
            Inst::MovRM(super::FUNCTION_OBJECT, Mem::base(super::FUNCTION_OBJECT, 8)),
        ),
        (
            Prim::Svref,
            Inst::MovRM(super::FUNCTION_OBJECT, Mem::base(super::FUNCTION_OBJECT, 0)),
        ),
        (
            Prim::Aref,
            Inst::MovRM(super::FUNCTION_OBJECT, Mem::base(super::FUNCTION_OBJECT, 0)),
        ),
        (
            Prim::Rplaca,
            Inst::MovMR(Mem::base(super::FUNCTION_OBJECT, 0), super::ENTRY),
        ),
        (
            Prim::Rplacd,
            Inst::MovMR(Mem::base(super::FUNCTION_OBJECT, 8), super::ENTRY),
        ),
        (
            Prim::Aset,
            Inst::MovMR(Mem::base(super::FUNCTION_OBJECT, 0), super::ENTRY),
        ),
    ] {
        let mut actual = Assembler::new();
        lower_prim(&mut actual, &prim, &[ValueId(0), ValueId(0)], None, &slots)
            .expect("supported memory primitive");
        let mut expected = Assembler::new();
        expected
            .emit(&expected_instruction)
            .expect("expected memory instruction encoding");
        assert!(
            actual.bytes().ends_with(expected.bytes()),
            "unexpected encoding for {prim:?}: {:02x?}",
            actual.bytes()
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn operation_lowering_emits_exact_load_store_and_compare_templates() {
    let function = FunctionBuilder::new(
        ncl_ir::FunctionId(217),
        "operation-templates",
        Vec::new(),
        Vec::new(),
    )
    .finish();
    let slots = slots_with_locations(
        &[
            (ValueId(0), Location::Register(0)),
            (ValueId(1), Location::Register(1)),
            (ValueId(2), Location::Register(4)),
        ],
        0,
    );

    assert_eq!(
        lower_exact(
            OpKind::Move { value: ValueId(0) },
            Some(ValueId(1)),
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::MovRR(Reg::Rdx, super::FUNCTION_OBJECT),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::Convert {
                op: Convert::WordToI64,
                value: ValueId(0),
            },
            Some(ValueId(1)),
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::MovRR(Reg::Rdx, super::FUNCTION_OBJECT),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::Load {
                address: ValueId(0),
            },
            Some(ValueId(1)),
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::BinRI(BinOp::And, super::FUNCTION_OBJECT, -8),
            Inst::MovRM(super::FUNCTION_OBJECT, Mem::base(super::FUNCTION_OBJECT, 0),),
            Inst::MovRR(Reg::Rdx, super::FUNCTION_OBJECT),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::LoadField {
                object: ValueId(0),
                field: 2,
            },
            Some(ValueId(1)),
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::BinRI(BinOp::And, super::FUNCTION_OBJECT, -8),
            Inst::MovRM(
                super::FUNCTION_OBJECT,
                Mem::base(super::FUNCTION_OBJECT, 24),
            ),
            Inst::MovRR(Reg::Rdx, super::FUNCTION_OBJECT),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
            None,
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::BinRI(BinOp::And, super::FUNCTION_OBJECT, -8),
            Inst::MovRR(super::ENTRY, Reg::Rdx),
            Inst::MovMR(Mem::base(super::FUNCTION_OBJECT, 0), super::ENTRY,),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::StoreField {
                object: ValueId(0),
                field: 3,
                value: ValueId(1),
            },
            None,
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::BinRI(BinOp::And, super::FUNCTION_OBJECT, -8),
            Inst::MovRR(super::ENTRY, Reg::Rdx),
            Inst::MovMR(Mem::base(super::FUNCTION_OBJECT, 32), super::ENTRY,),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::LoadArg { index: 0 },
            Some(ValueId(1)),
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::MovRR(Reg::Rdx, super::FUNCTION_OBJECT),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::LoadFunctionObject,
            Some(ValueId(1)),
            &function,
            &slots
        ),
        encoded([
            Inst::MovRM(super::FUNCTION_OBJECT, Mem::base(super::FRAME_POINTER, 16),),
            Inst::MovRR(Reg::Rdx, super::FUNCTION_OBJECT),
        ])
    );
    assert_eq!(
        lower_exact(
            OpKind::Compare {
                op: Compare::Eq,
                left: ValueId(0),
                right: ValueId(1),
            },
            Some(ValueId(2)),
            &function,
            &slots,
        ),
        encoded([
            Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax),
            Inst::MovRR(super::ENTRY, Reg::Rdx),
            Inst::CmpRR(super::FUNCTION_OBJECT, super::ENTRY),
            Inst::Setcc(Cond::E, super::FUNCTION_OBJECT),
            Inst::Movzx(super::FUNCTION_OBJECT, super::FUNCTION_OBJECT, 8),
            Inst::MovRR(Reg::Rcx, super::FUNCTION_OBJECT),
        ])
    );

    assert_eq!(
        lower_exact(OpKind::Move { value: ValueId(0) }, None, &function, &slots),
        encoded([Inst::MovRR(super::FUNCTION_OBJECT, Reg::Rax)])
    );
}

#[test]
fn operation_errors_keep_constant_handler_and_copy_contracts_typed() {
    let empty = FunctionBuilder::new(
        ncl_ir::FunctionId(214),
        "operation-errors",
        Vec::new(),
        Vec::new(),
    )
    .finish();
    let slots = super::super::slots(&empty, 0, allocate(&empty, AllocationTarget::X86_64), 0).0;
    let mut assembler = Assembler::new();
    let invalid_constant = Op {
        results: Vec::new(),
        kind: OpKind::Const {
            result: ncl_ir::ConstantIndex(4),
        },
        loc: None,
    };
    assert!(matches!(
        lower_op(
            &mut assembler,
            &invalid_constant,
            &empty,
            &slots,
            &X86_64Abi,
        ),
        Err(CodegenError::Unsupported(message))
            if message.contains("constant index out of range")
    ));

    let mut handler_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(215),
        "missing-cleanup",
        Vec::new(),
        Vec::new(),
    );
    let region = ncl_ir::HandlerRegionId(0);
    handler_builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
        kind: ncl_ir::HandlerKind::UnwindProtect,
        protected: vec![ncl_ir::BlockId(0)],
        handler: ncl_ir::BlockId(0),
        cleanup: None,
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    handler_builder
        .push_op(OpKind::EnterHandler { region }, &[])
        .expect("enter handler");
    handler_builder
        .terminate(Terminator::Unreachable)
        .expect("unreachable");
    let handler_function = handler_builder.finish();
    let handler_slots = super::super::slots(
        &handler_function,
        0,
        allocate(&handler_function, AllocationTarget::X86_64),
        0,
    )
    .0;
    let mut handler_assembler = Assembler::new();
    let handler_op = &handler_function.blocks[0].ops[0];
    assert!(matches!(
        lower_op(
            &mut handler_assembler,
            handler_op,
            &handler_function,
            &handler_slots,
            &X86_64Abi,
        ),
        Err(CodegenError::Unsupported(message))
            if message.contains("cleanup block is unavailable")
    ));

    let too_many = Op {
        results: Vec::new(),
        kind: OpKind::SetMultipleValues {
            values: (0..=ncl_sys::MULTIPLE_VALUE_AREA_WORDS)
                .map(|value| ValueId(u32::try_from(value).expect("value id")))
                .collect(),
        },
        loc: None,
    };
    assert!(matches!(
        lower_op(&mut assembler, &too_many, &empty, &slots, &X86_64Abi,),
        Err(CodegenError::MultipleValueAreaOverflow { .. })
    ));

    assert!(matches!(
        move_args(
            &mut assembler,
            &slots,
            &[],
            &[BlockParam {
                value: ValueId(0),
                ty: Ty::Word,
            }],
        ),
        Err(CodegenError::Unsupported(message))
            if message.contains("block argument arity mismatch")
    ));
}

#[test]
fn indirect_calls_emit_the_same_machine_call_template_as_direct_calls() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(216),
        "indirect-call",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        Vec::new(),
    );
    let argc_index = builder.add_constant(Constant::Fixnum(0));
    let argc = builder
        .push_op(OpKind::Const { result: argc_index }, &[Ty::Word])
        .expect("argc constant")[0];
    builder
        .push_op(
            OpKind::CallIndirect {
                callee: ValueId(0),
                args: vec![argc],
            },
            &[],
        )
        .expect("indirect call");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");

    let compiled = crate::compile_function_x86_64(&builder.finish(), &X86_64Abi)
        .expect("indirect call lowering");
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x41, 0xff, 0xd3])
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn move_args_emits_exact_parallel_copies_for_register_and_spill_cycles() {
    let register_slots = slots_with_locations(
        &[
            (ValueId(0), Location::Register(0)),
            (ValueId(1), Location::Register(1)),
        ],
        0,
    );
    let register_params = [
        BlockParam {
            value: ValueId(1),
            ty: Ty::Word,
        },
        BlockParam {
            value: ValueId(0),
            ty: Ty::Word,
        },
    ];
    let mut register_actual = Assembler::new();
    move_args(
        &mut register_actual,
        &register_slots,
        &[ValueId(0), ValueId(1)],
        &register_params,
    )
    .expect("register parallel copy");
    let mut register_expected = Assembler::new();
    for instruction in [
        Inst::MovRR(super::ENTRY, Reg::Rax),
        Inst::Push(super::ENTRY),
        Inst::MovRR(super::ENTRY, Reg::Rdx),
        Inst::Push(super::ENTRY),
        Inst::Pop(super::ENTRY),
        Inst::MovRR(Reg::Rax, super::ENTRY),
        Inst::Pop(super::ENTRY),
        Inst::MovRR(Reg::Rdx, super::ENTRY),
    ] {
        register_expected
            .emit(&instruction)
            .expect("register expected encoding");
    }
    assert_eq!(register_actual.bytes(), register_expected.bytes());

    let spill_slots = slots_with_locations(
        &[
            (ValueId(0), Location::Spill(0)),
            (ValueId(1), Location::Spill(1)),
        ],
        4,
    );
    let mut spill_actual = Assembler::new();
    move_args(
        &mut spill_actual,
        &spill_slots,
        &[ValueId(0), ValueId(1)],
        &register_params,
    )
    .expect("spill parallel copy");
    let mut spill_expected = Assembler::new();
    for instruction in [
        Inst::MovRM(super::ENTRY, Mem::base(super::FRAME_POINTER, -40)),
        Inst::Push(super::ENTRY),
        Inst::MovRM(super::ENTRY, Mem::base(super::FRAME_POINTER, -48)),
        Inst::Push(super::ENTRY),
        Inst::Pop(super::ENTRY),
        Inst::MovMR(Mem::base(super::FRAME_POINTER, -40), super::ENTRY),
        Inst::Pop(super::ENTRY),
        Inst::MovMR(Mem::base(super::FRAME_POINTER, -48), super::ENTRY),
    ] {
        spill_expected
            .emit(&instruction)
            .expect("spill expected encoding");
    }
    assert_eq!(spill_actual.bytes(), spill_expected.bytes());

    let non_overlapping_slots = slots_with_locations(
        &[
            (ValueId(0), Location::Register(0)),
            (ValueId(1), Location::Register(1)),
        ],
        0,
    );
    let mut non_overlapping_actual = Assembler::new();
    move_args(
        &mut non_overlapping_actual,
        &non_overlapping_slots,
        &[ValueId(0)],
        &[BlockParam {
            value: ValueId(1),
            ty: Ty::Word,
        }],
    )
    .expect("non-overlapping copy");
    let mut non_overlapping_expected = Assembler::new();
    non_overlapping_expected
        .emit(&Inst::MovRR(super::ENTRY, Reg::Rax))
        .expect("non-overlapping load encoding");
    non_overlapping_expected
        .emit(&Inst::MovRR(Reg::Rdx, super::ENTRY))
        .expect("non-overlapping store encoding");
    assert_eq!(
        non_overlapping_actual.bytes(),
        non_overlapping_expected.bytes()
    );
}
