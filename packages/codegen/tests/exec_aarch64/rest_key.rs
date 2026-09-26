use super::*;

#[test]
fn generated_lambda_loads_rest_after_four_declared_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(32),
        "generated-rest-layout",
        vec![
            Param {
                name: "argc".into(),
                ty: Ty::Word,
            },
            Param {
                name: "required".into(),
                ty: Ty::Word,
            },
            Param {
                name: "key-value".into(),
                ty: Ty::Word,
            },
            Param {
                name: "supplied-p".into(),
                ty: Ty::Word,
            },
            Param {
                name: "default".into(),
                ty: Ty::Word,
            },
            Param {
                name: "rest".into(),
                ty: Ty::Word,
            },
            Param {
                name: "rest-tail".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 6 }, &[Ty::Word])
        .expect("second rest argument")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");

    let abi = Aarch64Abi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let rest = [abi.encode_fixnum(50) as u64, abi.encode_fixnum(60) as u64];
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        6,
        [
            abi.encode_fixnum(10) as u64,
            abi.encode_fixnum(20) as u64,
            abi.encode_fixnum(30) as u64,
            abi.encode_fixnum(40) as u64,
        ],
        rest.as_ptr() as u64,
    );
    assert_eq!(value, abi.encode_fixnum(60) as u64);
    assert_eq!(count, 1);
}
