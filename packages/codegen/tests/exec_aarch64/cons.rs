use super::*;

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
        if function == RuntimeFunction::SafepointSlow {
            Ok(collect_precise_frame as *const () as usize as u64)
        } else {
            BuiltinAbi.runtime_address(function)
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

fn build_cons_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(6),
        "cons-fast",
        Vec::new(),
        vec![Ty::Word],
    );
    let object = builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .expect("allocation")[0];
    let object_word = convert(&mut builder, Convert::AddressToWord, object, Ty::Word);
    let first_loaded = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![object_word],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("load car")[0];
    let second_loaded = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Cdr,
                args: vec![object_word],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("load cdr")[0];
    let first_loaded = convert(&mut builder, Convert::WordToI64, first_loaded, Ty::I64);
    let second_loaded = convert(&mut builder, Convert::WordToI64, second_loaded, Ty::I64);
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![first_loaded, second_loaded],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("sum")[0];
    let sum = convert(&mut builder, Convert::I64ToWord, sum, Ty::Word);
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .expect("return");

    builder.finish()
}

fn build_cons_store_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(40),
        "cons-store",
        vec![
            Param {
                name: "object".into(),
                ty: Ty::Word,
            },
            Param {
                name: "car".into(),
                ty: Ty::Word,
            },
            Param {
                name: "cdr".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let object = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .expect("load object")[0];
    let car = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .expect("load car argument")[0];
    let cdr = builder
        .push_op(OpKind::LoadArg { index: 2 }, &[Ty::Word])
        .expect("load cdr argument")[0];
    // Cons cells have no header word, so use their headerless stores rather
    // than `StoreField`, whose field index is header-relative.
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Rplaca,
                args: vec![object, car],
                condition: None,
            },
            &[Ty::Unit],
        )
        .expect("store car");
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Rplacd,
                args: vec![object, cdr],
                condition: None,
            },
            &[Ty::Unit],
        )
        .expect("store cdr");
    let first_loaded = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![object],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("load car")[0];
    let second_loaded = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Cdr,
                args: vec![object],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("load cdr")[0];
    let first_loaded = convert(&mut builder, Convert::WordToI64, first_loaded, Ty::I64);
    let second_loaded = convert(&mut builder, Convert::WordToI64, second_loaded, Ty::I64);
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![first_loaded, second_loaded],
                condition: None,
            },
            &[Ty::I64],
        )
        .expect("sum")[0];
    let sum = convert(&mut builder, Convert::I64ToWord, sum, Ty::Word);
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .expect("return");
    builder.finish()
}

#[test]
fn executes_cons_allocation_car_and_cdr_on_tlab_fast_path() {
    let abi = BuiltinAbi;
    let compiled = compile_function_aarch64(&build_cons_function(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    let fast_storage = vec![0_u64; 8].into_boxed_slice();
    let bump = fast_storage.as_ptr() as usize;
    set_tlab(&mut thread, bump, bump + 16);
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        0,
        [0; 4],
        0,
    );
    assert_eq!(value, Word::fixnum(0).bits());
    assert_eq!(count, 1);
    assert_eq!(tlab_bump(&thread), bump + 16);
    let Some(map) = compiled.safepoint_maps.first() else {
        panic!("allocation map missing");
    };
    let end = usize::try_from(map.pc_offset).expect("map offset");
    let word = u32::from_le_bytes(compiled.code[end - 4..end].try_into().expect("instruction"));
    assert_eq!(
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &word.to_le_bytes(), 0)
            .expect("decode BLR")[0]
            .text,
        "blr x17"
    );
}

#[test]
fn executes_cons_stores_and_loads_for_word_argument() {
    let abi = BuiltinAbi;
    let compiled = compile_function_aarch64(&build_cons_store_function(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut storage = Box::new([0_u64; 2]);
    let object = Word::pointer(storage.as_mut_ptr() as usize, ncl_sys::LowTag::List);
    let mut thread = Thread::new();
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        3,
        [
            object.bits(),
            Word::fixnum(10).bits(),
            Word::fixnum(20).bits(),
            0,
        ],
        0,
    );
    assert_eq!(value, Word::fixnum(30).bits());
    assert_eq!(count, 1);
    assert_eq!(*storage, [Word::fixnum(10).bits(), Word::fixnum(20).bits()]);
}

#[test]
fn executes_cons_allocation_on_slow_path() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    ALLOC_SLOW_CALLS.store(0, Ordering::SeqCst);
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(7),
        "cons-slow",
        Vec::new(),
        vec![Ty::Address],
    );
    let object = builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .expect("allocation")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![object],
        })
        .expect("return");
    let abi = BuiltinAbi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let mut thread = Thread::new();
    set_tlab(&mut thread, 1, 1);
    let (value, count) = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        0,
        [0; 4],
        0,
    );
    assert_eq!(value, SLOW_STORAGE.as_ptr() as u64);
    assert_eq!(count, 1);
    assert_eq!(ALLOC_SLOW_CALLS.load(Ordering::SeqCst), 1);
}

#[test]
fn executes_safepoint_poll_without_and_with_request() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(8),
        "safepoint",
        Vec::new(),
        vec![Ty::Word],
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    let value = fixnum_word(&mut builder, 7);
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return");
    let abi = BuiltinAbi;
    let compiled = compile_function_aarch64(&builder.finish(), &abi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    SAFEPOINT_SLOW_CALLS.store(0, Ordering::SeqCst);
    let mut thread = Thread::new();
    enter_native(&mut thread);
    let native_no_request = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        0,
        [0; 4],
        0,
    );
    assert_eq!(native_no_request, (Word::fixnum(7).bits(), 1));
    assert_eq!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst), 0);
    leave_native(&mut thread);
    let no_request = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        0,
        [0; 4],
        0,
    );
    assert_eq!(no_request, (Word::fixnum(7).bits(), 1));
    assert_eq!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst), 0);
    request_safepoint(&mut thread);
    let requested = invoke_entry(
        &code,
        compiled.entry_offset as usize,
        &mut thread,
        0,
        [0; 4],
        0,
    );
    assert_eq!(requested, (Word::fixnum(7).bits(), 1));
    assert_eq!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst), 1);
    let Some(map) = compiled.safepoint_maps.first() else {
        panic!("safepoint map missing");
    };
    let end = usize::try_from(map.pc_offset).expect("map offset");
    let word = u32::from_le_bytes(compiled.code[end - 4..end].try_into().expect("instruction"));
    assert_eq!(
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &word.to_le_bytes(), 0)
            .expect("decode BLR")[0]
            .text,
        "blr x17"
    );
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
