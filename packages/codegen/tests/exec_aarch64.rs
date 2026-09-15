#![cfg(target_arch = "aarch64")]
#![allow(
    missing_docs,
    clippy::borrow_as_ptr,
    clippy::cast_sign_loss,
    clippy::expect_used
)]

use ncl_codegen::{RuntimeAbi, X86_64Abi, compile_function_aarch64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};
use ncl_sys::{Thread, alloc_code, invoke_entry, publish_code, write_code};

#[test]
fn executes_constant_return_in_published_code() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(0),
        "constant",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(42));
    let values = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("constant operation");
    builder
        .terminate(Terminator::Return { values })
        .expect("return terminator");
    let abi = X86_64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        0,
        [0; 4],
        0,
    );
    assert_eq!(value, abi.encode_fixnum(42) as u64);
    assert_eq!(count, 1);
}

#[test]
fn executes_fixnum_add_of_two_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(1),
        "add",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("left argument")[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right argument")[0];
    let values = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("add operation");
    builder
        .terminate(Terminator::Return { values })
        .expect("return terminator");
    let abi = X86_64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [
            abi.encode_fixnum(1) as u64,
            abi.encode_fixnum(2) as u64,
            0,
            0,
        ],
        0,
    );
    assert_eq!(value, abi.encode_fixnum(3) as u64);
    assert_eq!(count, 1);
}
