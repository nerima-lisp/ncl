use super::*;

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
    let first_constant = builder.add_constant(Constant::Fixnum(10));
    let car = builder
        .push_op(
            OpKind::Const {
                result: first_constant,
            },
            &[Ty::Word],
        )
        .expect("car")[0];
    let second_constant = builder.add_constant(Constant::Fixnum(20));
    let cdr = builder
        .push_op(
            OpKind::Const {
                result: second_constant,
            },
            &[Ty::Word],
        )
        .expect("cdr")[0];
    builder
        .push_op(
            OpKind::StoreField {
                object,
                field: 0,
                value: car,
            },
            &[],
        )
        .expect("store car");
    builder
        .push_op(
            OpKind::StoreField {
                object,
                field: 1,
                value: cdr,
            },
            &[],
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
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![first_loaded, second_loaded],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("sum")[0];
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
    assert_eq!(value, abi.encode_fixnum(30) as u64);
    assert_eq!(count, 1);
    assert_eq!(tlab_bump(&thread), bump + 16);
    let Some(map) = compiled.safepoint_maps.first() else {
        panic!("allocation map missing");
    };
    let end = usize::try_from(map.pc_offset).expect("map offset");
    let word = u32::from_le_bytes(compiled.code[end - 4..end].try_into().expect("instruction"));
    assert_eq!(
        ncl_asm_aarch64::decode(word),
        Ok(ncl_asm_aarch64::Inst::Blr {
            rn: ncl_asm_aarch64::Reg(17)
        })
    );
}

#[test]
fn executes_cons_allocation_on_slow_path() {
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
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(8),
        "safepoint",
        Vec::new(),
        vec![Ty::Word],
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    let constant = builder.add_constant(Constant::Fixnum(7));
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("constant")[0];
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
    assert_eq!(native_no_request, (abi.encode_fixnum(7) as u64, 1));
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
    assert_eq!(no_request, (abi.encode_fixnum(7) as u64, 1));
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
    assert_eq!(requested, (abi.encode_fixnum(7) as u64, 1));
    assert_eq!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst), 1);
    let Some(map) = compiled.safepoint_maps.first() else {
        panic!("safepoint map missing");
    };
    let end = usize::try_from(map.pc_offset).expect("map offset");
    let word = u32::from_le_bytes(compiled.code[end - 4..end].try_into().expect("instruction"));
    assert_eq!(
        ncl_asm_aarch64::decode(word),
        Ok(ncl_asm_aarch64::Inst::Blr {
            rn: ncl_asm_aarch64::Reg(17)
        })
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
    let compiled = compile_function_aarch64(&builder.finish(), &BuiltinAbi).expect("lowering");
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
    ncl_sys::enter_native(object_context.thread_mut());
    let mut thread = Thread::new();
    ncl_sys::register_thread_with_thread(object_context.thread_mut(), &mut thread)
        .expect("register generated thread");
    let _root = ncl_sys::push_root(&mut thread, &mut function);

    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(10),
        "real-function-object-frame",
        Vec::new(),
        vec![],
    );
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .expect("return");
    let compiled = compile_function_aarch64(&builder.finish(), &BuiltinAbi).expect("lowering");
    let mut code = alloc_code(compiled.code.len()).expect("code allocation");
    write_code(&mut code, 0, &compiled.code).expect("code write");
    publish_code(&mut code).expect("code publication");
    let map = compiled.safepoint_maps.first().expect("safepoint map");
    ncl_sys::register_code(
        &thread,
        &code,
        ncl_sys::CodeObjectMetadata {
            entry_offset: compiled.entry_offset as usize,
            size: compiled.code.len(),
            frame_words: map.frame_words,
            function_name: "real-function-object-frame".into(),
            source_locations: Vec::new(),
            constant_slots: Vec::new(),
            safepoint_map: {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(&map.pc_offset.to_le_bytes());
                bytes.extend_from_slice(&map.frame_words.to_le_bytes());
                bytes.extend_from_slice(&map.slot_words.to_le_bytes());
                bytes.extend_from_slice(&map.word_slot_count.to_le_bytes());
                bytes.extend_from_slice(&map.register_mask.to_le_bytes());
                bytes.extend_from_slice(&map.map_flags.to_le_bytes());
                bytes.extend_from_slice(&map.bitmap);
                for register in &map.registers {
                    bytes.extend_from_slice(&register.to_le_bytes());
                }
                ncl_sys::SafepointMap::decode(&bytes, 1).expect("decode safepoint map")
            },
            debug_table: Vec::new(),
        },
    )
    .expect("register code metadata");

    ncl_sys::unregister_thread(object_context.thread_mut());
    COLLECT_IN_SAFEPOINT.store(true, Ordering::SeqCst);
    thread.request_poll();
    let old = function.bits();
    let result = invoke_entry_with_function(
        &code,
        compiled.entry_offset as usize,
        std::ptr::from_mut(&mut thread),
        old,
        0,
        [0; 4],
        0,
    );
    COLLECT_IN_SAFEPOINT.store(false, Ordering::SeqCst);
    ncl_sys::register_thread_with_thread(&thread, object_context.thread_mut())
        .expect("re-register object context");
    assert_eq!(result, (0, 0));
    assert_eq!(SAFEPOINT_SLOW_CALLS.load(Ordering::SeqCst), 1);
    let after = function.bits();
    assert_ne!(old, after);
    assert_eq!(FRAME_WORD_BEFORE.load(Ordering::SeqCst), old);
    assert_eq!(FRAME_WORD_AFTER.load(Ordering::SeqCst), after);
    assert_eq!(
        ncl_object::function_name(&object_context, (*function).into()),
        Ok(Word::NIL)
    );
}
