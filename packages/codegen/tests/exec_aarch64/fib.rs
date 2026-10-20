use super::*;

#[test]
#[allow(clippy::too_many_lines)]
fn executes_recursive_fib_twenty_five() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(3),
        "fib",
        vec![
            Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            Param {
                name: "n".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let callee = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("callee argument")[0];
    let n = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("n argument")[0];
    let one = fixnum_i64(&mut builder, 1);
    let two = fixnum_i64(&mut builder, 2);
    let argc_one_i64 = fixnum_i64(&mut builder, 1);
    let argc_one = convert(&mut builder, Convert::I64ToWord, argc_one_i64, Ty::Word);
    let argc_two_i64 = fixnum_i64(&mut builder, 2);
    let argc_two = convert(&mut builder, Convert::I64ToWord, argc_two_i64, Ty::Word);
    let n_i64 = convert(&mut builder, Convert::WordToI64, n, Ty::I64);
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Le,
                left: n_i64,
                right: one,
            },
            &[Ty::Bool],
        )
        .expect("base comparison")[0];
    let base_n = builder.fresh_value();
    let recursive_callee = builder.fresh_value();
    let recursive_n = builder.fresh_value();
    let base = builder.create_block(vec![(Ty::Word, base_n)]);
    let recursive =
        builder.create_block(vec![(Ty::Word, recursive_callee), (Ty::Word, recursive_n)]);
    builder.position_at(ncl_ir::BlockId(0)).expect("entry");
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: base,
            then_args: vec![n],
            else_target: recursive,
            else_args: vec![callee, n],
        })
        .expect("branch");
    builder.position_at(base).expect("base");
    builder
        .terminate(Terminator::Return {
            values: vec![base_n],
        })
        .expect("base return");
    builder.position_at(recursive).expect("recursive");
    let recursive_n = convert(&mut builder, Convert::WordToI64, recursive_n, Ty::I64);
    let n_minus_one = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![recursive_n, one],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("n minus one")[0];
    let n_minus_one = convert(&mut builder, Convert::I64ToWord, n_minus_one, Ty::Word);
    let first = builder
        .push_op(
            OpKind::Call {
                function: recursive_callee,
                args: vec![argc_one, recursive_callee, n_minus_one],
            },
            &[Ty::Word],
        )
        .expect("first recursive call")[0];
    let n_minus_two = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![recursive_n, two],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("n minus two")[0];
    let n_minus_two = convert(&mut builder, Convert::I64ToWord, n_minus_two, Ty::Word);
    let second = builder
        .push_op(
            OpKind::Call {
                function: recursive_callee,
                args: vec![argc_two, recursive_callee, n_minus_two],
            },
            &[Ty::Word],
        )
        .expect("second recursive call")[0];
    let first = convert(&mut builder, Convert::WordToI64, first, Ty::I64);
    let second = convert(&mut builder, Convert::WordToI64, second, Ty::I64);
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![first, second],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("sum")[0];
    let sum = convert(&mut builder, Convert::I64ToWord, sum, Ty::Word);
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .expect("recursive return");
    let abi = Aarch64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let frame_words = compiled
        .safepoint_maps
        .first()
        .expect("fib safepoint map")
        .frame_words;
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let mut samples = Vec::with_capacity(10);
    for _ in 0..10 {
        let started = Instant::now();
        let (value, count) = invoke_entry(
            &code,
            compiled.entry_offset as usize,
            &mut thread,
            2,
            [code.address() as u64, Word::fixnum(25).bits(), 0, 0],
            0,
        );
        samples.push(started.elapsed().as_nanos());
        assert_eq!(value, Word::fixnum(75_025).bits());
        assert_eq!(count, 1);
    }
    samples.sort_unstable();
    println!(
        "fib(25) frame words: {frame_words}, median: {} ns",
        samples[samples.len() / 2]
    );
}
