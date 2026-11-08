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
        &raw mut thread,
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
        &raw mut thread,
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
        &raw mut thread,
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
        &raw mut thread,
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
        &raw mut thread,
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
        &raw mut thread,
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

#[path = "cons/forwarding.rs"]
mod forwarding;
