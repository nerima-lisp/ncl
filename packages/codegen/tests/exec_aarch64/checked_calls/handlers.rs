use super::*;

fn build_handler_failure_function(id: u32, leave_before_downstream: bool) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-handler-error",
        Vec::new(),
        vec![Ty::Word],
    );
    let catch_tag = fixnum_word(&mut builder, 17);
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("enter handler");
    if leave_before_downstream {
        builder
            .push_op(
                OpKind::LeaveHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                &[],
            )
            .expect("leave handler");
    }
    let value = fixnum_word(&mut builder, 23);
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-handler-error".into(),
                args: vec![value],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    if !leave_before_downstream {
        builder
            .push_op(
                OpKind::LeaveHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                &[],
            )
            .expect("leave handler");
    }
    builder
        .terminate(Terminator::Return {
            values: vec![downstream],
        })
        .expect("return");

    let handler_value = builder.fresh_value();
    let handler = builder.create_block(vec![(Ty::Word, handler_value)]);
    builder
        .terminate(Terminator::Return {
            values: vec![handler_value],
        })
        .expect("handler return");
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: Some(catch_tag),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.finish()
}

fn build_pending_dispatch_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(94),
        "pending-dispatch-leave-catch-error",
        Vec::new(),
        vec![Ty::Word],
    );
    let catch_tag = fixnum_word(&mut builder, 17);
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("enter handler");
    let pending_tag = fixnum_word(&mut builder, 19);
    let payload = fixnum_word(&mut builder, 29);
    let pending_value = builder
        .push_op(
            OpKind::Builtin {
                name: "set-pending-exit".into(),
                args: vec![pending_tag, payload],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("pending exit builtin")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-handler-error".into(),
                args: vec![pending_value],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("leave handler");
    builder
        .terminate(Terminator::Return {
            values: vec![downstream],
        })
        .expect("return");

    let handler_value = builder.fresh_value();
    let handler = builder.create_block(vec![(Ty::Word, handler_value)]);
    builder
        .terminate(Terminator::Return {
            values: vec![handler_value],
        })
        .expect("handler return");
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: Some(catch_tag),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.finish()
}

fn build_progv_failure_function(id: u32, leave_before_downstream: bool) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-progv-error",
        Vec::new(),
        Vec::new(),
    );
    let binding = fixnum_word(&mut builder, 31);
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("enter progv");
    if leave_before_downstream {
        builder
            .push_op(
                OpKind::LeaveHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                &[],
            )
            .expect("leave progv");
    }
    builder
        .push_op(
            OpKind::Builtin {
                name: "after-handler-error".into(),
                args: vec![binding],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin");
    if !leave_before_downstream {
        builder
            .push_op(
                OpKind::LeaveHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                &[],
            )
            .expect("leave progv");
    }
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");

    let handler = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("handler return");
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: ncl_ir::HandlerKind::Progv,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![binding],
        depth: 0,
        parent: None,
    });
    builder.finish()
}

fn build_unwind_protect_failure_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(99),
        "checked-unwind-protect-error",
        Vec::new(),
        Vec::new(),
    );
    let value = fixnum_word(&mut builder, 37);
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("enter unwind-protect");
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("leave unwind-protect");
    builder
        .push_op(
            OpKind::Builtin {
                name: "after-handler-error".into(),
                args: vec![value],
                specialization: None,
            },
            &[Ty::Word],
        )
        .expect("downstream builtin");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");

    let handler = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("handler return");
    let cleanup = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("cleanup return");
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: ncl_ir::HandlerKind::UnwindProtect,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: Some(cleanup),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.finish()
}

#[test]
fn enter_handler_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_CATCH_CALLS.store(0, Ordering::SeqCst);
    ENTER_CATCH_ERROR.store(true, Ordering::SeqCst);
    LEAVE_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_ERROR.store(false, Ordering::SeqCst);
    HANDLER_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (code, entry) = compile_and_publish(&build_handler_failure_function(95, false));
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);
    ENTER_CATCH_ERROR.store(false, Ordering::SeqCst);

    assert_eq!(ENTER_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_CATCH_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(HANDLER_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn leave_handler_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_CATCH_CALLS.store(0, Ordering::SeqCst);
    ENTER_CATCH_ERROR.store(false, Ordering::SeqCst);
    LEAVE_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_ERROR.store(true, Ordering::SeqCst);
    LEAVE_CATCH_SAW_PENDING.store(true, Ordering::SeqCst);
    HANDLER_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (code, entry) = compile_and_publish(&build_handler_failure_function(96, true));
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);
    LEAVE_CATCH_ERROR.store(false, Ordering::SeqCst);

    assert_eq!(ENTER_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert!(!LEAVE_CATCH_SAW_PENDING.load(Ordering::SeqCst));
    assert_eq!(HANDLER_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn pending_dispatch_leave_catch_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_CATCH_CALLS.store(0, Ordering::SeqCst);
    ENTER_CATCH_ERROR.store(false, Ordering::SeqCst);
    LEAVE_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_ERROR.store(true, Ordering::SeqCst);
    LEAVE_CATCH_SAW_PENDING.store(false, Ordering::SeqCst);
    PENDING_EXIT_CALLS.store(0, Ordering::SeqCst);
    HANDLER_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (code, entry) = compile_and_publish(&build_pending_dispatch_function());
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);
    LEAVE_CATCH_ERROR.store(false, Ordering::SeqCst);

    assert_eq!(ENTER_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(PENDING_EXIT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert!(LEAVE_CATCH_SAW_PENDING.load(Ordering::SeqCst));
    assert_eq!(HANDLER_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(!thread.runtime_error_pending());
    assert!(thread.pending());
}

#[test]
fn enter_progv_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_PROGV_CALLS.store(0, Ordering::SeqCst);
    LEAVE_PROGV_CALLS.store(0, Ordering::SeqCst);
    ENTER_PROGV_ERROR.store(true, Ordering::SeqCst);
    LEAVE_PROGV_ERROR.store(false, Ordering::SeqCst);
    HANDLER_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (code, entry) = compile_and_publish(&build_progv_failure_function(97, false));
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);
    ENTER_PROGV_ERROR.store(false, Ordering::SeqCst);

    assert_eq!(ENTER_PROGV_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_PROGV_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(HANDLER_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn leave_progv_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_PROGV_CALLS.store(0, Ordering::SeqCst);
    LEAVE_PROGV_CALLS.store(0, Ordering::SeqCst);
    ENTER_PROGV_ERROR.store(false, Ordering::SeqCst);
    LEAVE_PROGV_ERROR.store(true, Ordering::SeqCst);
    HANDLER_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (code, entry) = compile_and_publish(&build_progv_failure_function(98, true));
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);
    LEAVE_PROGV_ERROR.store(false, Ordering::SeqCst);

    assert_eq!(ENTER_PROGV_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_PROGV_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn leave_unwind_protect_error_stops_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_UNWIND_PROTECT_CALLS.store(0, Ordering::SeqCst);
    LEAVE_UNWIND_PROTECT_CALLS.store(0, Ordering::SeqCst);
    LEAVE_UNWIND_PROTECT_ERROR.store(true, Ordering::SeqCst);
    HANDLER_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
    let (code, entry) = compile_and_publish(&build_unwind_protect_failure_function());
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);
    LEAVE_UNWIND_PROTECT_ERROR.store(false, Ordering::SeqCst);

    assert_eq!(ENTER_UNWIND_PROTECT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_UNWIND_PROTECT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}
