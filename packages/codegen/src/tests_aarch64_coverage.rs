#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use crate::{compile_function_aarch64, CodegenError, ContextField, RuntimeAbi, RuntimeFunction};
use ncl_ir::{
    Compare, Constant, FunctionBuilder, HandlerKind, HandlerRegion, OpKind, Prim, Terminator, Ty,
};

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
            ContextField::MultipleValueArea => layout.mv,
            ContextField::Pending => layout.pending,
            ContextField::MultipleValueCount => layout.mv_count,
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
    compile_function_aarch64(&builder.finish(), &CoverageAbi).expect("AArch64 lowering")
}

#[test]
fn aarch64_operation_matrix_reaches_real_lowering_paths() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(140),
        "aarch64-operation-matrix",
        vec![
            ncl_ir::Param {
                name: "object".into(),
                ty: Ty::Address,
            },
            ncl_ir::Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let object = ncl_ir::ValueId(0);
    let value = ncl_ir::ValueId(1);
    let constants = [
        Constant::Fixnum(7),
        Constant::Character('x' as u32),
        Constant::Nil,
        Constant::Unbound,
        Constant::T,
        Constant::FunctionEntry(ncl_ir::FunctionId(7)),
        Constant::StringBytes(vec![1, 2]),
    ];
    let mut values = Vec::new();
    for constant in constants {
        let index = builder.add_constant(constant);
        values.push(
            builder
                .push_op(OpKind::Const { result: index }, &[Ty::Word])
                .unwrap()[0],
        );
    }
    let moved = builder
        .push_op(OpKind::Move { value }, &[Ty::Word])
        .unwrap()[0];
    let converted = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::WordToI64,
                value: moved,
            },
            &[Ty::I64],
        )
        .unwrap()[0];
    builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: converted,
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: converted,
            },
            &[],
        )
        .unwrap();
    let loaded = builder
        .push_op(OpKind::Load { address: object }, &[Ty::Word])
        .unwrap()[0];
    builder
        .push_op(OpKind::Load { address: object }, &[])
        .unwrap();
    builder
        .push_op(
            OpKind::Store {
                address: object,
                value: loaded,
            },
            &[],
        )
        .unwrap();
    let field = builder
        .push_op(OpKind::LoadField { object, field: 2 }, &[Ty::Word])
        .unwrap()[0];
    builder
        .push_op(OpKind::LoadField { object, field: 4 }, &[])
        .unwrap();
    builder
        .push_op(
            OpKind::StoreField {
                object,
                field: 3,
                value: field,
            },
            &[],
        )
        .unwrap();
    builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .unwrap();
    builder.push_op(OpKind::LoadArg { index: 1 }, &[]).unwrap();
    builder
        .push_op(OpKind::LoadCapture { index: 0 }, &[Ty::Word])
        .unwrap();
    builder
        .push_op(OpKind::LoadCapture { index: 0 }, &[])
        .unwrap();
    builder
        .push_op(OpKind::LoadFunctionObject, &[Ty::Word])
        .unwrap();
    builder.push_op(OpKind::LoadFunctionObject, &[]).unwrap();
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
                    right: values[0],
                },
                &[Ty::Bool],
            )
            .unwrap();
    }
    builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: value,
                right: values[0],
            },
            &[],
        )
        .unwrap();
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
        Prim::Rplaca,
        Prim::Rplacd,
        Prim::Aset,
    ] {
        let result = !matches!(op, Prim::Rplaca | Prim::Rplacd | Prim::Aset);
        builder
            .push_op(
                OpKind::Prim {
                    op,
                    args: vec![object, value],
                    condition: None,
                },
                if result { &[Ty::Word] } else { &[] },
            )
            .unwrap();
    }
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![value, converted],
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .unwrap();
    builder.push_op(OpKind::Safepoint, &[]).unwrap();
    builder
        .push_op(
            OpKind::Builtin {
                name: "identity".into(),
                args: vec![value],
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .terminate(Terminator::Return {
            values: vec![values[0]],
        })
        .unwrap();
    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
    assert!(compiled.safepoint_maps.len() >= 3);
}

#[test]
fn aarch64_calls_closures_handlers_and_all_terminators_compile() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(141),
        "aarch64-call-and-handlers",
        vec![ncl_ir::Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
    );
    let callee = ncl_ir::ValueId(0);
    let argc = builder.add_constant(Constant::Fixnum(0));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .unwrap()[0];
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry = builder
        .push_op(OpKind::Const { result: entry }, &[Ty::Address])
        .unwrap()[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![argc],
            },
            &[Ty::Address],
        )
        .unwrap()[0];
    builder
        .push_op(
            OpKind::Call {
                function: callee,
                args: vec![argc],
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .push_op(
            OpKind::CallIndirect {
                callee,
                args: vec![argc],
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![argc],
                named_symbol: Some(argc),
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .push_op(OpKind::MakeValueCell { value: argc }, &[Ty::Address])
        .unwrap();
    let region = ncl_ir::HandlerRegionId(1);
    let handler = builder.create_block(Vec::new());
    builder.add_handler_region(HandlerRegion {
        id: region,
        kind: HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: Some(argc),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    let unwind_region = ncl_ir::HandlerRegionId(2);
    builder.add_handler_region(HandlerRegion {
        id: unwind_region,
        kind: HandlerKind::UnwindProtect,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: Some(handler),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    let progv_region = ncl_ir::HandlerRegionId(3);
    builder.add_handler_region(HandlerRegion {
        id: progv_region,
        kind: HandlerKind::Progv,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![argc],
        depth: 0,
        parent: None,
    });
    builder.position_at(ncl_ir::BlockId(0)).unwrap();
    builder
        .push_op(OpKind::EnterHandler { region }, &[])
        .unwrap();
    builder
        .push_op(OpKind::LeaveHandler { region }, &[])
        .unwrap();
    builder
        .push_op(OpKind::EnterHandler { region: unwind_region }, &[])
        .unwrap();
    builder
        .push_op(OpKind::LeaveHandler { region: unwind_region }, &[])
        .unwrap();
    builder
        .push_op(OpKind::EnterHandler { region: progv_region }, &[])
        .unwrap();
    builder
        .push_op(OpKind::LeaveHandler { region: progv_region }, &[])
        .unwrap();
    builder
        .terminate(Terminator::Throw { condition: argc })
        .unwrap();
    builder.position_at(handler).unwrap();
    builder
        .terminate(Terminator::Return { values: vec![argc] })
        .unwrap();
    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
    assert!(!compiled.safepoint_maps.is_empty());

    for terminator in [
        Terminator::Unreachable,
        Terminator::CallReturn {
            function: callee,
            args: vec![argc],
        },
        Terminator::TailCall {
            function: callee,
            args: vec![argc],
        },
    ] {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(142),
            "aarch64-terminator",
            vec![ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Address,
            }],
            vec![],
        );
        let argc_index = builder.add_constant(Constant::Fixnum(0));
        let argc = builder
            .push_op(OpKind::Const { result: argc_index }, &[Ty::Word])
            .unwrap()[0];
        builder.terminate(terminator).unwrap();
        let compiled = compile(builder);
        assert!(!compiled.code.is_empty());
        let _ = argc;
    }
}

#[test]
fn aarch64_dispatches_all_covering_handler_kinds_and_bindings() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(145),
        "aarch64-dispatch-candidates",
        vec![ncl_ir::Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![],
    );
    let value = ncl_ir::ValueId(0);
    let tag = builder.add_constant(Constant::Fixnum(1));
    let tag = builder
        .push_op(OpKind::Const { result: tag }, &[Ty::Word])
        .unwrap()[0];
    let catch_handler = builder.create_block(vec![
        (Ty::Word, ncl_ir::ValueId(10)),
        (Ty::Word, ncl_ir::ValueId(11)),
    ]);
    let unwind_handler = builder.create_block(Vec::new());
    let progv_handler = builder.create_block(vec![(Ty::Word, ncl_ir::ValueId(12))]);
    let protected = vec![ncl_ir::BlockId(0)];
    builder.add_handler_region(HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: HandlerKind::Catch,
        protected: protected.clone(),
        handler: catch_handler,
        cleanup: None,
        catch_tag: Some(tag),
        binding_targets: vec![value],
        depth: 0,
        parent: None,
    });
    builder.add_handler_region(HandlerRegion {
        id: ncl_ir::HandlerRegionId(1),
        kind: HandlerKind::UnwindProtect,
        protected: protected.clone(),
        handler: unwind_handler,
        cleanup: Some(unwind_handler),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.add_handler_region(HandlerRegion {
        id: ncl_ir::HandlerRegionId(2),
        kind: HandlerKind::Progv,
        protected,
        handler: progv_handler,
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![value],
        depth: 0,
        parent: None,
    });
    builder
        .push_op(
            OpKind::Builtin {
                name: "identity".into(),
                args: vec![value],
            },
            &[Ty::Word],
        )
        .unwrap();
    builder
        .terminate(Terminator::Throw { condition: tag })
        .unwrap();
    for (block, result) in [
        (catch_handler, ncl_ir::ValueId(10)),
        (unwind_handler, value),
        (progv_handler, ncl_ir::ValueId(12)),
    ] {
        builder.position_at(block).unwrap();
        builder
            .terminate(Terminator::Return {
                values: vec![result],
            })
            .unwrap();
    }

    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());
}

#[test]
fn aarch64_branch_switch_and_generated_lambda_paths_compile() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(143),
        "aarch64-control-flow",
        vec![ncl_ir::Param {
            name: "argc".into(),
            ty: Ty::Word,
        }],
        vec![],
    );
    let argc = ncl_ir::ValueId(0);
    builder
        .push_op(
            OpKind::Builtin {
                name: "make-rest-list".into(),
                args: vec![argc, argc],
            },
            &[Ty::Word],
        )
        .unwrap();
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).unwrap();
    builder
        .terminate(Terminator::Branch {
            condition: argc,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .unwrap();
    for block in [then_block, else_block] {
        builder.position_at(block).unwrap();
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .unwrap();
    }
    let compiled = compile(builder);
    assert!(!compiled.code.is_empty());

    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(144),
        "aarch64-switch",
        Vec::new(),
        vec![],
    );
    let selector = builder.add_constant(Constant::Fixnum(1));
    let selector = builder
        .push_op(OpKind::Const { result: selector }, &[Ty::Word])
        .unwrap()[0];
    let case = builder.create_block(Vec::new());
    let fallback = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).unwrap();
    builder
        .terminate(Terminator::Switch {
            value: selector,
            cases: vec![(1, case, Vec::new())],
            default: fallback,
            default_args: Vec::new(),
        })
        .unwrap();
    for block in [case, fallback] {
        builder.position_at(block).unwrap();
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .unwrap();
    }
    assert!(!compile(builder).code.is_empty());
}

#[test]
fn aarch64_coverage_reaches_large_frames_rest_arguments_and_contract_errors() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(146),
        "aarch64-large-frame-call",
        std::iter::once(ncl_ir::Param {
            name: "callee".into(),
            ty: Ty::Address,
        })
        .chain((0..599).map(|index| ncl_ir::Param {
            name: format!("value-{index}"),
            ty: Ty::Word,
        }))
        .collect(),
        vec![Ty::Word],
    );
    let argc_index = builder.add_constant(Constant::Fixnum(599));
    let argc = builder
        .push_op(OpKind::Const { result: argc_index }, &[Ty::Word])
        .unwrap()[0];
    let args = std::iter::once(argc)
        .chain((1..600).map(ncl_ir::ValueId))
        .collect::<Vec<_>>();
    let result = builder
        .push_op(
            OpKind::Call {
                function: ncl_ir::ValueId(0),
                args: args.clone(),
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .unwrap();
    assert!(matches!(
        compile_function_aarch64(&builder.finish(), &CoverageAbi),
        Err(CodegenError::Encode(_))
    ));

    for (function_id, name, terminator) in [
        (
            ncl_ir::FunctionId(147),
            "aarch64-large-frame-call-return",
            true,
        ),
        (
            ncl_ir::FunctionId(148),
            "aarch64-large-frame-tail-call",
            false,
        ),
    ] {
        let mut builder = FunctionBuilder::new(
            function_id,
            name,
            std::iter::once(ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Address,
            })
            .chain((0..599).map(|index| ncl_ir::Param {
                name: format!("value-{index}"),
                ty: Ty::Word,
            }))
            .collect(),
            vec![],
        );
        let argc_index = builder.add_constant(Constant::Fixnum(599));
        let argc = builder
            .push_op(OpKind::Const { result: argc_index }, &[Ty::Word])
            .unwrap()[0];
        let args = std::iter::once(argc)
            .chain((1..600).map(ncl_ir::ValueId))
            .collect::<Vec<_>>();
        builder
            .terminate(if terminator {
                Terminator::CallReturn {
                    function: ncl_ir::ValueId(0),
                    args,
                }
            } else {
                Terminator::TailCall {
                    function: ncl_ir::ValueId(0),
                    args,
                }
            })
            .unwrap();
        assert!(matches!(
            compile_function_aarch64(&builder.finish(), &CoverageAbi),
            Err(CodegenError::Encode(_))
        ));
    }

    let mut malformed_call = FunctionBuilder::new(
        ncl_ir::FunctionId(149),
        "aarch64-malformed-call",
        vec![ncl_ir::Param {
            name: "callee".into(),
            ty: Ty::Address,
        }],
        vec![],
    );
    malformed_call
        .push_op(
            OpKind::Call {
                function: ncl_ir::ValueId(0),
                args: Vec::new(),
            },
            &[],
        )
        .unwrap();
    malformed_call
        .terminate(Terminator::Unreachable)
        .unwrap();
    assert!(matches!(
        compile_function_aarch64(&malformed_call.finish(), &CoverageAbi),
        Err(CodegenError::Unsupported(_))
    ));

    let mut malformed_branch = FunctionBuilder::new(
        ncl_ir::FunctionId(150),
        "aarch64-malformed-block-arguments",
        vec![ncl_ir::Param {
            name: "condition".into(),
            ty: Ty::Word,
        }],
        vec![],
    );
    let then_block = malformed_branch.create_block(vec![(Ty::Word, ncl_ir::ValueId(1))]);
    let else_block = malformed_branch.create_block(Vec::new());
    malformed_branch
        .position_at(ncl_ir::BlockId(0))
        .unwrap();
    malformed_branch
        .terminate(Terminator::Branch {
            condition: ncl_ir::ValueId(0),
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        compile_function_aarch64(&malformed_branch.finish(), &CoverageAbi),
        Err(CodegenError::Unsupported(_))
    ));
}
