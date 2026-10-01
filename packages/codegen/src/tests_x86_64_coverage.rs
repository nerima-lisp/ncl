#![allow(missing_docs, clippy::expect_used, clippy::too_many_lines)]

use crate::{
    compile_function_x86_64, CodegenError, ContextField, RuntimeAbi, RuntimeFunction, X86_64Abi,
};
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
    let constants = [
        Constant::Fixnum(9),
        Constant::Character('x' as u32),
        Constant::Nil,
        Constant::Unbound,
        Constant::T,
    ];
    let constant_values = constants
        .into_iter()
        .map(|constant| {
            let index = builder.add_constant(constant);
            builder
                .push_op(OpKind::Const { result: index }, &[Ty::Word])
                .expect("immediate constant")[0]
        })
        .collect::<Vec<_>>();
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
        .push_op(OpKind::LoadCapture { index: 0 }, &[Ty::Word])
        .expect("load capture");
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
            values: vec![heap, entry, constant_values[1]],
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
        .push_op(
            OpKind::Call {
                function: callee,
                args: vec![argc, argc, argc, argc, argc, argc],
            },
            &[],
        )
        .expect("call with overflow arguments");
    builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .expect("alloc");
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    let cell = builder
        .push_op(OpKind::MakeValueCell { value: argc }, &[Ty::Address])
        .expect("value cell")[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry: callee,
                captures: vec![argc],
            },
            &[Ty::Word],
        )
        .expect("closure")[0];
    builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![argc],
                named_symbol: Some(callee),
            },
            &[],
        )
        .expect("named closure call");
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

#[test]
fn x86_64_lowering_covers_progv_move_chain_and_unreachable() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(124),
        "x86-64-progv-move-unreachable",
        Vec::new(),
        Vec::new(),
    );
    let value = builder.add_constant(Constant::Fixnum(1));
    let value = builder
        .push_op(OpKind::Const { result: value }, &[Ty::Word])
        .expect("value")[0];
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry = builder
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .expect("entry")[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![value],
            },
            &[Ty::Word],
        )
        .expect("closure")[0];
    let moved = builder
        .push_op(OpKind::Move { value: closure }, &[Ty::Word])
        .expect("moved closure")[0];
    let argc = builder.add_constant(Constant::Fixnum(0));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc")[0];
    builder
        .push_op(
            OpKind::CallClosure {
                closure: moved,
                args: vec![argc],
                named_symbol: None,
            },
            &[],
        )
        .expect("closure call");
    let region = ncl_ir::HandlerRegionId(8);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
        kind: ncl_ir::HandlerKind::Progv,
        protected: vec![ncl_ir::BlockId(0)],
        handler: ncl_ir::BlockId(0),
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![value, entry],
        depth: 0,
        parent: None,
    });
    builder
        .push_op(OpKind::EnterHandler { region }, &[])
        .expect("enter progv");
    builder
        .push_op(OpKind::LeaveHandler { region }, &[])
        .expect("leave progv");
    builder
        .terminate(Terminator::Unreachable)
        .expect("unreachable");

    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
    assert!(compiled.safepoint_maps.len() >= 4);
    assert_eq!(compiled.code[compiled.code.len() - 2..], [0x0f, 0x0b]);
}

#[test]
fn x86_64_lowering_reports_reachable_invalid_operation_forms() {
    let mut builtin = FunctionBuilder::new(
        ncl_ir::FunctionId(125),
        "x86-64-invalid-rest-list",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builtin.add_constant(Constant::Fixnum(0));
    let constant = builtin
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("constant")[0];
    let result = builtin
        .push_op(
            OpKind::Builtin {
                name: "make-rest-list".into(),
                args: vec![constant],
            },
            &[Ty::Word],
        )
        .expect("builtin")[0];
    builtin
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");
    assert!(matches!(
        compile_function_x86_64(&builtin.finish(), &CoverageAbi),
        Err(CodegenError::Unsupported(message)) if message.contains("make-rest-list")
    ));

    let mut primitive = FunctionBuilder::new(
        ncl_ir::FunctionId(126),
        "x86-64-invalid-primitive",
        Vec::new(),
        Vec::new(),
    );
    primitive
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: Vec::new(),
                condition: None,
            },
            &[],
        )
        .expect("primitive");
    primitive
        .terminate(Terminator::Unreachable)
        .expect("unreachable");
    assert!(matches!(
        compile_function_x86_64(&primitive.finish(), &CoverageAbi),
        Err(CodegenError::Unsupported(message)) if message.contains("no operands")
    ));

    let mut call = FunctionBuilder::new(
        ncl_ir::FunctionId(127),
        "x86-64-invalid-call",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        Vec::new(),
    );
    call.push_op(
        OpKind::Call {
            function: ncl_ir::ValueId(0),
            args: Vec::new(),
        },
        &[],
    )
    .expect("call");
    call.terminate(Terminator::Unreachable)
        .expect("unreachable");
    assert!(matches!(
        compile_function_x86_64(&call.finish(), &CoverageAbi),
        Err(CodegenError::Unsupported(message)) if message.contains("tagged argc")
    ));
}
