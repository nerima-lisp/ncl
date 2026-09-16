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
fn forwards_function_object_from_generated_frame_map() {
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
