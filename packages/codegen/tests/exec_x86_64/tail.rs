use super::*;

#[test]
fn executes_tail_transfer_with_the_regular_callee_frame_abi() {
    let mut target_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(91),
        "tail-callee",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = target_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("callee value")[0];
    target_builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("callee return");
    let abi = X86_64Abi;
    let target = compile_function_x86_64(&target_builder.finish(), &abi).expect("callee lowering");
    let mut target_code = alloc_code(target.code.len()).expect("callee code allocation");
    write_code(&mut target_code, 0, &target.code).expect("callee code write");
    publish_code(&mut target_code).expect("callee code publication");

    let mut entry_builder = FunctionBuilder::new(
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
    let target_value = entry_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Address])
        .expect("callee argument")[0];
    let value = entry_builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("value argument")[0];
    entry_builder
        .terminate(Terminator::TailCall {
            function: target_value,
            args: vec![target_value, value],
        })
        .expect("tail call");
    let entry = compile_function_x86_64(&entry_builder.finish(), &abi).expect("caller lowering");
    let mut entry_code = alloc_code(entry.code.len()).expect("caller code allocation");
    write_code(&mut entry_code, 0, &entry.code).expect("caller code write");
    publish_code(&mut entry_code).expect("caller code publication");

    let mut thread = Thread::new();
    let value = Word::fixnum(37).bits();
    let result = invoke_entry(
        &entry_code,
        entry.entry_offset as usize,
        &mut thread,
        2,
        [target_code.address() as u64, value, 0, 0],
        0,
    );
    assert_eq!(result, (value, 1));
}
