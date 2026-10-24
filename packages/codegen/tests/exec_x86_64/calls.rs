use super::*;

fn build_weighted_target() -> ncl_ir::Function {
    let mut target_builder = FunctionBuilder::new(
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
            target_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("target argument")[0]
        })
        .collect::<Vec<_>>();
    let mut result = word_to_i64(&mut target_builder, arguments[0]);
    for (index, argument) in arguments.iter().copied().enumerate().skip(1) {
        let mut weighted = word_to_i64(&mut target_builder, argument);
        for _ in 0..index {
            weighted = target_builder
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
        result = target_builder
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
    let result = i64_to_word(&mut target_builder, result);
    target_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("target return");
    target_builder.finish()
}

fn build_five_argument_entry() -> ncl_ir::Function {
    let mut entry_builder = FunctionBuilder::new(
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
    let callee_value = entry_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("target address")[0];
    let arguments = (1..=5)
        .map(|index| {
            entry_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("entry argument")[0]
        })
        .collect::<Vec<_>>();
    let argc = fixnum_word(&mut entry_builder, 5);
    entry_builder
        .push_op(OpKind::Safepoint, &[])
        .expect("call safepoint");
    let mut call_args = vec![argc];
    call_args.extend(arguments);
    let result = entry_builder
        .push_op(
            OpKind::Call {
                function: callee_value,
                args: call_args,
            },
            &[Ty::Word],
        )
        .expect("five-argument call")[0];
    entry_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("entry return");
    entry_builder.finish()
}

#[test]
fn executes_generated_call_with_fifth_argument_from_rest_area() {
    let target =
        compile_function_x86_64(&build_weighted_target(), &X86_64Abi).expect("target lowering");
    let mut target_code = alloc_code(target.code.len()).expect("target code allocation");
    write_code(&mut target_code, 0, &target.code).expect("target code write");
    publish_code(&mut target_code).expect("target code publication");

    let entry =
        compile_function_x86_64(&build_five_argument_entry(), &BuiltinAbi).expect("entry lowering");
    let mut entry_code = alloc_code(entry.code.len()).expect("entry code allocation");
    write_code(&mut entry_code, 0, &entry.code).expect("entry code write");
    publish_code(&mut entry_code).expect("entry code publication");

    let values = [1, 2, 3, 4, 5].map(|value| Word::fixnum(value).bits());
    let rest = [values[3], values[4]];
    let mut thread = Thread::new();
    let result = invoke_entry(
        &entry_code,
        entry.entry_offset as usize,
        &mut thread,
        6,
        [
            target_code.address() as u64,
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
                specialization: None,
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
