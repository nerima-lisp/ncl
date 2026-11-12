//! Strict-forwarding diagnostics for FUNCALL and APPLY.

use ncl_lib_macros::register;
use ncl_object::{
    Arity, Builtin, BuiltinArgs, BuiltinConvention, BuiltinIdentifier, BuiltinImplementation,
    BuiltinName, BuiltinPackage, FunctionObject, LambdaList, MultipleValues, ObjectError, Package,
    Runtime, ThreadContext, Word, symbol_function,
};
use std::sync::Arc;

#[expect(
    clippy::unnecessary_wraps,
    reason = "BuiltinImplementation::direct requires the Result callback signature"
)]
const fn stale_target(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    Ok(Word::NIL)
}

const fn encode_word(word: Word) -> u64 {
    !word.bits()
}

const fn decode_word(encoded: u64) -> Word {
    Word::from_bits(!encoded)
}

fn register_stale_target(runtime: &Arc<Runtime>) -> u64 {
    let mut setup = ThreadContext::new();
    setup
        .register(runtime)
        .unwrap_or_else(|error| panic!("register setup context: {error:?}"));
    register(runtime).unwrap_or_else(|error| panic!("register FUNCALL/APPLY: {error:?}"));
    let function = runtime
        .register_builtin(
            &mut setup,
            BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("STALE-CALL")),
            BuiltinImplementation::direct(
                Builtin {
                    lambda_list: LambdaList::fixed(&[]),
                    convention: BuiltinConvention::Direct(Arity::exact(0)),
                },
                stale_target,
            ),
        )
        .unwrap_or_else(|error| panic!("register stale target: {error:?}"));
    encode_word(function.as_word())
}

#[test]
fn funcall_and_apply_report_forwarding_violation_for_stale_function_designators() {
    let runtime = Arc::new(Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}")));
    let setup_runtime = Arc::clone(&runtime);
    let stale_function_bits = std::thread::spawn(move || register_stale_target(&setup_runtime))
        .join()
        .unwrap_or_else(|error| panic!("setup thread: {error:?}"));

    // Keep the setup thread's raw Lisp words out of the collector's conservative stack scan.
    let collection_runtime = Arc::clone(&runtime);
    std::thread::spawn(move || {
        let mut collector = ThreadContext::new();
        collector
            .register(&collection_runtime)
            .unwrap_or_else(|error| panic!("register collector context: {error:?}"));
        collector
            .collect(true)
            .unwrap_or_else(|error| panic!("full collection: {error:?}"));
    })
    .join()
    .unwrap_or_else(|error| panic!("collector thread: {error:?}"));

    let mut ctx = ThreadContext::new();
    ctx.register(&runtime)
        .unwrap_or_else(|error| panic!("register caller context: {error:?}"));
    let package = runtime
        .find_package(&ctx, "NCL-TEST")
        .unwrap_or_else(|| panic!("missing NCL-TEST package"));
    let (symbol, _) = Package::from_word(package)
        .intern(&mut ctx, &runtime, "STALE-CALL")
        .unwrap_or_else(|error| panic!("find stale target symbol: {error:?}"));
    let stale_function = decode_word(stale_function_bits);
    let current_function = symbol_function(&ctx, symbol)
        .unwrap_or_else(|error| panic!("read current function: {error:?}"));
    assert_ne!(current_function, stale_function);

    let funcall = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "FUNCALL")
            .unwrap_or_else(|| panic!("missing FUNCALL builtin")),
    )
    .unwrap_or_else(|error| panic!("convert FUNCALL: {error:?}"));
    let apply = FunctionObject::try_from(
        runtime
            .function(&mut ctx, "COMMON-LISP", "APPLY")
            .unwrap_or_else(|| panic!("missing APPLY builtin")),
    )
    .unwrap_or_else(|error| panic!("convert APPLY: {error:?}"));

    ctx.write_object_slot(symbol, ncl_object::symbol_offset::FUNCTION, stale_function)
        .unwrap_or_else(|error| panic!("write stale function cell: {error:?}"));
    ctx.set_strict_forwarding(true);
    assert_eq!(
        ctx.validate_forwarding(stale_function),
        Err(ObjectError::ForwardingViolation)
    );
    assert_eq!(
        ctx.validate_forwarding(
            symbol_function(&ctx, symbol)
                .unwrap_or_else(|error| panic!("read stale function cell: {error:?}"))
        ),
        Err(ObjectError::ForwardingViolation)
    );

    assert_eq!(
        runtime.call_builtin(&mut ctx, funcall, &[stale_function]),
        Err(ObjectError::ForwardingViolation)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, apply, &[stale_function, Word::NIL]),
        Err(ObjectError::ForwardingViolation)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, funcall, &[symbol]),
        Err(ObjectError::ForwardingViolation)
    );
    assert_eq!(
        runtime.call_builtin(&mut ctx, apply, &[symbol, Word::NIL]),
        Err(ObjectError::ForwardingViolation)
    );
}
