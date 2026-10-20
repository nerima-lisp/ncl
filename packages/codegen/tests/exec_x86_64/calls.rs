use super::*;

#[test]
fn executes_generated_call_with_fifth_argument_from_rest_area() {
    let mut callee_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(100),
        "weighted-five-argument-callee",
        ["first", "second", "third", "fourth", "fifth"]
            .into_iter()
            .map(|name| Param {
                name: name.into(),
                ty: Ty::Word,
            })
            .collect(),
        vec![Ty::Word],
    );
    let arguments = (0..5)
        .map(|index| {
            callee_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("callee argument")[0]
        })
        .collect::<Vec<_>>();
    let mut result = word_to_i64(&mut callee_builder, arguments[0]);
    for (index, argument) in arguments.iter().copied().enumerate().skip(1) {
        let mut weighted = word_to_i64(&mut callee_builder, argument);
        for _ in 0..index {
            weighted = callee_builder
                .push_op(
                    OpKind::Prim {
                        op: Prim::FixnumAdd,
                        args: vec![weighted, weighted],
                        condition: None,
                    },
                    &[Ty::I64],
                )
                .expect("double weighted argument")[0];
        }
        result = callee_builder
            .push_op(
                OpKind::Prim {
                    op: Prim::FixnumAdd,
                    args: vec![result, weighted],
                    condition: None,
                },
                &[Ty::I64],
            )
            .expect("add weighted argument")[0];
    }
    let result = i64_to_word(&mut callee_builder, result);
    callee_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("callee return");
    let callee =
        compile_function_x86_64(&callee_builder.finish(), &X86_64Abi).expect("callee lowering");
    let mut callee_code = alloc_code(callee.code.len()).expect("callee code allocation");
    write_code(&mut callee_code, 0, &callee.code).expect("callee code write");
    publish_code(&mut callee_code).expect("callee code publication");

    let mut caller_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(101),
        "five-argument-caller",
        vec![
            Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            Param {
                name: "first".into(),
                ty: Ty::Word,
            },
            Param {
                name: "second".into(),
                ty: Ty::Word,
            },
            Param {
                name: "third".into(),
                ty: Ty::Word,
            },
            Param {
                name: "fourth".into(),
                ty: Ty::Word,
            },
            Param {
                name: "fifth".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let callee_value = caller_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("callee address")[0];
    let arguments = (1..=5)
        .map(|index| {
            caller_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("caller argument")[0]
        })
        .collect::<Vec<_>>();
    let argc = fixnum_word(&mut caller_builder, 5);
    caller_builder
        .push_op(OpKind::Safepoint, &[])
        .expect("call safepoint");
    let mut call_args = vec![argc];
    call_args.extend(arguments);
    let result = caller_builder
        .push_op(
            OpKind::Call {
                function: callee_value,
                args: call_args,
            },
            &[Ty::Word],
        )
        .expect("five-argument call")[0];
    caller_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("caller return");
    let caller =
        compile_function_x86_64(&caller_builder.finish(), &BuiltinAbi).expect("caller lowering");
    let mut caller_code = alloc_code(caller.code.len()).expect("caller code allocation");
    write_code(&mut caller_code, 0, &caller.code).expect("caller code write");
    publish_code(&mut caller_code).expect("caller code publication");

    let values = [1, 2, 3, 4, 5].map(|value| Word::fixnum(value).bits());
    let rest = [values[3], values[4]];
    let mut thread = Thread::new();
    let result = invoke_entry(
        &caller_code,
        caller.entry_offset as usize,
        &mut thread,
        6,
        [
            callee_code.address() as u64,
            values[0],
            values[1],
            values[2],
        ],
        rest.as_ptr() as u64,
    );
    assert_eq!(result, (Word::fixnum(129).bits(), 1));
}

#[test]
fn rejects_builtin_missing_from_execution_fixture_abi() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(102),
        "missing-builtin",
        Vec::new(),
        vec![Ty::Word],
    );
    builder
        .push_op(OpKind::Safepoint, &[])
        .expect("builtin safepoint");
    let value = builder
        .push_op(
            OpKind::Builtin {
                name: "unregistered".into(),
                args: Vec::new(),
            },
            &[Ty::Word],
        )
        .expect("builtin operation")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");

    assert!(matches!(
        compile_function_x86_64(&builder.finish(), &BuiltinAbi),
        Err(ncl_codegen::CodegenError::Unsupported(message))
            if message.contains("builtin address is unavailable")
    ));
}
