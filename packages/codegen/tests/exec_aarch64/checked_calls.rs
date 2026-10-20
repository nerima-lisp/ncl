use super::*;

static CALL_TARGET_ERROR_CALLS: AtomicUsize = AtomicUsize::new(0);
static CALL_TARGET_DOWNSTREAM_CALLS: AtomicUsize = AtomicUsize::new(0);
static HANDLER_DOWNSTREAM_CALLS: AtomicUsize = AtomicUsize::new(0);
static PENDING_EXIT_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTER_CATCH_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_CATCH_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTER_CATCH_ERROR: AtomicBool = AtomicBool::new(false);
static LEAVE_CATCH_ERROR: AtomicBool = AtomicBool::new(false);
static LEAVE_CATCH_SAW_PENDING: AtomicBool = AtomicBool::new(false);
static ENTER_PROGV_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_PROGV_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTER_PROGV_ERROR: AtomicBool = AtomicBool::new(false);
static LEAVE_PROGV_ERROR: AtomicBool = AtomicBool::new(false);
static ENTER_UNWIND_PROTECT_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_UNWIND_PROTECT_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_UNWIND_PROTECT_ERROR: AtomicBool = AtomicBool::new(false);

pub extern "C" fn call_target_error(ctx: &mut Thread, _value: u64) -> u64 {
    CALL_TARGET_ERROR_CALLS.fetch_add(1, Ordering::SeqCst);
    ctx.set_runtime_error_pending(RuntimeErrorSource::Object, true);
    ctx.set_pending(true);
    Word::NIL.bits()
}

pub extern "C" fn after_call_error(_ctx: &mut Thread, value: u64) -> u64 {
    CALL_TARGET_DOWNSTREAM_CALLS.fetch_add(1, Ordering::SeqCst);
    value
}

pub extern "C" fn after_handler_error(_ctx: &mut Thread, value: u64) -> u64 {
    HANDLER_DOWNSTREAM_CALLS.fetch_add(1, Ordering::SeqCst);
    value
}

pub extern "C" fn set_pending_exit(ctx: &mut Thread, tag: u64, payload: u64) -> u64 {
    PENDING_EXIT_CALLS.fetch_add(1, Ordering::SeqCst);
    ctx.set_multiple_value_area(&[Word::from_bits(tag), Word::from_bits(payload)]);
    ctx.set_pending(true);
    Word::NIL.bits()
}

pub extern "C" fn enter_catch(ctx: &mut Thread, _region: u64, _depth: u64, _tag: u64) -> u64 {
    ENTER_CATCH_CALLS.fetch_add(1, Ordering::SeqCst);
    if ENTER_CATCH_ERROR.load(Ordering::SeqCst) {
        ctx.set_runtime_error_pending(RuntimeErrorSource::Object, true);
        ctx.set_pending(true);
    }
    Word::NIL.bits()
}

pub extern "C" fn leave_catch(ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_CATCH_CALLS.fetch_add(1, Ordering::SeqCst);
    LEAVE_CATCH_SAW_PENDING.store(ctx.pending(), Ordering::SeqCst);
    if LEAVE_CATCH_ERROR.load(Ordering::SeqCst) {
        ctx.set_runtime_error_pending(RuntimeErrorSource::Object, true);
        ctx.set_pending(true);
    }
    Word::NIL.bits()
}

pub extern "C" fn enter_progv(ctx: &mut Thread, _region: u64, _binding: u64) -> u64 {
    ENTER_PROGV_CALLS.fetch_add(1, Ordering::SeqCst);
    if ENTER_PROGV_ERROR.load(Ordering::SeqCst) {
        ctx.set_runtime_error_pending(RuntimeErrorSource::Object, true);
        ctx.set_pending(true);
    }
    Word::NIL.bits()
}

pub extern "C" fn leave_progv(ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_PROGV_CALLS.fetch_add(1, Ordering::SeqCst);
    if LEAVE_PROGV_ERROR.load(Ordering::SeqCst) {
        ctx.set_runtime_error_pending(RuntimeErrorSource::Object, true);
        ctx.set_pending(true);
    }
    Word::NIL.bits()
}

pub extern "C" fn enter_unwind_protect(_ctx: &mut Thread, _region: u64, _cleanup: u64) -> u64 {
    ENTER_UNWIND_PROTECT_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

pub extern "C" fn leave_unwind_protect(ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_UNWIND_PROTECT_CALLS.fetch_add(1, Ordering::SeqCst);
    if LEAVE_UNWIND_PROTECT_ERROR.load(Ordering::SeqCst) {
        ctx.set_runtime_error_pending(RuntimeErrorSource::Object, true);
        ctx.set_pending(true);
    }
    Word::NIL.bits()
}

fn compile_and_publish(function: &ncl_ir::Function) -> (ncl_sys::CodePtr, usize) {
    let compiled = compile_function_aarch64(function, &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    (code, compiled.entry_offset as usize)
}

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
    let argc = builder.add_constant(Constant::Fixnum(1));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc")[0];
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
    let argc = builder.add_constant(Constant::Fixnum(1));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc")[0];
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

fn build_handler_failure_function(id: u32, leave_before_downstream: bool) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        "checked-handler-error",
        Vec::new(),
        vec![Ty::Word],
    );
    let catch_tag = builder.add_constant(Constant::Fixnum(17));
    let catch_tag = builder
        .push_op(OpKind::Const { result: catch_tag }, &[Ty::Word])
        .expect("catch tag")[0];
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
    let value = builder.add_constant(Constant::Fixnum(23));
    let value = builder
        .push_op(OpKind::Const { result: value }, &[Ty::Word])
        .expect("downstream value")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-handler-error".into(),
                args: vec![value],
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
    let catch_tag = builder.add_constant(Constant::Fixnum(17));
    let catch_tag = builder
        .push_op(OpKind::Const { result: catch_tag }, &[Ty::Word])
        .expect("catch tag")[0];
    builder
        .push_op(
            OpKind::EnterHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            &[],
        )
        .expect("enter handler");
    let pending_tag = builder.add_constant(Constant::Fixnum(19));
    let pending_tag = builder
        .push_op(
            OpKind::Const {
                result: pending_tag,
            },
            &[Ty::Word],
        )
        .expect("pending tag")[0];
    let payload = builder.add_constant(Constant::Fixnum(29));
    let payload = builder
        .push_op(OpKind::Const { result: payload }, &[Ty::Word])
        .expect("pending payload")[0];
    let pending_value = builder
        .push_op(
            OpKind::Builtin {
                name: "set-pending-exit".into(),
                args: vec![pending_tag, payload],
            },
            &[Ty::Word],
        )
        .expect("pending exit builtin")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-handler-error".into(),
                args: vec![pending_value],
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
    let binding = builder.add_constant(Constant::Fixnum(31));
    let binding = builder
        .push_op(OpKind::Const { result: binding }, &[Ty::Word])
        .expect("binding target")[0];
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
    let value = builder.add_constant(Constant::Fixnum(37));
    let value = builder
        .push_op(OpKind::Const { result: value }, &[Ty::Word])
        .expect("downstream value")[0];
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
fn executes_builtin_call_with_context_and_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(4),
        "builtin-add",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("left")[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("right")[0];
    let result = builder
        .push_op(
            OpKind::Builtin {
                name: "add".into(),
                args: vec![left, right],
            },
            &[Ty::Word],
        )
        .expect("builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");
    let abi = BuiltinAbi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        2,
        [Word::fixnum(4).bits(), Word::fixnum(5).bits(), 0, 0],
        0,
    );
    assert_eq!(value, Word::fixnum(9).bits());
    assert_eq!(count, 1);
}

#[test]
fn make_closure_error_is_checked_before_capture_store() {
    MAKE_CLOSURE_ERROR_CALLS.store(0, Ordering::SeqCst);
    MAKE_CLOSURE_CAPTURE_COUNT.store(0, Ordering::SeqCst);
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(36),
        "make-closure-error",
        vec![Param {
            name: "capture".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let capture = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("capture argument")[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry: capture,
                captures: vec![capture],
            },
            &[Ty::Word],
        )
        .expect("make closure")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![closure],
        })
        .expect("return");

    let compiled = compile_function_aarch64(&builder.finish(), &BuiltinAbi).expect("lowering");
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

    assert_eq!(MAKE_CLOSURE_ERROR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        MAKE_CLOSURE_CAPTURE_COUNT.load(Ordering::SeqCst),
        Word::fixnum(1).bits()
    );
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn make_value_cell_error_stops_before_downstream_builtin() {
    MAKE_VALUE_CELL_ERROR_CALLS.store(0, Ordering::SeqCst);
    MAKE_VALUE_CELL_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
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
        .expect("make value cell")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-value-cell-error".into(),
                args: vec![cell],
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![downstream],
        })
        .expect("return");

    let compiled = compile_function_aarch64(&builder.finish(), &BuiltinAbi).expect("lowering");
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

    assert_eq!(MAKE_VALUE_CELL_ERROR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(MAKE_VALUE_CELL_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}

#[test]
fn builtin_error_stops_before_downstream_builtin() {
    BUILTIN_ERROR_CALLS.store(0, Ordering::SeqCst);
    BUILTIN_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
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
    let error = builder
        .push_op(
            OpKind::Builtin {
                name: "error-return".into(),
                args: vec![value],
            },
            &[Ty::Word],
        )
        .expect("error builtin")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-error".into(),
                args: vec![error],
            },
            &[Ty::Word],
        )
        .expect("downstream builtin")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![downstream],
        })
        .expect("return");

    let compiled = compile_function_aarch64(&builder.finish(), &BuiltinAbi).expect("lowering");
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

    assert_eq!(BUILTIN_ERROR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(BUILTIN_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
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
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
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
