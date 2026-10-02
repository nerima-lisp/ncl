use super::compile_function_x86_64;
use crate::{AllocationTarget, CodegenError, X86_64Abi, allocate};
use ncl_asm_x86_64::{Assembler, BinOp, Inst, Mem, Reg};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Param, Terminator, Ty};

fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler.emit(&instruction).expect("instruction encoding");
    }
    assembler.bytes().to_vec()
}

#[test]
fn rejects_a_function_without_an_entry_block_with_a_typed_error() {
    let function = ncl_ir::Function {
        id: ncl_ir::FunctionId(200),
        name: "empty".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };

    assert_eq!(
        compile_function_x86_64(&function, &X86_64Abi),
        Err(CodegenError::EmptyFunction)
    );
}

#[test]
fn reports_an_unknown_jump_target_before_emitting_a_jump() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(201),
        "unknown-jump",
        Vec::new(),
        Vec::new(),
    );
    builder
        .terminate(Terminator::Jump {
            target: ncl_ir::BlockId(99),
            args: Vec::new(),
        })
        .expect("jump terminator");

    assert_eq!(
        compile_function_x86_64(&builder.finish(), &X86_64Abi),
        Err(CodegenError::UnknownBlock(ncl_ir::BlockId(99)))
    );
}

#[test]
fn reports_an_unknown_branch_target_after_lowering_the_condition() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(202),
        "unknown-branch",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
    );
    builder
        .terminate(Terminator::Branch {
            condition: ncl_ir::ValueId(0),
            then_target: ncl_ir::BlockId(99),
            then_args: Vec::new(),
            else_target: ncl_ir::BlockId(100),
            else_args: Vec::new(),
        })
        .expect("branch terminator");

    assert_eq!(
        compile_function_x86_64(&builder.finish(), &X86_64Abi),
        Err(CodegenError::UnknownBlock(ncl_ir::BlockId(99)))
    );
}

#[test]
fn reserves_only_argument_overflow_for_a_non_closure_call() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(203),
        "closure-without-definition",
        Vec::new(),
        Vec::new(),
    );
    let argc_index = builder.add_constant(Constant::Fixnum(5));
    let argc = builder
        .push_op(OpKind::Const { result: argc_index }, &[Ty::Word])
        .expect("argc constant")[0];
    builder
        .push_op(
            OpKind::CallClosure {
                closure: argc,
                args: vec![argc; 6],
                named_symbol: None,
            },
            &[],
        )
        .expect("closure call");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return terminator");

    let compiled =
        compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("closure call lowering");
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x4c, 0x89, 0x5d])
    );
    assert!(compiled.frame_size >= 8 * 8);
}

#[test]
fn generated_lambda_prologue_stages_rest_arguments_at_exact_incoming_slots() {
    let params = [
        "argc", "first", "second", "third", "fourth", "fifth", "sixth",
    ]
    .into_iter()
    .map(|name| Param {
        name: name.into(),
        ty: Ty::Word,
    })
    .collect::<Vec<_>>();
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(204),
        "generated-lambda-overflow-prologue",
        params,
        Vec::new(),
    );
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let (_, local_words) = super::lowering::slots(
        &function,
        u32::try_from(function.params.len()).expect("parameter count"),
        allocation.clone(),
        0,
    );
    let incoming_base = u32::try_from(function.params.len()).expect("parameter count")
        + local_words
        + allocation.spill_words;
    let compiled = compile_function_x86_64(&function, &X86_64Abi).expect("generated lambda");

    let encode = |instruction: Inst| {
        let mut assembler = Assembler::new();
        assembler.emit(&instruction).expect("instruction encoding");
        assembler.bytes().to_vec()
    };
    for (offset, expected) in [
        (
            0,
            encode(Inst::MovRM(
                super::lowering::ENTRY,
                Mem::base(super::lowering::REST_ARGUMENT, 0),
            )),
        ),
        (
            8,
            encode(Inst::MovRM(
                super::lowering::ENTRY,
                Mem::base(super::lowering::REST_ARGUMENT, 8),
            )),
        ),
    ] {
        assert!(
            compiled
                .code
                .windows(expected.len())
                .any(|bytes| bytes == expected),
            "generated-lambda overflow load at +{offset} missing from {:02x?}",
            compiled.code
        );
    }
    let expected_rest_slot = encode(Inst::MovMR(
        super::lowering::slot_mem_of(incoming_base + 4).expect("incoming rest slot"),
        Reg::R9,
    ));
    assert!(
        compiled
            .code
            .windows(expected_rest_slot.len())
            .any(|bytes| bytes == expected_rest_slot),
        "incoming rest register was not saved at slot {}: {:02x?}",
        incoming_base + 4,
        compiled.code
    );
}

#[test]
fn call_return_and_tail_call_end_with_their_exact_frame_transfers() {
    let mut call_return = FunctionBuilder::new(
        ncl_ir::FunctionId(205),
        "call-return-frame-transfer",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        Vec::new(),
    );
    let argc = call_return.add_constant(Constant::Fixnum(0));
    let argc = call_return
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc")[0];
    call_return
        .terminate(Terminator::CallReturn {
            function: ncl_ir::ValueId(0),
            args: vec![argc],
        })
        .expect("call return");
    let call_return =
        compile_function_x86_64(&call_return.finish(), &X86_64Abi).expect("call-return lowering");
    assert!(call_return.code.ends_with(&encoded([
        Inst::BinRI(BinOp::Add, Reg::Rsp, 16),
        Inst::MovRR(Reg::Rsp, Reg::Rbp),
        Inst::Pop(Reg::Rbp),
        Inst::Ret,
    ])));

    let mut tail_call = FunctionBuilder::new(
        ncl_ir::FunctionId(206),
        "tail-call-frame-transfer",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        Vec::new(),
    );
    let argc = tail_call.add_constant(Constant::Fixnum(0));
    let argc = tail_call
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc")[0];
    tail_call
        .terminate(Terminator::TailCall {
            function: ncl_ir::ValueId(0),
            args: vec![argc],
        })
        .expect("tail call");
    let tail_call =
        compile_function_x86_64(&tail_call.finish(), &X86_64Abi).expect("tail-call lowering");
    assert!(tail_call.code.ends_with(&encoded([
        Inst::MovRR(Reg::Rsp, Reg::Rbp),
        Inst::Pop(Reg::Rbp),
        Inst::MovRM(Reg::Rax, Mem::base(Reg::Rsp, 0)),
        Inst::MovMR(Mem::base(Reg::Rsp, 8), super::lowering::FUNCTION_OBJECT),
        Inst::MovMR(Mem::base(Reg::Rsp, 16), Reg::Rax),
        Inst::JmpReg(super::lowering::ENTRY),
    ])));
}
