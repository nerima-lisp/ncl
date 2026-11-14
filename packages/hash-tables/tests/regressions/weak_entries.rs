use super::*;
use ncl_codegen::{AbiError, ContextField, RuntimeAbi, RuntimeFunction};
use ncl_ir::{FunctionBuilder, OpKind, Terminator};

std::thread_local! {
    static PRECISE_COLLECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct PreciseCollectionAbi<T>(T);

impl<T: RuntimeAbi> RuntimeAbi for PreciseCollectionAbi<T> {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        self.0.builtin_address(identifier)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        self.0.field_offset(field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        if function == RuntimeFunction::SafepointSlow {
            Ok(collect_precisely as *const () as usize as u64)
        } else {
            self.0.runtime_address(function)
        }
    }
}

extern "C" fn collect_precisely(thread: &mut ncl_sys::Thread, frame_fp: usize, return_pc: usize) {
    thread.capture_native_frame(frame_fp, return_pc);
    if !thread.frame_snapshot_failed() {
        ncl_sys::collect(thread, true);
        PRECISE_COLLECTIONS.with(|count| count.set(count.get() + 1));
    }
    thread.clear_safepoint_request();
}

fn compile_collection_function(function: &ncl_ir::Function) -> ncl_codegen::CompiledFunction {
    #[cfg(target_arch = "aarch64")]
    let compiled = ncl_codegen::compile_function_aarch64(
        function,
        &PreciseCollectionAbi(ncl_codegen::Aarch64Abi),
    );
    #[cfg(target_arch = "x86_64")]
    let compiled = ncl_codegen::compile_function_x86_64(
        function,
        &PreciseCollectionAbi(ncl_codegen::X86_64Abi),
    );
    compiled.unwrap_or_else(|error| panic!("lower precise collection function: {error:?}"))
}

pub fn collect_at_precise_safepoint(ctx: &mut ThreadContext) {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(1),
        "weak-table-precise-collection",
        Vec::new(),
        Vec::new(),
    );
    builder
        .push_op(OpKind::Safepoint, &[])
        .unwrap_or_else(|error| panic!("build safepoint: {error:?}"));
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .unwrap_or_else(|error| panic!("build return: {error:?}"));
    let compiled = compile_collection_function(&builder.finish());
    assert_eq!(compiled.safepoint_maps.len(), 1);
    let map = &compiled.safepoint_maps[0];
    let mut encoded_maps = Vec::new();
    for map in &compiled.safepoint_maps {
        encoded_maps.extend(
            map.encode()
                .unwrap_or_else(|error| panic!("encode safepoint map: {error:?}")),
        );
    }
    let mut code = ncl_sys::alloc_code(compiled.code.len())
        .unwrap_or_else(|error| panic!("allocate code: {error:?}"));
    ncl_sys::write_code(&mut code, 0, &compiled.code)
        .unwrap_or_else(|error| panic!("write code: {error:?}"));
    ncl_sys::publish_code(&mut code).unwrap_or_else(|error| panic!("publish code: {error:?}"));
    let metadata = ncl_sys::CodeObjectMetadata {
        entry_offset: compiled.entry_offset as usize,
        size: compiled.code.len(),
        frame_words: map.frame_words,
        function_name: "weak-table-precise-collection".into(),
        source_locations: Vec::new(),
        constant_slots: Vec::new(),
        safepoint_map: ncl_sys::SafepointMap::decode(&encoded_maps, compiled.safepoint_maps.len())
            .unwrap_or_else(|error| panic!("decode safepoint maps: {error:?}")),
        debug_table: Vec::new(),
    };
    ctx.thread_access()
        .register_code(&code, metadata)
        .unwrap_or_else(|error| panic!("register collection code: {error:?}"));

    PRECISE_COLLECTIONS.with(|count| count.set(0));
    ctx.thread_access().request_poll();
    let (_, count) =
        ctx.thread_access()
            .invoke_entry(&code, compiled.entry_offset as usize, 0, [0; 4], 0);
    assert_eq!(count, 0);
    PRECISE_COLLECTIONS.with(|count| assert_eq!(count.get(), 1));
    assert!(ctx.thread_access().unregister_code(&code).is_some());
}

#[test]
fn weak_key_entry_is_removed_when_key_dies() {
    let (runtime, mut ctx) = setup();
    let mut table_word = HashTable::new(&mut ctx, &runtime, HashTest::Eq, Weakness::Key)
        .unwrap_or_else(|error| panic!("table allocation failed: {error:?}"))
        .as_word();
    let table_token = ncl_object::push_root(&mut ctx, &mut table_word);
    let mut key = string(&mut ctx, &runtime, "weak-key");
    let key_token = ncl_object::push_root(&mut ctx, &mut key);
    let mut value = string(&mut ctx, &runtime, "weak-value");
    let value_token = ncl_object::push_root(&mut ctx, &mut value);
    HashTable::from_word(table_word)
        .insert(&mut ctx, &runtime, key, value)
        .unwrap_or_else(|error| panic!("weak-key insertion failed: {error:?}"));
    assert_eq!(HashTable::from_word(table_word).count(&ctx), Ok(1));
    assert!(ncl_object::pop_root(&mut ctx, value_token));
    assert!(ncl_object::pop_root(&mut ctx, key_token));
    // A generated safepoint keeps stale Rust stack words outside the precise root map.
    collect_at_precise_safepoint(&mut ctx);
    assert_eq!(HashTable::from_word(table_word).count(&ctx), Ok(0));
    assert_ne!(std::hint::black_box(key).bits(), Word::NIL.bits());
    assert!(ncl_object::pop_root(&mut ctx, table_token));
}

#[test]
fn weak_value_entry_is_removed_when_value_dies() {
    weak_entry_helpers::weak_entry_after_gc(Weakness::Value, true, false, 0, Word::NIL, true);
}

#[test]
fn weak_key_and_value_entry_requires_both_referents() {
    weak_entry_helpers::weak_entry_after_gc(
        Weakness::KeyAndValue,
        false,
        false,
        0,
        Word::NIL,
        true,
    );
}

#[test]
fn weak_key_or_value_entry_survives_when_key_is_live() {
    weak_entry_helpers::weak_entry_after_gc(
        Weakness::KeyOrValue,
        true,
        false,
        1,
        Word::fixnum(99),
        false,
    );
}
