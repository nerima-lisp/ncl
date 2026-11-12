//! Stress-tests function-cell publication while a symbol is redefined concurrently.

use std::sync::{Arc, Barrier};
use std::thread;

use ncl_object::typed::{FunctionDesignator, Symbol};
use ncl_object::{
    BuiltinFunctionCaller, Function, FunctionArguments, FunctionCaller, FunctionObject,
    HeapRootedWord, MultipleValues, ObjectError, Runtime, ThreadContext, Word, function_entry,
    make_code_object, make_simple_fun, make_string, make_symbol, symbol_function, symbol_offset,
};

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn concurrent_symbol_redefinition_and_native_calls_observe_complete_entries() {
    let fixture = prepare_function_cell_fixture();
    let barrier = Arc::new(Barrier::new(4));
    thread::scope(|scope| {
        let mut readers = Vec::new();
        for _ in 0..3 {
            let runtime = Arc::clone(&fixture.runtime);
            let barrier = Arc::clone(&barrier);
            let words = fixture.words;
            readers.push(scope.spawn(move || run_reader(&runtime, &barrier, &words)));
        }
        let runtime = Arc::clone(&fixture.runtime);
        let writer_barrier = Arc::clone(&barrier);
        let words = fixture.words;
        let writer = scope.spawn(move || run_writer(&runtime, &writer_barrier, &words));
        for reader in readers {
            reader.join().unwrap_or_else(|_| panic!("reader panicked"));
        }
        writer.join().unwrap_or_else(|_| panic!("writer panicked"));
    });
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn function_entry_accessors_validate_object_layouts() {
    let fixture = prepare_function_cell_fixture();
    let mut context = ThreadContext::new();
    context
        .register(&fixture.runtime)
        .unwrap_or_else(|error| panic!("register context: {error:?}"));

    let entry = Word::fixnum(37);
    {
        let mut access = context.thread_access();
        assert!(
            access.store_function_entry_release(decode_word(fixture.words.old_function), entry)
        );
        assert_eq!(
            access.load_function_entry_acquire(decode_word(fixture.words.old_function)),
            Some(entry)
        );
        assert!(!access.store_function_entry_release(decode_word(fixture.words.symbol), entry));
        assert_eq!(
            access.load_function_entry_acquire(decode_word(fixture.words.symbol)),
            None
        );
    }

    assert_eq!(
        function_entry(&context, Function::from_word(Word::NIL)),
        Err(ObjectError::TypeError)
    );
    let code = make_code_object(
        &mut context,
        &fixture.runtime,
        0,
        8,
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("code object: {error:?}"));
    assert!(matches!(
        make_simple_fun(
            &mut context,
            &fixture.runtime,
            usize::MAX,
            Word::NIL,
            Word::NIL,
            code,
        ),
        Err(ObjectError::Layout)
    ));
    assert_eq!(
        function_entry(
            &context,
            Function::from_word(decode_word(fixture.words.old_function)),
        ),
        Ok(37)
    );
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn symbol_function_slot_survives_two_full_collections() {
    let fixture = prepare_function_cell_fixture();
    let mut context = ThreadContext::new();
    context
        .register(&fixture.runtime)
        .unwrap_or_else(|error| panic!("register context: {error:?}"));
    let symbol = root_word(&context, fixture.words.symbol);
    let stale_function_bits = fixture.words.old_function;
    for _ in 0..2 {
        context
            .collect(true)
            .unwrap_or_else(|error| panic!("full collection: {error:?}"));
    }
    let stale_function = FunctionObject::try_from(decode_word(stale_function_bits))
        .unwrap_or_else(|error| panic!("stale function: {error:?}"));
    let current_function = symbol_function(&context, symbol.get())
        .unwrap_or_else(|error| panic!("symbol function after collection: {error:?}"));
    assert_ne!(current_function, stale_function.as_word());
    context
        .write_object_slot(
            symbol.get(),
            symbol_offset::FUNCTION,
            stale_function.as_word(),
        )
        .unwrap_or_else(|error| panic!("write stale function cell: {error:?}"));
    context.set_strict_forwarding(true);

    let mut caller = BuiltinFunctionCaller;
    let mut values = MultipleValues::new();
    assert_eq!(
        caller.call_function(
            &mut context,
            &fixture.runtime,
            FunctionDesignator::Function(stale_function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::ForwardingViolation)
    );
    assert_eq!(
        fixture
            .runtime
            .call_builtin_if_registered(&mut context, stale_function, &[], &mut values,),
        Some(Err(ObjectError::ForwardingViolation))
    );
    assert_eq!(
        caller.call_function(
            &mut context,
            &fixture.runtime,
            FunctionDesignator::Symbol(Symbol::from_word(symbol.get())),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ObjectError::ForwardingViolation)
    );
    context
        .write_object_slot(symbol.get(), symbol_offset::FUNCTION, current_function)
        .unwrap_or_else(|error| panic!("restore function cell: {error:?}"));
    let result = caller.call_function(
        &mut context,
        &fixture.runtime,
        FunctionDesignator::Symbol(Symbol::from_word(symbol.get())),
        FunctionArguments::new(&[]),
        &mut values,
    );
    assert_eq!(result, Ok(Word::fixnum(11)));
    assert_eq!(values.as_slice(), &[Word::fixnum(11)]);
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[derive(Clone, Copy)]
struct FunctionCellWords {
    symbol: u64,
    old_function: u64,
    new_function: u64,
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
struct FunctionCellFixture {
    runtime: Arc<Runtime>,
    words: FunctionCellWords,
    _code: [ncl_sys::CodePtr; 2],
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const fn encode_word(word: Word) -> u64 {
    !word.bits()
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const fn decode_word(encoded: u64) -> Word {
    Word::from_bits(!encoded)
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn root_word(context: &ThreadContext, encoded: u64) -> HeapRootedWord {
    HeapRootedWord::new(context, decode_word(encoded))
        .unwrap_or_else(|error| panic!("root word: {error:?}"))
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn prepare_function_cell_fixture() -> FunctionCellFixture {
    let runtime = Arc::new(Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}")));
    let mut setup = ThreadContext::new();
    setup
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register setup context: {error:?}"));
    let old_code = published_value_code(11);
    let new_code = published_value_code(29);
    let old_code_object = make_code_object(
        &mut setup,
        &runtime,
        old_code.address(),
        old_code.len(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("old code object: {error:?}"));
    let new_code_object = make_code_object(
        &mut setup,
        &runtime,
        new_code.address(),
        new_code.len(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("new code object: {error:?}"));
    let name = make_string(&mut setup, &runtime, &['f'])
        .unwrap_or_else(|error| panic!("symbol name: {error:?}"));
    let symbol =
        make_symbol(&mut setup, &runtime, name).unwrap_or_else(|error| panic!("symbol: {error:?}"));
    let old_function = make_simple_fun(
        &mut setup,
        &runtime,
        old_code.address(),
        Word::NIL,
        Word::NIL,
        old_code_object,
    )
    .unwrap_or_else(|error| panic!("old function: {error:?}"))
    .as_word();
    let new_function = make_simple_fun(
        &mut setup,
        &runtime,
        new_code.address(),
        Word::NIL,
        Word::NIL,
        new_code_object,
    )
    .unwrap_or_else(|error| panic!("new function: {error:?}"))
    .as_word();
    setup
        .write_object_slot(symbol, symbol_offset::FUNCTION, old_function)
        .unwrap_or_else(|error| panic!("initial function cell: {error:?}"));
    drop(setup);

    FunctionCellFixture {
        runtime,
        words: FunctionCellWords {
            symbol: encode_word(symbol),
            old_function: encode_word(old_function),
            new_function: encode_word(new_function),
        },
        _code: [old_code, new_code],
    }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn run_reader(runtime: &Runtime, barrier: &Barrier, words: &FunctionCellWords) {
    let mut context = ThreadContext::new();
    context
        .register(runtime)
        .unwrap_or_else(|error| panic!("register reader: {error:?}"));
    barrier.wait();
    for _ in 0..5_000 {
        let function = symbol_function(&context, decode_word(words.symbol))
            .unwrap_or_else(|error| panic!("read symbol function: {error:?}"));
        let expected = if function == decode_word(words.old_function) {
            11
        } else if function == decode_word(words.new_function) {
            29
        } else {
            panic!("observed an unpublished function object");
        };
        let entry = function_entry(&context, Function::from_word(function))
            .unwrap_or_else(|error| panic!("read function entry: {error:?}"));
        let thread = context.thread_access().as_non_null();
        let (value, count) = ncl_sys::invoke_entry_with_function_address(
            entry,
            thread.as_ptr(),
            function.bits(),
            0,
            [0; 4],
            0,
        );
        assert_eq!(Word::from_bits(value).as_fixnum(), Some(expected));
        assert_eq!(count, 1);
    }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn run_writer(runtime: &Runtime, barrier: &Barrier, words: &FunctionCellWords) {
    let mut context = ThreadContext::new();
    context
        .register(runtime)
        .unwrap_or_else(|error| panic!("register writer: {error:?}"));
    barrier.wait();
    for index in 0..10_000 {
        let function = if index % 2 == 0 {
            decode_word(words.new_function)
        } else {
            decode_word(words.old_function)
        };
        context
            .write_object_slot(decode_word(words.symbol), symbol_offset::FUNCTION, function)
            .unwrap_or_else(|error| panic!("redefine symbol function: {error:?}"));
    }
}

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn published_value_code(value: u16) -> ncl_sys::CodePtr {
    #[cfg(target_arch = "x86_64")]
    let bytes = {
        let fixnum = u32::from(value) * 2;
        let mut bytes = [0; 11];
        bytes[0] = 0xb8;
        bytes[1..5].copy_from_slice(&fixnum.to_le_bytes());
        bytes[5] = 0xba;
        bytes[6..10].copy_from_slice(&1_u32.to_le_bytes());
        bytes[10] = 0xc3;
        bytes
    };
    #[cfg(target_arch = "aarch64")]
    let bytes = {
        let mov_value = 0xd280_0000_u32 | ((u32::from(value) * 2) << 5);
        let mut bytes = [0; 12];
        bytes[..4].copy_from_slice(&mov_value.to_le_bytes());
        bytes[4..8].copy_from_slice(&0xd280_0021_u32.to_le_bytes());
        bytes[8..].copy_from_slice(&0xd65f_03c0_u32.to_le_bytes());
        bytes
    };
    let mut code = ncl_sys::alloc_code(bytes.len())
        .unwrap_or_else(|error| panic!("allocate native code: {error:?}"));
    ncl_sys::write_code(&mut code, 0, &bytes)
        .unwrap_or_else(|error| panic!("write native code: {error:?}"));
    ncl_sys::publish_code(&mut code)
        .unwrap_or_else(|error| panic!("publish native code: {error:?}"));
    code
}
