use super::*;

fn build_error_callee() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(90),
        "call-target-error",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let value = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("value argument")[0];
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "call-target-error".into(),
                args: vec![value],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("failing builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");
    builder.finish()
}

fn build_checked_call_caller(id: u32, indirect: bool) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-call-error",
        vec![
            Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let callee = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("callee argument")[0];
    let value = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("value argument")[0];
    let argc = fixnum_word(&mut builder, 1);
    let call = if indirect {
        OpKind::CallIndirect {
            callee,
            args: vec![argc, value],
        }
    } else {
        OpKind::Call {
            function: callee,
            args: vec![argc, value],
        }
    };
    let result = builder.push_op(call, &[Ty::Word]).expect("compiled call")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-call-error".into(),
                args: vec![result],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![downstream],
        })
        .expect("return");
    builder.finish()
}

fn build_closure_caller() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(91),
        "checked-closure-call-error",
        vec![
            Param {
                name: "closure".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let closure = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("closure argument")[0];
    let value = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("value argument")[0];
    let argc = fixnum_word(&mut builder, 1);
    let result = builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![argc, value],
                named_symbol: None,
            },
            &[Ty::Word],
        )
        .expect("closure call")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-call-error".into(),
                args: vec![result],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![downstream],
        })
        .expect("return");
    builder.finish()
}

#[test]
fn call_and_call_indirect_errors_stop_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let (inner_code, inner_entry) = compile_and_publish(&build_error_callee());
    let inner_address = (inner_code.address() + inner_entry) as u64;

    for (id, indirect, name) in [(92, false, "Call"), (93, true, "CallIndirect")] {
        CALL_TARGET_ERROR_CALLS.store(0, Ordering::SeqCst);
        CALL_TARGET_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
        let (outer_code, outer_entry) =
            compile_and_publish(&build_checked_call_caller(id, indirect));
        let mut thread = Thread::new();
        let (value, count) = invoke_entry(
            &outer_code,
            outer_entry,
            &mut thread,
            2,
            [inner_address, Word::fixnum(23).bits(), 0, 0],
            0,
        );

        assert_eq!(CALL_TARGET_ERROR_CALLS.load(Ordering::SeqCst), 1, "{name}");
        assert_eq!(
            CALL_TARGET_DOWNSTREAM_CALLS.load(Ordering::SeqCst),
            0,
            "{name} used the failed result"
        );
        assert_eq!(value, Word::ZERO.bits(), "{name}");
        assert_eq!(count, 0, "{name}");
        assert!(thread.runtime_error_pending(), "{name}");
        assert!(!thread.pending(), "{name}");
    }
}

#[test]
fn call_closure_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    CALL_TARGET_ERROR_CALLS.store(0, Ordering::SeqCst);
    CALL_TARGET_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (target_code, target_entry) = compile_and_publish(&build_error_callee());
    let mut closure_object = [0_u64; ncl_object::function_offset::ENTRY + 2];
    let entry = target_code.address() + target_entry;
    closure_object[ncl_object::function_offset::ENTRY + 1] =
        Word::fixnum(i64::try_from(entry).expect("entry address fits a fixnum")).bits();
    let closure_address = closure_object.as_ptr() as usize as u64;
    let (wrapper_code, wrapper_entry) = compile_and_publish(&build_closure_caller());
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &wrapper_code,
        wrapper_entry,
        &mut thread,
        2,
        [closure_address, Word::fixnum(23).bits(), 0, 0],
        0,
    );

    assert_eq!(CALL_TARGET_ERROR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(CALL_TARGET_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}
