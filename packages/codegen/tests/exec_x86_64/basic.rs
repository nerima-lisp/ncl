use super::*;

#[test]
fn executes_constant_return_in_published_code() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(0),
        "constant",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = fixnum_i64(&mut builder, 42);
    let value = i64_to_word(&mut builder, constant);
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return terminator");
    let abi = X86_64Abi;
    let compiled = compile_function_x86_64(&builder.finish(), &abi).expect("lowering");
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
    assert_eq!(value, Word::fixnum(42).bits());
    assert_eq!(count, 1);
}

#[test]
fn executes_immediate_constants_as_words() {
    fn run(constant: Constant, id: u32) -> Word {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(id),
            "immediate",
            Vec::new(),
            vec![Ty::Word],
        );
        let is_fixnum = matches!(&constant, Constant::Fixnum(_));
        let constant = builder.add_constant(constant);
        let result_ty = if is_fixnum { Ty::I64 } else { Ty::Word };
        let value = builder
            .push_op(OpKind::Const { result: constant }, &[result_ty])
            .expect("constant operation")[0];
        let value = if is_fixnum {
            i64_to_word(&mut builder, value)
        } else {
            value
        };
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .expect("return terminator");
        let compiled = compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("lowering");
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
        assert_eq!(count, 1);
        Word::from_bits(value)
    }

    assert_eq!(run(Constant::Character(65), 1), Word::character(65));
    assert_eq!(run(Constant::T, 2), Word::TRUE);
    assert_eq!(run(Constant::Nil, 3), Word::NIL);
    assert_eq!(run(Constant::Unbound, 4), Word::UNBOUND);
    assert_eq!(run(Constant::Fixnum(17), 5), Word::fixnum(17));
}
