use super::*;

fn register_safepoint_code(
    thread: &Thread,
    code: &ncl_sys::CodePtr,
    compiled: &ncl_codegen::CompiledFunction,
    name: &str,
) {
    let map = compiled.safepoint_maps.first().expect("tail safepoint map");
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
            function_name: name.into(),
            source_locations: Vec::new(),
            constant_slots: Vec::new(),
            safepoint_map: ncl_sys::SafepointMap::decode(&bytes, 1).expect("decode safepoint map"),
            debug_table: Vec::new(),
        },
    )
    .expect("register tail code metadata");
}

fn build_mutual_tail_function(id: u32, name: &str) -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(id),
        name,
        vec![
            Param {
                name: "self".into(),
                ty: Ty::Address,
            },
            Param {
                name: "next".into(),
                ty: Ty::Address,
            },
            Param {
                name: "remaining".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let self_address = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Address])
        .expect("self address")[0];
    let next_address = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Address])
        .expect("next address")[0];
    let remaining = builder
        .push_op(OpKind::LoadArg { index: 2 }, &[Ty::Word])
        .expect("remaining depth")[0];
    builder.push_op(OpKind::Safepoint, &[]).expect("safepoint");
    let zero = builder.add_constant(Constant::Fixnum(0));
    let zero = builder
        .push_op(OpKind::Const { result: zero }, &[Ty::Word])
        .expect("zero")[0];
    let done = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Le,
                left: remaining,
                right: zero,
            },
            &[Ty::Word],
        )
        .expect("depth comparison")[0];
    let base_value = builder.fresh_value();
    let recurse_self = builder.fresh_value();
    let recurse_next = builder.fresh_value();
    let recurse_remaining = builder.fresh_value();
    let base = builder.create_block(vec![(Ty::Word, base_value)]);
    let recurse = builder.create_block(vec![
        (Ty::Address, recurse_self),
        (Ty::Address, recurse_next),
        (Ty::Word, recurse_remaining),
    ]);
    builder
        .position_at(ncl_ir::BlockId(0))
        .expect("entry block");
    builder
        .terminate(Terminator::Branch {
            condition: done,
            then_target: base,
            then_args: vec![remaining],
            else_target: recurse,
            else_args: vec![next_address, self_address, remaining],
        })
        .expect("tail branch");
    builder.position_at(base).expect("base block");
    builder
        .terminate(Terminator::Return {
            values: vec![base_value],
        })
        .expect("base return");
    builder.position_at(recurse).expect("recurse block");
    let one = builder.add_constant(Constant::Fixnum(1));
    let one = builder
        .push_op(OpKind::Const { result: one }, &[Ty::Word])
        .expect("one")[0];
    let next_remaining = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![recurse_remaining, one],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("decrement")[0];
    builder
        .terminate(Terminator::TailCall {
            function: recurse_self,
            args: vec![recurse_self, recurse_next, next_remaining],
        })
        .expect("mutual tail call");
    builder.finish()
}

#[test]
fn executes_one_million_mutual_tail_calls_under_gc_stress() {
    let _guard = TEST_SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
    TAIL_GC_POLLS.store(0, Ordering::SeqCst);
    TAIL_GC_COLLECTIONS.store(0, Ordering::SeqCst);
    let runtime = ncl_object::Runtime::new().expect("runtime");
    let mut object_context = ncl_object::ThreadContext::new();
    object_context
        .register(&runtime)
        .expect("register object context");
    ncl_sys::enter_native(object_context.thread_mut());
    let mut thread = Thread::new();
    ncl_sys::register_thread_with_thread(object_context.thread_mut(), &mut thread)
        .expect("register generated thread");

    let abi = BuiltinAbi;
    let first_ir = build_mutual_tail_function(101, "mutual-tail-a");
    let first = compile_function_aarch64(&first_ir, &abi).expect("first lowering");
    let second_ir = build_mutual_tail_function(102, "mutual-tail-b");
    let second = compile_function_aarch64(&second_ir, &abi).expect("second lowering");
    let mut first_code = alloc_code(first.code.len()).expect("first code allocation");
    write_code(&mut first_code, 0, &first.code).expect("first code write");
    publish_code(&mut first_code).expect("first code publication");
    let mut second_code = alloc_code(second.code.len()).expect("second code allocation");
    write_code(&mut second_code, 0, &second.code).expect("second code write");
    publish_code(&mut second_code).expect("second code publication");
    register_safepoint_code(&thread, &first_code, &first, "mutual-tail-a");
    register_safepoint_code(&thread, &second_code, &second, "mutual-tail-b");

    TAIL_GC_STRESS.store(true, Ordering::SeqCst);
    thread.request_poll();
    let result = invoke_entry(
        &first_code,
        first.entry_offset as usize,
        &mut thread,
        3,
        [
            first_code.address() as u64,
            second_code.address() as u64,
            Word::fixnum(1_000_000).bits(),
            0,
        ],
        0,
    );
    TAIL_GC_STRESS.store(false, Ordering::SeqCst);
    ncl_sys::unregister_thread(&thread);
    assert_eq!(result, (Word::fixnum(0).bits(), 1));
    let polls = TAIL_GC_POLLS.load(Ordering::SeqCst);
    let collections = TAIL_GC_COLLECTIONS.load(Ordering::SeqCst);
    assert!(polls >= 1_000_000);
    assert!(
        collections >= 2,
        "expected repeated full GC, got {collections}"
    );
}
