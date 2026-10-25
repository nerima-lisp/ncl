use super::*;

static CALL_TARGET_ERROR_CALLS: AtomicUsize = AtomicUsize::new(0);
static CALL_TARGET_DOWNSTREAM_CALLS: AtomicUsize = AtomicUsize::new(0);
static HANDLER_OBSERVER_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTER_PROGV_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_PROGV_CALLS: AtomicUsize = AtomicUsize::new(0);
static ENTER_CATCH_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_CATCH_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_CATCH_SAW_PENDING: AtomicBool = AtomicBool::new(false);
static ENTER_UNWIND_PROTECT_CALLS: AtomicUsize = AtomicUsize::new(0);
static LEAVE_UNWIND_PROTECT_CALLS: AtomicUsize = AtomicUsize::new(0);
static OUTER_CATCH_HANDLER_CALLS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn call_target_error(ctx: &mut Thread, value: u64) -> u64 {
    assert_eq!(value, Word::fixnum(23).bits());
    CALL_TARGET_ERROR_CALLS.fetch_add(1, Ordering::SeqCst);
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}

extern "C" fn call_target_downstream(_ctx: &mut Thread, value: u64) -> u64 {
    CALL_TARGET_DOWNSTREAM_CALLS.fetch_add(1, Ordering::SeqCst);
    value
}

extern "C" fn handler_observer(_ctx: &mut Thread, value: u64) -> u64 {
    HANDLER_OBSERVER_CALLS.fetch_add(1, Ordering::SeqCst);
    value
}

extern "C" fn outer_catch_handler(_ctx: &mut Thread, value: u64) -> u64 {
    OUTER_CATCH_HANDLER_CALLS.fetch_add(1, Ordering::SeqCst);
    value
}

extern "C" fn enter_progv_success(
    _ctx: &mut Thread,
    _region: u64,
    _symbols: u64,
    _values: u64,
) -> u64 {
    ENTER_PROGV_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

extern "C" fn enter_progv_error(ctx: &mut Thread, region: u64, symbols: u64, values: u64) -> u64 {
    enter_progv_success(ctx, region, symbols, values);
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}

extern "C" fn leave_progv_success(_ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_PROGV_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

extern "C" fn leave_progv_error(ctx: &mut Thread, region: u64) -> u64 {
    leave_progv_success(ctx, region);
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}

extern "C" fn enter_catch_success(_ctx: &mut Thread, _region: u64, _depth: u64, _tag: u64) -> u64 {
    ENTER_CATCH_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

extern "C" fn enter_catch_error(ctx: &mut Thread, region: u64, depth: u64, tag: u64) -> u64 {
    enter_catch_success(ctx, region, depth, tag);
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}

extern "C" fn enter_catch_pending(ctx: &mut Thread, region: u64, depth: u64, tag: u64) -> u64 {
    enter_catch_success(ctx, region, depth, tag);
    if region == 1 {
        ctx.set_pending(true);
    }
    Word::NIL.bits()
}

extern "C" fn leave_catch_success(_ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_CATCH_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

extern "C" fn leave_catch_error(ctx: &mut Thread, region: u64) -> u64 {
    leave_catch_success(ctx, region);
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}

extern "C" fn leave_catch_pending_error(ctx: &mut Thread, region: u64) -> u64 {
    assert_eq!(region, 1);
    leave_catch_success(ctx, region);
    let was_pending = ctx.pending();
    LEAVE_CATCH_SAW_PENDING.store(was_pending, Ordering::SeqCst);
    if was_pending {
        set_runtime_error_for_codegen_test(ctx);
    }
    Word::NIL.bits()
}

extern "C" fn enter_unwind_protect(_ctx: &mut Thread, _region: u64, _cleanup: u64) -> u64 {
    ENTER_UNWIND_PROTECT_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

extern "C" fn leave_unwind_protect_success(_ctx: &mut Thread, _region: u64) -> u64 {
    LEAVE_UNWIND_PROTECT_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

extern "C" fn leave_unwind_protect_error(ctx: &mut Thread, region: u64) -> u64 {
    leave_unwind_protect_success(ctx, region);
    set_runtime_error_for_codegen_test(ctx);
    Word::NIL.bits()
}

#[derive(Default)]
struct CheckedCallsAbi {
    mode: CheckedCallsMode,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum CheckedCallsMode {
    #[default]
    Normal,
    FailEnterProgv,
    FailLeaveProgv,
    FailEnterCatch,
    FailLeaveCatch,
    FailLeaveUnwindProtect,
    PendingDispatch,
}

impl RuntimeAbi for CheckedCallsAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, ncl_codegen::AbiError> {
        let address = match identifier.name.as_str() {
            "checked-call-error" => call_target_error as *const () as usize as u64,
            "checked-call-after" => call_target_downstream as *const () as usize as u64,
            "checked-handler-observer" => handler_observer as *const () as usize as u64,
            "checked-outer-catch-handler" => outer_catch_handler as *const () as usize as u64,
            _ => return BuiltinAbi.builtin_address(identifier),
        };
        Ok(address)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, ncl_codegen::AbiError> {
        BuiltinAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, ncl_codegen::AbiError> {
        let address = match function {
            RuntimeFunction::EnterCatch if self.mode == CheckedCallsMode::FailEnterCatch => {
                enter_catch_error as *const () as usize as u64
            }
            RuntimeFunction::EnterCatch if self.mode == CheckedCallsMode::PendingDispatch => {
                enter_catch_pending as *const () as usize as u64
            }
            RuntimeFunction::EnterCatch => enter_catch_success as *const () as usize as u64,
            RuntimeFunction::LeaveCatch if self.mode == CheckedCallsMode::PendingDispatch => {
                leave_catch_pending_error as *const () as usize as u64
            }
            RuntimeFunction::LeaveCatch if self.mode == CheckedCallsMode::FailLeaveCatch => {
                leave_catch_error as *const () as usize as u64
            }
            RuntimeFunction::LeaveCatch => leave_catch_success as *const () as usize as u64,
            RuntimeFunction::EnterProgv if self.mode == CheckedCallsMode::FailEnterProgv => {
                enter_progv_error as *const () as usize as u64
            }
            RuntimeFunction::EnterProgv => enter_progv_success as *const () as usize as u64,
            RuntimeFunction::LeaveProgv if self.mode == CheckedCallsMode::FailLeaveProgv => {
                leave_progv_error as *const () as usize as u64
            }
            RuntimeFunction::LeaveProgv => leave_progv_success as *const () as usize as u64,
            RuntimeFunction::EnterUnwindProtect => {
                enter_unwind_protect as *const () as usize as u64
            }
            RuntimeFunction::LeaveUnwindProtect
                if self.mode == CheckedCallsMode::FailLeaveUnwindProtect =>
            {
                leave_unwind_protect_error as *const () as usize as u64
            }
            RuntimeFunction::LeaveUnwindProtect => {
                leave_unwind_protect_success as *const () as usize as u64
            }
            _ => return BuiltinAbi.runtime_address(function),
        };
        Ok(address)
    }
}

fn compile_and_publish(
    function: &ncl_ir::Function,
    abi: &dyn RuntimeAbi,
) -> (ncl_sys::CodePtr, usize) {
    let compiled = compile_function_x86_64(function, abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    (code, compiled.entry_offset as usize)
}

fn push_value(builder: &mut FunctionBuilder, kind: OpKind, ty: Ty) -> ncl_ir::ValueId {
    builder.push_op(kind, &[ty]).expect("value op")[0]
}

fn push_effect(builder: &mut FunctionBuilder, kind: OpKind) {
    builder.push_op(kind, &[]).expect("effect op");
}

fn enter_handler(builder: &mut FunctionBuilder, region: ncl_ir::HandlerRegionId) {
    push_effect(builder, OpKind::EnterHandler { region });
}

fn leave_handler(builder: &mut FunctionBuilder, region: ncl_ir::HandlerRegionId) {
    push_effect(builder, OpKind::LeaveHandler { region });
}

fn load_arg(builder: &mut FunctionBuilder, index: u8, ty: Ty) -> ncl_ir::ValueId {
    push_value(builder, OpKind::LoadArg { index }, ty)
}

fn const_fixnum(builder: &mut FunctionBuilder, value: i64) -> ncl_ir::ValueId {
    let result = builder.add_constant(Constant::Fixnum(value));
    let integer = push_value(builder, OpKind::Const { result }, Ty::I64);
    push_value(
        builder,
        OpKind::Convert {
            op: ncl_ir::Convert::I64ToWord,
            value: integer,
        },
        Ty::Word,
    )
}

fn const_nil(builder: &mut FunctionBuilder) -> ncl_ir::ValueId {
    let result = builder.add_constant(Constant::Nil);
    push_value(builder, OpKind::Const { result }, Ty::Word)
}

fn builtin(
    builder: &mut FunctionBuilder,
    name: &str,
    args: Vec<ncl_ir::ValueId>,
) -> ncl_ir::ValueId {
    push_effect(builder, OpKind::Safepoint);
    push_value(
        builder,
        OpKind::Builtin {
            name: name.into(),
            args,
            specialization: None,
            specialization_disabled: false,
        },
        Ty::Word,
    )
}

fn return_values(builder: &mut FunctionBuilder, values: Vec<ncl_ir::ValueId>) {
    builder
        .terminate(Terminator::Return { values })
        .expect("return");
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
    let value = load_arg(&mut builder, 0, Ty::Word);
    let result = builtin(&mut builder, "checked-call-error", vec![value]);
    return_values(&mut builder, vec![result]);
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
    let callee = load_arg(&mut builder, 0, Ty::Word);
    let value = load_arg(&mut builder, 1, Ty::Word);
    let argc = const_fixnum(&mut builder, 1);
    push_effect(&mut builder, OpKind::Safepoint);
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
    let result = push_value(&mut builder, call, Ty::Word);
    let downstream = builtin(&mut builder, "checked-call-after", vec![result]);
    return_values(&mut builder, vec![downstream]);
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
    let closure = load_arg(&mut builder, 0, Ty::Word);
    let value = load_arg(&mut builder, 1, Ty::Word);
    let argc = const_fixnum(&mut builder, 1);
    push_effect(&mut builder, OpKind::Safepoint);
    let result = push_value(
        &mut builder,
        OpKind::CallClosure {
            closure,
            args: vec![argc, value],
            named_symbol: None,
        },
        Ty::Word,
    );
    let downstream = builtin(&mut builder, "checked-call-after", vec![result]);
    return_values(&mut builder, vec![downstream]);
    builder.finish()
}

#[path = "checked_calls/handlers.rs"]
mod handlers;

#[test]
fn call_and_call_indirect_errors_stop_before_downstream_builtin() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let abi = CheckedCallsAbi::default();
    let (target_code, target_entry) = compile_and_publish(&build_error_callee(), &abi);
    let target_address = (target_code.address() + target_entry) as u64;

    for (id, indirect, name) in [(92, false, "Call"), (93, true, "CallIndirect")] {
        CALL_TARGET_ERROR_CALLS.store(0, Ordering::SeqCst);
        CALL_TARGET_DOWNSTREAM_CALLS.store(0, Ordering::SeqCst);
        let (caller_code, caller_entry) =
            compile_and_publish(&build_checked_call_caller(id, indirect), &abi);
        let mut thread = Thread::new();
        let (value, count) = invoke_entry(
            &caller_code,
            caller_entry,
            &mut thread,
            2,
            [target_address, Word::fixnum(23).bits(), 0, 0],
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
    let abi = CheckedCallsAbi::default();
    let (target_code, target_entry) = compile_and_publish(&build_error_callee(), &abi);
    let mut closure_object = [0_u64; ncl_object::function_offset::ENTRY + 2];
    let entry_address = target_code.address() + target_entry;
    closure_object[ncl_object::function_offset::ENTRY + 1] =
        Word::fixnum(i64::try_from(entry_address).expect("entry address fits a fixnum")).bits();
    let closure = Word::pointer(closure_object.as_ptr() as usize, ncl_sys::LowTag::Function).bits();
    let (caller_code, caller_entry) = compile_and_publish(&build_closure_caller(), &abi);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &caller_code,
        caller_entry,
        &mut thread,
        2,
        [closure, Word::fixnum(23).bits(), 0, 0],
        0,
    );

    assert_eq!(CALL_TARGET_ERROR_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(CALL_TARGET_DOWNSTREAM_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(value, Word::ZERO.bits());
    assert_eq!(count, 0);
    assert!(thread.runtime_error_pending());
    assert!(!thread.pending());
}
