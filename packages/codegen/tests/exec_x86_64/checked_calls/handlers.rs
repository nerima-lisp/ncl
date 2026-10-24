use super::*;

fn build_progv_handler_function(id: u32) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-progv-handler-error",
        Vec::new(),
        vec![Ty::Word],
    );
    let symbols = const_nil(&mut builder);
    let values = const_nil(&mut builder);
    let region = ncl_ir::HandlerRegionId(0);
    enter_handler(&mut builder, region);
    let value = const_fixnum(&mut builder, 11);
    let before = builtin(&mut builder, "checked-handler-observer", vec![value]);
    leave_handler(&mut builder, region);
    let after = builtin(&mut builder, "checked-handler-observer", vec![before]);
    return_values(&mut builder, vec![after]);

    let handler = builder.create_block(Vec::new());
    let handler_value = const_nil(&mut builder);
    return_values(&mut builder, vec![handler_value]);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
        kind: ncl_ir::HandlerKind::Progv,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![symbols, values],
        depth: 0,
        parent: None,
    });
    builder.finish()
}

fn build_catch_handler_function(id: u32) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-catch-handler-error",
        Vec::new(),
        vec![Ty::Word],
    );
    let tag = const_fixnum(&mut builder, 17);
    let region = ncl_ir::HandlerRegionId(2);
    enter_handler(&mut builder, region);
    let value = const_fixnum(&mut builder, 11);
    let before = builtin(&mut builder, "checked-handler-observer", vec![value]);
    leave_handler(&mut builder, region);
    let after = builtin(&mut builder, "checked-handler-observer", vec![before]);
    return_values(&mut builder, vec![after]);

    let catch_value = builder.fresh_value();
    let handler = builder.create_block(vec![(Ty::Word, catch_value)]);
    return_values(&mut builder, vec![catch_value]);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler,
        cleanup: None,
        catch_tag: Some(tag),
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
    let outer_tag = const_fixnum(&mut builder, 2);
    let inner_tag = const_fixnum(&mut builder, 1);
    let outer_region = ncl_ir::HandlerRegionId(0);
    let inner_region = ncl_ir::HandlerRegionId(1);
    enter_handler(&mut builder, outer_region);
    enter_handler(&mut builder, inner_region);
    leave_handler(&mut builder, inner_region);
    leave_handler(&mut builder, outer_region);
    let normal = const_fixnum(&mut builder, 99);
    return_values(&mut builder, vec![normal]);

    let inner_value = builder.fresh_value();
    let inner_handler = builder.create_block(vec![(Ty::Word, inner_value)]);
    return_values(&mut builder, vec![inner_value]);
    let outer_value = builder.fresh_value();
    let outer_handler = builder.create_block(vec![(Ty::Word, outer_value)]);
    let observed = builtin(
        &mut builder,
        "checked-outer-catch-handler",
        vec![outer_value],
    );
    return_values(&mut builder, vec![observed]);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: outer_region,
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler: outer_handler,
        cleanup: None,
        catch_tag: Some(outer_tag),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: inner_region,
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler: inner_handler,
        cleanup: None,
        catch_tag: Some(inner_tag),
        binding_targets: Vec::new(),
        depth: 1,
        parent: Some(outer_region),
    });
    builder.finish()
}

fn build_unwind_protect_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(97),
        "checked-unwind-protect-error",
        Vec::new(),
        Vec::new(),
    );
    let region = ncl_ir::HandlerRegionId(3);
    enter_handler(&mut builder, region);
    let value = const_fixnum(&mut builder, 11);
    let before = builtin(&mut builder, "checked-handler-observer", vec![value]);
    leave_handler(&mut builder, region);
    let _after = builtin(&mut builder, "checked-handler-observer", vec![before]);
    return_values(&mut builder, Vec::new());

    let handler = builder.create_block(Vec::new());
    return_values(&mut builder, Vec::new());
    let cleanup = builder.create_block(Vec::new());
    return_values(&mut builder, Vec::new());
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
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
fn enter_progv_error_stops_before_downstream_calls() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    HANDLER_OBSERVER_CALLS.store(0, Ordering::SeqCst);
    ENTER_PROGV_CALLS.store(0, Ordering::SeqCst);
    LEAVE_PROGV_CALLS.store(0, Ordering::SeqCst);
    let abi = CheckedCallsAbi {
        mode: CheckedCallsMode::FailEnterProgv,
    };
    let (code, entry) = compile_and_publish(&build_progv_handler_function(95), &abi);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);

    assert_eq!(ENTER_PROGV_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_OBSERVER_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(LEAVE_PROGV_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn leave_progv_error_stops_before_downstream_call() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    HANDLER_OBSERVER_CALLS.store(0, Ordering::SeqCst);
    ENTER_PROGV_CALLS.store(0, Ordering::SeqCst);
    LEAVE_PROGV_CALLS.store(0, Ordering::SeqCst);
    let abi = CheckedCallsAbi {
        mode: CheckedCallsMode::FailLeaveProgv,
    };
    let (code, entry) = compile_and_publish(&build_progv_handler_function(96), &abi);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);

    assert_eq!(ENTER_PROGV_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_OBSERVER_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_PROGV_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn enter_catch_error_stops_before_downstream_calls() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    HANDLER_OBSERVER_CALLS.store(0, Ordering::SeqCst);
    ENTER_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_CALLS.store(0, Ordering::SeqCst);
    let abi = CheckedCallsAbi {
        mode: CheckedCallsMode::FailEnterCatch,
    };
    let (code, entry) = compile_and_publish(&build_catch_handler_function(98), &abi);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);

    assert_eq!(ENTER_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_OBSERVER_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(LEAVE_CATCH_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn leave_catch_error_stops_before_downstream_call() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    HANDLER_OBSERVER_CALLS.store(0, Ordering::SeqCst);
    ENTER_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_CALLS.store(0, Ordering::SeqCst);
    let abi = CheckedCallsAbi {
        mode: CheckedCallsMode::FailLeaveCatch,
    };
    let (code, entry) = compile_and_publish(&build_catch_handler_function(99), &abi);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);

    assert_eq!(ENTER_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_OBSERVER_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn leave_unwind_protect_error_stops_before_downstream_call() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    HANDLER_OBSERVER_CALLS.store(0, Ordering::SeqCst);
    ENTER_UNWIND_PROTECT_CALLS.store(0, Ordering::SeqCst);
    LEAVE_UNWIND_PROTECT_CALLS.store(0, Ordering::SeqCst);
    let abi = CheckedCallsAbi {
        mode: CheckedCallsMode::FailLeaveUnwindProtect,
    };
    let (code, entry) = compile_and_publish(&build_unwind_protect_function(), &abi);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);

    assert_eq!(ENTER_UNWIND_PROTECT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(HANDLER_OBSERVER_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(LEAVE_UNWIND_PROTECT_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn pending_dispatch_leave_catch_error_reaches_matching_outer_handler() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ENTER_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_CALLS.store(0, Ordering::SeqCst);
    LEAVE_CATCH_SAW_PENDING.store(false, Ordering::SeqCst);
    OUTER_CATCH_HANDLER_CALLS.store(0, Ordering::SeqCst);
    let abi = CheckedCallsAbi {
        mode: CheckedCallsMode::PendingDispatch,
    };
    let (code, entry) = compile_and_publish(&build_pending_dispatch_function(), &abi);
    let mut thread = Thread::new();
    thread.set_multiple_value_area(&[Word::fixnum(2)]);
    let (value, count) = invoke_entry(&code, entry, &mut thread, 0, [0; 4], 0);

    assert_eq!(ENTER_CATCH_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(LEAVE_CATCH_CALLS.load(Ordering::SeqCst), 1);
    assert!(LEAVE_CATCH_SAW_PENDING.load(Ordering::SeqCst));
    assert_eq!(OUTER_CATCH_HANDLER_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!((value, count), (Word::ZERO.bits(), 0));
    assert!(!thread.runtime_error_pending());
    assert!(thread.pending());
}
