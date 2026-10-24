use super::*;
fn weighted_target_builder() -> FunctionBuilder {
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
            let argument = target_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("callee argument")[0];
            convert(&mut target_builder, Convert::WordToI64, argument, Ty::I64)
        })
        .collect::<Vec<_>>();
    let mut weighted_sum = arguments[0];
    for (index, argument) in arguments.iter().copied().enumerate().skip(1) {
        let mut weighted_argument = argument;
        for _ in 0..index {
            weighted_argument = target_builder
                .push_op(
                    OpKind::Prim {
                        op: Prim::FixnumAdd,
                        args: vec![weighted_argument, weighted_argument],
                        condition: None,
                    },
                    &[Ty::I64],
                )
                .expect("double weighted argument")[0];
        }
        weighted_sum = target_builder
            .push_op(
                OpKind::Prim {
                    op: Prim::FixnumAdd,
                    args: vec![weighted_sum, weighted_argument],
                    condition: None,
                },
                &[Ty::I64],
            )
            .expect("add weighted argument")[0];
    }
    let weighted_sum = convert(
        &mut target_builder,
        Convert::I64ToWord,
        weighted_sum,
        Ty::Word,
    );
    target_builder
        .terminate(Terminator::Return {
            values: vec![weighted_sum],
        })
        .expect("target function return");
    target_builder
}

fn five_argument_invoker_builder() -> FunctionBuilder {
    let mut invoker_builder = FunctionBuilder::new(
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
    let target_address = invoker_builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("target address")[0];
    let arguments = (1..=5)
        .map(|index| {
            invoker_builder
                .push_op(OpKind::LoadArg { index }, &[Ty::Word])
                .expect("invoker argument")[0]
        })
        .collect::<Vec<_>>();
    let argc = fixnum_word(&mut invoker_builder, 5);
    invoker_builder
        .push_op(OpKind::Safepoint, &[])
        .expect("call safepoint");
    let mut call_args = vec![argc];
    call_args.extend(arguments);
    let result = invoker_builder
        .push_op(
            OpKind::Call {
                function: target_address,
                args: call_args,
            },
            &[Ty::Word],
        )
        .expect("five-argument call")[0];
    invoker_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("invoker return");
    invoker_builder
}

#[test]
fn executes_generated_call_with_fifth_argument_from_rest_area() {
    let target_function =
        compile_function_aarch64(&weighted_target_builder().finish(), &Aarch64Abi)
            .expect("target function lowering");
    let mut target_code = alloc_code(target_function.code.len()).expect("target code allocation");
    write_code(&mut target_code, 0, &target_function.code).expect("target code write");
    publish_code(&mut target_code).expect("target code publication");

    let invoker_function =
        compile_function_aarch64(&five_argument_invoker_builder().finish(), &BuiltinAbi)
            .expect("invoker lowering");
    let mut invoker_code =
        alloc_code(invoker_function.code.len()).expect("invoker code allocation");
    write_code(&mut invoker_code, 0, &invoker_function.code).expect("invoker code write");
    publish_code(&mut invoker_code).expect("invoker code publication");

    let values = [1, 2, 3, 4, 5].map(|value| Word::fixnum(value).bits());
    let rest = [values[3], values[4]];
    let mut thread = Thread::new();
    let result = invoke_entry(
        &invoker_code,
        invoker_function.entry_offset as usize,
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
fn executes_switch_with_negative_and_wide_i64_cases_and_default() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(102),
        "switch-cases-and-default",
        vec![Param {
            name: "selector".into(),
            ty: Ty::I64,
        }],
        vec![Ty::Word],
    );
    let selector = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::I64])
        .expect("switch selector")[0];
    let negative_case = -1_i64;
    let wide_case = 1_i64 << 40;
    let case_one_value = fixnum_word(&mut builder, 101);
    let case_two_value = fixnum_word(&mut builder, 202);
    let default_value = fixnum_word(&mut builder, 303);
    let case_one_result = builder.fresh_value();
    let case_one = builder.create_block(vec![(Ty::Word, case_one_result)]);
    let case_two_result = builder.fresh_value();
    let case_two = builder.create_block(vec![(Ty::Word, case_two_result)]);
    let default_result = builder.fresh_value();
    let default = builder.create_block(vec![(Ty::Word, default_result)]);
    builder
        .position_at(ncl_ir::BlockId(0))
        .expect("entry block");
    builder
        .terminate(Terminator::Switch {
            value: selector,
            cases: vec![
                (negative_case, case_one, vec![case_one_value]),
                (wide_case, case_two, vec![case_two_value]),
            ],
            default,
            default_args: vec![default_value],
        })
        .expect("switch terminator");
    builder.position_at(case_one).expect("first case block");
    builder
        .terminate(Terminator::Return {
            values: vec![case_one_result],
        })
        .expect("first case return");
    builder.position_at(case_two).expect("second case block");
    builder
        .terminate(Terminator::Return {
            values: vec![case_two_result],
        })
        .expect("second case return");
    builder.position_at(default).expect("default block");
    builder
        .terminate(Terminator::Return {
            values: vec![default_result],
        })
        .expect("default return");

    let compiled =
        compile_function_aarch64(&builder.finish(), &Aarch64Abi).expect("switch lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let mut invoke = |selector| {
        invoke_entry(
            &code,
            compiled.entry_offset as usize,
            &mut thread,
            1,
            [selector, 0, 0, 0],
            0,
        )
    };
    assert_eq!(invoke(negative_case as u64), (Word::fixnum(101).bits(), 1));
    assert_eq!(invoke(wide_case as u64), (Word::fixnum(202).bits(), 1));
    assert_eq!(invoke(9), (Word::fixnum(303).bits(), 1));
}

#[test]
fn rejects_builtin_missing_from_execution_fixture_abi() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(103),
        "missing-builtin",
        Vec::new(),
        vec![Ty::Word],
    );
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
        compile_function_aarch64(&builder.finish(), &BuiltinAbi),
        Err(ncl_codegen::CodegenError::Unsupported(message))
            if message.contains("builtin address is unavailable")
    ));
}

#[path = "calls_switch/abi.rs"]
mod abi;
