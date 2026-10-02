use super::compile_function_x86_64;
use crate::{CodegenError, X86_64Abi};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Param, Terminator, Ty};

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
