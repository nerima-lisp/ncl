#![allow(missing_docs, clippy::expect_used)]

use ncl_codegen::{CodegenError, X86_64Abi, compile_function_x86_64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Param, Terminator, Ty, ValueId};

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn x86_64_indirect_call_emits_the_fixed_call_and_frame_templates() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(224),
        "integration-indirect-call",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        Vec::new(),
    );
    let argc_index = builder.add_constant(Constant::Fixnum(0));
    let argc = builder
        .push_op(OpKind::Const { result: argc_index }, &[Ty::Word])
        .expect("argc constant")[0];
    builder
        .push_op(
            OpKind::CallIndirect {
                callee: ValueId(0),
                args: vec![argc],
            },
            &[],
        )
        .expect("indirect call");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64Abi)
        .expect("x86-64 indirect call lowering");
    assert!(contains(&compiled.code, &[0x41, 0xff, 0xd3]));
    assert!(contains(&compiled.code, &[0x48, 0x83, 0xec, 0x10]));
    assert!(compiled.frame_size >= 32);
}

#[test]
fn x86_64_lowering_reports_empty_functions_and_missing_abi_entries() {
    let empty = ncl_ir::Function {
        id: ncl_ir::FunctionId(225),
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
        compile_function_x86_64(&empty, &X86_64Abi),
        Err(CodegenError::EmptyFunction)
    );

    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(226),
        "integration-missing-builtin",
        Vec::new(),
        vec![Ty::Word],
    );
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "identity".into(),
                args: Vec::new(),
            },
            &[Ty::Word],
        )
        .expect("builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");
    assert!(matches!(
        compile_function_x86_64(&builder.finish(), &X86_64Abi),
        Err(CodegenError::Unsupported(message))
            if message.contains("builtin address is unavailable")
    ));
}
