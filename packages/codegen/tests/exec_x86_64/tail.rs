use super::*;

#[test]
fn executes_tail_transfer_with_the_regular_callee_frame_abi() {
    let mut callee_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(91),
        "tail-callee",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = callee_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("callee value")[0];
    callee_builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("callee return");
    let abi = X86_64Abi;
    let callee = compile_function_x86_64(&callee_builder.finish(), &abi).expect("callee lowering");
    let mut callee_code = alloc_code(callee.code.len()).expect("callee code allocation");
    write_code(&mut callee_code, 0, &callee.code).expect("callee code write");
    publish_code(&mut callee_code).expect("callee code publication");

    let mut caller_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(92),
        "tail-caller",
        vec![
            Param {
                name: "callee".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let callee_value = caller_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Address])
        .expect("callee argument")[0];
    let value = caller_builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("value argument")[0];
    caller_builder
        .terminate(Terminator::TailCall {
            function: callee_value,
            args: vec![callee_value, value],
        })
        .expect("tail call");
    let caller = compile_function_x86_64(&caller_builder.finish(), &abi).expect("caller lowering");
    let mut caller_code = alloc_code(caller.code.len()).expect("caller code allocation");
    write_code(&mut caller_code, 0, &caller.code).expect("caller code write");
    publish_code(&mut caller_code).expect("caller code publication");

    let mut thread = Thread::new();
    let value = abi.encode_fixnum(37) as u64;
    let result = invoke_entry(
        &caller_code,
        caller.entry_offset as usize,
        &mut thread,
        2,
        [callee_code.address() as u64, value, 0, 0],
        0,
    );
    assert_eq!(result, (value, 1));
}
