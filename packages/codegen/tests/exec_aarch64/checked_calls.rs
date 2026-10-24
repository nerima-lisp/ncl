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
    set_runtime_error_for_codegen_test(ctx);
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
        set_runtime_error_for_codegen_test(ctx);
    }
    Word::NIL.bits()
}

pub extern "C" fn leave_catch(ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_CATCH_CALLS.fetch_add(1, Ordering::SeqCst);
    LEAVE_CATCH_SAW_PENDING.store(ctx.pending(), Ordering::SeqCst);
    if LEAVE_CATCH_ERROR.load(Ordering::SeqCst) {
        set_runtime_error_for_codegen_test(ctx);
    }
    Word::NIL.bits()
}

pub extern "C" fn enter_progv(ctx: &mut Thread, _region: u64, _binding: u64) -> u64 {
    ENTER_PROGV_CALLS.fetch_add(1, Ordering::SeqCst);
    if ENTER_PROGV_ERROR.load(Ordering::SeqCst) {
        set_runtime_error_for_codegen_test(ctx);
    }
    Word::NIL.bits()
}

pub extern "C" fn leave_progv(ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_PROGV_CALLS.fetch_add(1, Ordering::SeqCst);
    if LEAVE_PROGV_ERROR.load(Ordering::SeqCst) {
        set_runtime_error_for_codegen_test(ctx);
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
        set_runtime_error_for_codegen_test(ctx);
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
                specialization: None,
                specialization_disabled: false,
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
                specialization: None,
                specialization_disabled: false,
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
                specialization: None,
                specialization_disabled: false,
            },
            &[Ty::Word],
        )
        .expect("error builtin")[0];
    let downstream = builder
        .push_op(
            OpKind::Builtin {
                name: "after-error".into(),
                args: vec![error],
                specialization: None,
                specialization_disabled: false,
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

#[path = "checked_calls/calls.rs"]
mod calls;

#[path = "checked_calls/handlers.rs"]
mod handlers;

#[path = "checked_calls/fixnum.rs"]
mod fixnum;
