#![allow(missing_docs, clippy::expect_used, clippy::too_many_lines)]

use crate::{ContextField, RuntimeAbi, RuntimeFunction, X86_64Abi, compile_function_x86_64};
use ncl_ir::{Compare, Constant, Convert, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};

struct CoverageAbi;

impl RuntimeAbi for CoverageAbi {
    fn builtin_address(
        &self,
        _identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, crate::AbiError> {
        Ok(0x1000)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, crate::AbiError> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::TlabBump => layout.tlab_bump,
            ContextField::TlabLimit => layout.tlab_limit,
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::Pending => layout.pending,
            ContextField::MultipleValueCount => layout.mv_count,
            ContextField::MultipleValueArea => layout.mv,
            ContextField::Handler => layout.handler,
            ContextField::Cleanup => layout.cleanup,
            ContextField::Catch => layout.catch,
        };
        i32::try_from(offset).map_err(|_| crate::AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, _function: RuntimeFunction) -> Result<u64, crate::AbiError> {
        Ok(0x1000)
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:7").then_some(0x2000)
    }
}

fn compile(builder: FunctionBuilder) -> crate::CompiledFunction {
    compile_function_x86_64(&builder.finish(), &CoverageAbi).expect("x86-64 lowering")
}

#[test]
fn x86_64_lowering_covers_memory_constants_conversions_and_primitives() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(120),
        "x86-64-lowering-operation-matrix",
        vec![
            Param {
                name: "object".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let object = ncl_ir::ValueId(0);
    let value = ncl_ir::ValueId(1);
    let fixnum = builder.add_constant(Constant::Fixnum(9));
    let heap = builder.add_constant(Constant::StringBytes(vec![1, 2, 3]));
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let fixnum = builder
        .push_op(OpKind::Const { result: fixnum }, &[Ty::Word])
        .expect("fixnum")[0];
    let heap = builder
        .push_op(OpKind::Const { result: heap }, &[Ty::Word])
        .expect("heap")[0];
    let entry = builder
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .expect("entry")[0];
    let moved = builder
        .push_op(OpKind::Move { value }, &[Ty::Word])
        .expect("move")[0];
    let converted = builder
        .push_op(
            OpKind::Convert {
                op: Convert::WordToI64,
                value: moved,
            },
            &[Ty::I64],
        )
        .expect("convert")[0];
    let loaded = builder
        .push_op(OpKind::Load { address: object }, &[Ty::Word])
        .expect("load")[0];
    builder
        .push_op(
            OpKind::Store {
                address: object,
                value: loaded,
            },
            &[],
        )
        .expect("store");
    let field = builder
        .push_op(OpKind::LoadField { object, field: 2 }, &[Ty::Word])
        .expect("load field")[0];
    builder
        .push_op(
            OpKind::StoreField {
                object,
                field: 3,
                value: field,
            },
            &[],
        )
        .expect("store field");
    builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("load arg");
    builder
        .push_op(OpKind::LoadFunctionObject, &[Ty::Word])
        .expect("load function object");
    for op in [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Le,
        Compare::Gt,
        Compare::Ge,
    ] {
        builder
            .push_op(
                OpKind::Compare {
                    op,
                    left: value,
                    right: fixnum,
                },
                &[Ty::Bool],
            )
            .expect("compare");
    }
    for op in [
        Prim::FixnumAdd,
        Prim::FixnumSub,
        Prim::FixnumMul,
        Prim::FixnumEq,
        Prim::Eq,
        Prim::Eql,
        Prim::FixnumLt,
        Prim::FixnumLe,
        Prim::Car,
        Prim::Cdr,
        Prim::Svref,
        Prim::Aref,
    ] {
        builder
            .push_op(
                OpKind::Prim {
                    op,
                    args: vec![value, fixnum],
                    condition: None,
                },
                &[Ty::Word],
            )
            .expect("primitive");
    }
    for op in [Prim::Rplaca, Prim::Rplacd, Prim::Aset] {
        builder
            .push_op(
                OpKind::Prim {
                    op,
                    args: vec![object, value],
                    condition: None,
                },
                &[],
            )
            .expect("store primitive");
    }
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![value, converted],
            },
            &[Ty::Word],
        )
        .expect("multiple values");
    builder
        .terminate(Terminator::Return {
            values: vec![heap, entry],
        })
        .expect("return");

    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.safepoint_maps.len(), 0);
}

#[test]
fn x86_64_lowering_covers_allocation_safepoint_builtin_and_call_return() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(121),
        "x86-64-lowering-runtime-operation-matrix",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
    );
    let callee = ncl_ir::ValueId(0);
    let argc = builder.add_constant(Constant::Fixnum(0));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc")[0];
    builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .expect("alloc");
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    let cell = builder
        .push_op(OpKind::MakeValueCell { value: argc }, &[Ty::Address])
        .expect("value cell")[0];
    builder
        .push_op(
            OpKind::Builtin {
                name: "identity".into(),
                args: vec![cell],
            },
            &[Ty::Word],
        )
        .expect("builtin");
    builder
        .terminate(Terminator::CallReturn {
            function: callee,
            args: vec![argc],
        })
        .expect("call return");

    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
    assert!(compiled.safepoint_maps.len() >= 4);
}

#[test]
fn x86_64_generated_lambda_lowering_covers_make_rest_list() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(122),
        "x86-64-generated-lambda-rest-list",
        vec![
            Param {
                name: "argc".into(),
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
    builder
        .push_op(
            OpKind::Builtin {
                name: "make-rest-list".into(),
                args: vec![ncl_ir::ValueId(0), ncl_ir::ValueId(1)],
            },
            &[Ty::Word],
        )
        .expect("make-rest-list");
    builder
        .terminate(Terminator::Return {
            values: vec![ncl_ir::ValueId(6)],
        })
        .expect("return");

    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.safepoint_maps.len(), 1);
}

#[test]
fn x86_64_abi_compiles_without_fixture_specific_addresses() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(123),
        "x86-64-public-abi",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(11));
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("constant");
    builder
        .terminate(Terminator::Return { values: value })
        .expect("return");
    let compiled =
        compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("public ABI lowering");
    assert!(!compiled.code.is_empty());
}
