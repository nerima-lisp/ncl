use super::*;

static UNDEFINED_FUNCTION_CALLS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn undefined_function_test_stub(
    _argc: u64,
    _first: u64,
    _second: u64,
    _third: u64,
    _fourth: u64,
    _rest: u64,
) -> u64 {
    UNDEFINED_FUNCTION_CALLS.fetch_add(1, Ordering::SeqCst);
    Word::NIL.bits()
}

fn sys_safepoint(map: &ncl_codegen::SafepointMap) -> ncl_sys::Safepoint {
    ncl_sys::Safepoint {
        pc_offset: map.pc_offset,
        frame_words: map.frame_words,
        slot_words: map.slot_words,
        word_slot_count: map.word_slot_count,
        register_mask: map.register_mask,
        map_flags: map.map_flags,
        slot_bitmap: map.bitmap.clone(),
        register_ids: map.registers.clone(),
    }
}

fn register_real_frame_code(
    thread: &Thread,
    code: &ncl_sys::CodePtr,
    compiled: &ncl_codegen::CompiledFunction,
) {
    let map = compiled.safepoint_maps.first().expect("safepoint map");
    let sys_map = sys_safepoint(map);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&sys_map.pc_offset.to_le_bytes());
    bytes.extend_from_slice(&sys_map.frame_words.to_le_bytes());
    bytes.extend_from_slice(&sys_map.slot_words.to_le_bytes());
    bytes.extend_from_slice(&sys_map.word_slot_count.to_le_bytes());
    bytes.extend_from_slice(&sys_map.register_mask.to_le_bytes());
    bytes.extend_from_slice(&sys_map.map_flags.to_le_bytes());
    bytes.extend_from_slice(&sys_map.slot_bitmap);
    for register in &sys_map.register_ids {
        bytes.extend_from_slice(&register.to_le_bytes());
    }
    ncl_sys::register_code(
        thread,
        code,
        ncl_sys::CodeObjectMetadata {
            entry_offset: compiled.entry_offset as usize,
            size: compiled.code.len(),
            frame_words: map.frame_words,
            function_name: "real-function-object-frame".into(),
            source_locations: Vec::new(),
            constant_slots: Vec::new(),
            safepoint_map: ncl_sys::SafepointMap::decode(&bytes, 1).expect("decode safepoint map"),
            debug_table: Vec::new(),
        },
    )
    .expect("register code metadata");
}

struct PreciseFrameAbi;

impl RuntimeAbi for PreciseFrameAbi {
    fn builtin_address(
        &self,
        identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, ncl_codegen::AbiError> {
        BuiltinAbi.builtin_address(identifier)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, ncl_codegen::AbiError> {
        BuiltinAbi.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, ncl_codegen::AbiError> {
        match function {
            RuntimeFunction::SafepointSlow => {
                Ok(collect_precise_frame as *const () as usize as u64)
            }
            RuntimeFunction::UndefinedFunction => {
                Ok(undefined_function_test_stub as *const () as usize as u64)
            }
            _ => BuiltinAbi.runtime_address(function),
        }
    }
}

extern "C" fn collect_precise_frame(ctx: &mut Thread, frame_fp: usize, return_pc: usize) {
    SAFEPOINT_SLOW_CALLS.fetch_add(1, Ordering::SeqCst);
    ctx.capture_native_frame(frame_fp, return_pc);
    if COLLECT_IN_SAFEPOINT.swap(false, Ordering::SeqCst) {
        FRAME_WORD_BEFORE.store(
            ctx.frame_word(2)
                .expect("captured frame function object")
                .bits(),
            Ordering::SeqCst,
        );
        FRAME_LOCAL_BEFORE.store(
            ctx.frame_word(4).expect("captured live local").bits(),
            Ordering::SeqCst,
        );
        ctx.clear_safepoint_request();
        ncl_sys::collect(ctx, true);
        FRAME_WORD_AFTER.store(
            ctx.last_written_frame_word(2)
                .expect("written-back frame function object")
                .bits(),
            Ordering::SeqCst,
        );
        FRAME_LOCAL_AFTER.store(
            ctx.last_written_frame_word(4)
                .expect("written-back live local")
                .bits(),
            Ordering::SeqCst,
        );
    }
    ctx.clear_safepoint_request();
}

#[test]
fn forwards_function_object_from_generated_frame_map_simulation() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(9),
        "function-object-frame",
        Vec::new(),
        vec![],
    );
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");
    let compiled = compile_function_aarch64(&builder.finish(), &PreciseFrameAbi).expect("lowering");
    let map = compiled.safepoint_maps.first().expect("safepoint map");
    let sys_map = ncl_sys::Safepoint {
        pc_offset: map.pc_offset,
        frame_words: map.frame_words,
        slot_words: map.slot_words,
        word_slot_count: map.word_slot_count,
        register_mask: map.register_mask,
        map_flags: map.map_flags,
        slot_bitmap: map.bitmap.clone(),
        register_ids: map.registers.clone(),
    };
    let old_function = ncl_sys::Word::from_bits(0x1000);
    let moved_function = ncl_sys::Word::from_bits(0x2000);
    let mut frame = vec![ncl_sys::Word::NIL; usize::from(map.frame_words)];
    frame[2] = old_function;
    let updated = ncl_sys::scan_frame(&mut frame, 0, &sys_map, |word| {
        assert_eq!(word, old_function);
        moved_function
    });
    assert_eq!(updated, Some(1));
    assert_eq!(frame[2], moved_function);
}

#[test]
fn forwards_function_object_from_real_frame_after_safepoint_collection() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let runtime = ncl_object::Runtime::new().expect("runtime");
    let mut object_context = ncl_object::ThreadContext::new();
    object_context
        .register(&runtime)
        .expect("register object context");
    let code_object = ncl_object::make_code_object(
        &mut object_context,
        &runtime,
        0,
        0,
        ncl_sys::Word::NIL,
        ncl_sys::Word::NIL,
        ncl_sys::Word::NIL,
    )
    .expect("code object");
    let mut function = Box::new(
        ncl_object::make_simple_fun(
            &mut object_context,
            &runtime,
            0,
            ncl_sys::Word::NIL,
            ncl_sys::Word::NIL,
            code_object,
        )
        .expect("function object")
        .into(),
    );
    object_context.thread_access().enter_native();
    let mut thread = Thread::new();
    object_context
        .thread_access()
        .register_thread_with(&mut thread)
        .expect("register generated thread");
    let _root = ncl_sys::push_root(&mut thread, &mut function);

    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(10),
        "real-function-object-frame",
        vec![Param {
            name: "function".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let live_local = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("live local")[0];
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    builder
        .terminate(Terminator::Return {
            values: vec![live_local],
        })
        .expect("return");
    let compiled = compile_function_aarch64(&builder.finish(), &PreciseFrameAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    register_real_frame_code(&thread, &code, &compiled);

    let slow_before = SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst);
    COLLECT_IN_SAFEPOINT.store(true, Ordering::SeqCst);
    thread.request_poll();
    drop(object_context);
    let old = function.bits();
    let result = invoke_entry_with_function(
        &code,
        compiled.entry_offset as usize,
        std::ptr::from_mut(&mut thread),
        old,
        1,
        [old, 0, 0, 0],
        0,
    );
    COLLECT_IN_SAFEPOINT.store(false, Ordering::SeqCst);
    assert!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst) > slow_before);
    let after = function.bits();
    assert_eq!(result, (after, 1));
    assert_ne!(old, after);
    // Frame word 2 holds the function object; word 4 holds the live local.
    assert_eq!(FRAME_WORD_BEFORE.load(Ordering::SeqCst), old);
    assert_eq!(FRAME_WORD_AFTER.load(Ordering::SeqCst), after);
    assert_eq!(FRAME_LOCAL_BEFORE.load(Ordering::SeqCst), old);
    assert_eq!(FRAME_LOCAL_AFTER.load(Ordering::SeqCst), after);
    let mut name_context = ncl_object::ThreadContext::new();
    name_context
        .register(&runtime)
        .expect("register function-name context");
    assert_eq!(
        ncl_object::function_name(&name_context, ncl_object::Function::from_word(*function),),
        Ok(Word::NIL)
    );
}

#[test]
fn named_call_uses_function_object_forwarded_at_its_safepoint() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let (identity_code, identity_entry) = build_function_object_return_code();
    let named_call = compile_named_call();
    let mut execution =
        prepare_named_call_execution(identity_entry, identity_code.len(), &named_call.compiled);
    let symbol_argument = execution.symbol_object.bits();
    let result = invoke_entry_with_function(
        &execution.caller_code,
        named_call.compiled.entry_offset as usize,
        std::ptr::from_mut(execution.thread.as_mut()),
        Word::NIL.bits(),
        1,
        [symbol_argument, 0, 0, 0],
        0,
    );
    COLLECT_IN_SAFEPOINT.store(false, Ordering::SeqCst);

    let forwarded_function = *execution.function_object;
    let function_cell = ncl_sys::load_object_word_acquire(
        execution.thread.as_ref(),
        *execution.symbol_object,
        ncl_sys::AtomicObjectWordCell::SymbolFunction,
    )
    .expect("forwarded function cell");
    let forwarded_callee = execution
        .thread
        .last_written_frame_word(named_call.function_cell_frame_slot)
        .expect("written-back callee spill");
    assert!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst) > execution.slow_calls_before);
    assert_ne!(execution.initial_function_cell, forwarded_function);
    assert_ne!(execution.initial_function_cell, function_cell);
    assert_eq!(function_cell, forwarded_function);
    assert_eq!(forwarded_callee, forwarded_function);
    assert_eq!(result, (forwarded_function.bits(), 1));
    assert_ne!(result.0, execution.initial_function_cell.bits());
    assert_eq!(UNDEFINED_FUNCTION_CALLS.load(Ordering::SeqCst), 0);
}

struct NamedCallArtifact {
    compiled: ncl_codegen::CompiledFunction,
    function_cell_frame_slot: usize,
}

struct NamedCallExecution {
    thread: Box<Thread>,
    caller_code: ncl_sys::CodePtr,
    function_object: Box<Word>,
    symbol_object: Box<Word>,
    initial_function_cell: Word,
    slow_calls_before: usize,
    _runtime: ncl_object::Runtime,
}

fn build_function_object_return_code() -> (ncl_sys::CodePtr, usize) {
    let mut identity_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(11),
        "return-function-object",
        Vec::new(),
        vec![Ty::Word],
    );
    let self_function = identity_builder
        .push_op(OpKind::LoadFunctionObject, &[Ty::Word])
        .expect("load callee function object")[0];
    identity_builder
        .terminate(Terminator::Return {
            values: vec![self_function],
        })
        .expect("callee return");
    let identity_artifact =
        compile_function_aarch64(&identity_builder.finish(), &BuiltinAbi).expect("callee lowering");
    let mut identity_code =
        alloc_code(identity_artifact.code.len()).expect("callee code allocation");
    write_code(&mut identity_code, 0, &identity_artifact.code).expect("callee code write");
    publish_code(&mut identity_code).expect("callee code publication");
    let identity_entry = identity_code
        .address()
        .checked_add(
            usize::try_from(identity_artifact.entry_offset)
                .expect("callee entry offset fits usize"),
        )
        .expect("callee entry address");
    (identity_code, identity_entry)
}

fn compile_named_call() -> NamedCallArtifact {
    let mut named_call_builder = FunctionBuilder::new(
        ncl_ir::FunctionId(12),
        "named-call-after-forwarding",
        vec![Param {
            name: "symbol".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let symbol = ncl_ir::ValueId(0);
    let function_field =
        u32::try_from(ncl_object::symbol_offset::FUNCTION).expect("function field fits u32");
    let function_cell_value = named_call_builder
        .push_op(
            OpKind::LoadField {
                object: symbol,
                field: function_field,
            },
            &[Ty::Word],
        )
        .expect("load named function cell")[0];
    let argc = fixnum_word(&mut named_call_builder, 0);
    named_call_builder
        .push_op(OpKind::Safepoint, &[])
        .expect("call safepoint");
    let result = named_call_builder
        .push_op(
            OpKind::CallClosure {
                closure: function_cell_value,
                args: vec![argc],
                named_symbol: Some(symbol),
            },
            &[Ty::Word],
        )
        .expect("named closure call")[0];
    named_call_builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("caller return");
    let named_call_function = named_call_builder.finish();
    let allocation =
        ncl_codegen::allocate(&named_call_function, ncl_codegen::AllocationTarget::AArch64);
    let Some(ncl_codegen::Location::Spill(function_cell_spill)) =
        allocation.location(function_cell_value)
    else {
        panic!("callee crossing the safepoint must be spilled");
    };
    let function_cell_frame_slot = usize::try_from(ncl_codegen::FRAME_HEADER_WORDS)
        .expect("frame header size fits usize")
        .checked_add(usize::try_from(function_cell_spill).expect("callee spill fits usize"))
        .expect("callee frame slot");
    let compiled =
        compile_function_aarch64(&named_call_function, &PreciseFrameAbi).expect("caller lowering");
    let map = compiled
        .safepoint_maps
        .first()
        .expect("explicit safepoint map");
    let sys_map = sys_safepoint(map);
    assert!(
        ncl_sys::SafepointMap::is_slot_live(&sys_map, function_cell_frame_slot),
        "the direct-call callee spill must be rooted at the explicit safepoint"
    );
    NamedCallArtifact {
        compiled,
        function_cell_frame_slot,
    }
}

fn prepare_named_call_execution(
    identity_entry: usize,
    identity_code_size: usize,
    caller_artifact: &ncl_codegen::CompiledFunction,
) -> NamedCallExecution {
    let runtime = ncl_object::Runtime::new().expect("runtime");
    let mut object_context = ncl_object::ThreadContext::new();
    object_context
        .register(&runtime)
        .expect("register object context");
    let identity_code_object = ncl_object::make_code_object(
        &mut object_context,
        &runtime,
        identity_entry,
        identity_code_size,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .expect("callee code object");
    let mut function_object = Box::new(
        ncl_object::make_simple_fun(
            &mut object_context,
            &runtime,
            identity_entry,
            Word::NIL,
            Word::NIL,
            identity_code_object,
        )
        .expect("callee function object")
        .as_word(),
    );
    let _setup_function_root = ncl_object::push_root(&mut object_context, &mut function_object);
    let name = ncl_object::make_string(
        &mut object_context,
        &runtime,
        &['D', 'I', 'R', 'E', 'C', 'T'],
    )
    .expect("symbol name");
    let mut symbol_object = Box::new(
        ncl_object::make_symbol(&mut object_context, &runtime, name).expect("function symbol"),
    );
    let _setup_symbol_root = ncl_object::push_root(&mut object_context, &mut symbol_object);
    object_context
        .write_object_slot(
            *symbol_object,
            ncl_object::symbol_offset::FUNCTION,
            *function_object,
        )
        .expect("publish function cell");

    object_context.thread_access().enter_native();
    let mut thread = Box::new(Thread::new());
    object_context
        .thread_access()
        .register_thread_with(thread.as_mut())
        .expect("register generated thread");
    let _function_root = ncl_sys::push_root(thread.as_mut(), &mut function_object);
    let _symbol_root = ncl_sys::push_root(thread.as_mut(), &mut symbol_object);
    let mut caller_code = alloc_code(caller_artifact.code.len()).expect("caller code allocation");
    write_code(&mut caller_code, 0, &caller_artifact.code).expect("caller code write");
    publish_code(&mut caller_code).expect("caller code publication");
    register_real_frame_code(thread.as_ref(), &caller_code, caller_artifact);

    let original_function_cell = *function_object;
    let initial_function_cell = ncl_sys::load_object_word_acquire(
        thread.as_ref(),
        *symbol_object,
        ncl_sys::AtomicObjectWordCell::SymbolFunction,
    )
    .expect("initial function cell");
    assert_eq!(initial_function_cell, original_function_cell);
    assert_ne!(initial_function_cell, Word::UNBOUND);
    let slow_calls_before = SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst);
    UNDEFINED_FUNCTION_CALLS.store(0, Ordering::SeqCst);
    COLLECT_IN_SAFEPOINT.store(true, Ordering::SeqCst);
    thread.request_poll();
    drop(object_context);
    NamedCallExecution {
        thread,
        caller_code,
        function_object,
        symbol_object,
        initial_function_cell,
        slow_calls_before,
        _runtime: runtime,
    }
}
