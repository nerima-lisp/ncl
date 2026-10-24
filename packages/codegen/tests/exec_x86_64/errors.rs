use super::*;

#[test]
fn builtin_fixture_exposes_all_context_fields() {
    let layout = thread_layout();
    let abi = BuiltinAbi;
    let fields = [
        (ContextField::TlabBump, layout.tlab_bump),
        (ContextField::TlabLimit, layout.tlab_limit),
        (ContextField::SafepointRequest, layout.safepoint_request),
        (ContextField::MultipleValueArea, layout.mv),
        (ContextField::Pending, layout.pending),
        (ContextField::MultipleValueCount, layout.mv_count),
        (ContextField::Handler, layout.handler),
        (ContextField::Cleanup, layout.cleanup),
        (ContextField::Catch, layout.catch),
        (ContextField::RuntimeErrorFlags, layout.runtime_error_flags),
    ];
    for (field, expected) in fields {
        assert_eq!(abi.field_offset(field).ok(), i32::try_from(expected).ok());
    }
}

#[test]
fn make_closure_error_stops_before_capture_stores() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(36),
        "make-closure-error",
        vec![
            Param {
                name: "entry".into(),
                ty: Ty::Word,
            },
            Param {
                name: "capture".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let entry = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("entry argument")[0];
    let capture = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("capture argument")[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![capture],
            },
            &[Ty::Word],
        )
        .expect("closure allocation")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![closure],
        })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [Word::fixnum(9).bits(), Word::fixnum(37).bits(), 0, 0],
        0,
    );

    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
    assert_eq!(thread.mv_count(), 0);
}

#[test]
fn make_value_cell_error_returns_no_values() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(37),
        "make-value-cell-error",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("value argument")[0];
    let cell = builder
        .push_op(OpKind::MakeValueCell { value }, &[Ty::Word])
        .expect("value cell allocation")[0];
    builder
        .terminate(Terminator::Return { values: vec![cell] })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        1,
        [Word::fixnum(37).bits(), 0, 0, 0],
        0,
    );

    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert_eq!(thread.mv_count(), 0);
}

#[test]
fn builtin_error_skips_later_helper_calls() {
    AFTER_ERROR_BUILTIN_CALLS.store(0, Ordering::SeqCst);
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(38),
        "builtin-error",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("value argument")[0];
    let failed = builder
        .push_op(
            OpKind::Builtin {
                name: "error".into(),
                args: vec![value],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("failing builtin")[0];
    let observed = builder
        .push_op(
            OpKind::Builtin {
                name: "observe-after-error".into(),
                args: vec![failed],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![observed],
        })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        1,
        [Word::fixnum(23).bits(), 0, 0, 0],
        0,
    );

    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert_eq!(AFTER_ERROR_BUILTIN_CALLS.load(Ordering::SeqCst), 0);
}
