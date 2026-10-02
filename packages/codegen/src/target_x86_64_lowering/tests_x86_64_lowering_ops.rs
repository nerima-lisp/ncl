use super::{lower_op, lower_prim, move_args};
use crate::{AllocationTarget, CodegenError, X86_64Abi, allocate};
use ncl_asm_x86_64::Assembler;
use ncl_ir::{
    BlockParam, Constant, FunctionBuilder, Op, OpKind, Param, Prim, Terminator, Ty, ValueId,
};

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
