use super::*;

#[test]
fn runtime_function_caller_preserves_unset_and_explicit_empty_results() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let function = register_empty_values_builtin(&mut runtime);
    let mut caller = RuntimeFunctionCaller;
    let mut values = MultipleValues::new();

    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[Word::NIL]),
            &mut values,
        ),
        Ok(Word::fixnum(7))
    );
    assert!(!values.is_explicit());
    assert_eq!(runtime.context.values(), &[Word::fixnum(7)]);

    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[Word::TRUE]),
            &mut values,
        ),
        Ok(Word::fixnum(7))
    );
    assert!(values.is_explicit());
    assert!(values.is_empty());
    assert!(runtime.context.values().is_empty());
}

#[test]
fn successful_registered_builtin_clears_reused_values_buffer() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    let function = register_builtin(&mut runtime);
    let arguments = [Word::fixnum(2), Word::fixnum(3)];
    let mut caller = RuntimeFunctionCaller;
    let mut values = MultipleValues::new();
    values.set(&[Word::fixnum(7), Word::fixnum(8)]);

    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&arguments),
            &mut values,
        ),
        Ok(Word::fixnum(5))
    );
    assert!(values.is_empty());
}

#[test]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn calls_a_published_simple_fun_through_the_native_abi() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    #[cfg(target_arch = "x86_64")]
    let machine_code = [
        0xb8, 0x54, 0, 0, 0, // mov eax, (42 << 1)
        0xba, 1, 0, 0, 0,    // mov edx, 1
        0xc3, // ret
    ];
    #[cfg(target_arch = "aarch64")]
    let machine_code = [
        0x80, 0x0a, 0x80, 0xd2, // mov x0, #84
        0x21, 0x00, 0x80, 0xd2, // mov x1, #1
        0xc0, 0x03, 0x5f, 0xd6, // ret
    ];
    let mut native = ncl_sys::alloc_code(machine_code.len()).unwrap();
    ncl_sys::write_code(&mut native, 0, &machine_code).unwrap();
    ncl_sys::publish_code(&mut native).unwrap();
    let code = make_code_object(
        &mut runtime.context,
        &runtime.object,
        0,
        machine_code.len(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("code object: {error:?}"));
    let closure = make_closure(
        &mut runtime.context,
        &runtime.object,
        native.address(),
        Word::NIL,
        Word::NIL,
        code,
        &[],
    )
    .unwrap_or_else(|error| panic!("closure: {error:?}"));
    let function = ncl_object::FunctionObject::try_from(closure.as_word())
        .unwrap_or_else(|error| panic!("function object: {error:?}"));
    let mut caller = RuntimeFunctionCaller;
    let mut values = MultipleValues::new();

    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Ok(Word::fixnum(42))
    );
    assert_eq!(values.as_slice(), &[Word::fixnum(42)]);
}

#[test]
fn macro_call_rejects_a_forwarded_function_under_strict_forwarding() {
    let runtime = std::sync::Arc::new(
        ncl_object::Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}")),
    );
    let setup_runtime = std::sync::Arc::clone(&runtime);
    let stale_function_bits = std::thread::spawn(move || {
        let mut setup = ncl_object::ThreadContext::new();
        setup
            .register(&setup_runtime)
            .unwrap_or_else(|error| panic!("register setup context: {error:?}"));
        let function = setup_runtime
            .register_builtin(
                &mut setup,
                ncl_object::BuiltinIdentifier::new(
                    ncl_object::BuiltinPackage::NclTest,
                    ncl_object::BuiltinName::new("STALE-MACRO"),
                ),
                ncl_object::BuiltinImplementation::direct(
                    ncl_object::Builtin {
                        lambda_list: ncl_object::LambdaList::fixed(&[]),
                        convention: ncl_object::BuiltinConvention::Direct(
                            ncl_object::Arity::exact(0),
                        ),
                    },
                    stale_macro_target,
                ),
            )
            .unwrap_or_else(|error| panic!("register stale macro: {error:?}"));
        !function.as_word().bits()
    })
    .join()
    .unwrap_or_else(|error| panic!("setup thread: {error:?}"));

    let collection_runtime = std::sync::Arc::clone(&runtime);
    std::thread::spawn(move || {
        let mut collector = ncl_object::ThreadContext::new();
        collector
            .register(&collection_runtime)
            .unwrap_or_else(|error| panic!("register collector context: {error:?}"));
        collector
            .collect(true)
            .unwrap_or_else(|error| panic!("full collection: {error:?}"));
    })
    .join()
    .unwrap_or_else(|error| panic!("collector thread: {error:?}"));

    let mut caller = ncl_object::ThreadContext::new();
    caller
        .register(&runtime)
        .unwrap_or_else(|error| panic!("register caller context: {error:?}"));
    let stale_function = Word::from_bits(!stale_function_bits);
    let current_function = runtime
        .function(&mut caller, "NCL-TEST", "STALE-MACRO")
        .unwrap_or_else(|| panic!("missing stale macro builtin"));
    assert_ne!(current_function, stale_function);
    caller.set_strict_forwarding(true);

    let entry_codes = std::collections::BTreeMap::<usize, (Box<Word>, ncl_sys::RootToken)>::new();
    assert_eq!(
        crate::support::call_macro_function(
            &mut caller,
            &runtime,
            FunctionObject::try_from(stale_function)
                .unwrap_or_else(|error| panic!("stale function object: {error:?}")),
            &[],
            &entry_codes,
        ),
        Err(ncl_object::ObjectError::ForwardingViolation)
    );
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "BuiltinImplementation::direct requires the Result callback signature"
)]
fn stale_macro_target(
    _ctx: &mut ncl_object::ThreadContext,
    _runtime: &ncl_object::Runtime,
    _args: &ncl_object::BuiltinArgs<'_>,
    _values: &mut ncl_object::MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    Ok(Word::NIL)
}

#[test]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
fn native_call_reports_pending_non_local_exit_after_preserving_values() {
    let mut runtime = Runtime::new().unwrap_or_else(|error| panic!("runtime: {error:?}"));
    #[cfg(target_arch = "x86_64")]
    let machine_code = [
        0xb8, 0x54, 0, 0, 0, // mov eax, (42 << 1)
        0xba, 1, 0, 0, 0,    // mov edx, 1
        0xc3, // ret
    ];
    #[cfg(target_arch = "aarch64")]
    let machine_code = [
        0x80, 0x0a, 0x80, 0xd2, // mov x0, #84
        0x21, 0x00, 0x80, 0xd2, // mov x1, #1
        0xc0, 0x03, 0x5f, 0xd6, // ret
    ];
    let mut native = ncl_sys::alloc_code(machine_code.len()).unwrap();
    ncl_sys::write_code(&mut native, 0, &machine_code).unwrap();
    ncl_sys::publish_code(&mut native).unwrap();
    let code = make_code_object(
        &mut runtime.context,
        &runtime.object,
        0,
        machine_code.len(),
        Word::NIL,
        Word::NIL,
        Word::NIL,
    )
    .unwrap_or_else(|error| panic!("code object: {error:?}"));
    let closure = make_closure(
        &mut runtime.context,
        &runtime.object,
        native.address(),
        Word::NIL,
        Word::NIL,
        code,
        &[],
    )
    .unwrap_or_else(|error| panic!("closure: {error:?}"));
    let function = ncl_object::FunctionObject::try_from(closure.as_word())
        .unwrap_or_else(|error| panic!("function object: {error:?}"));
    let mut caller = RuntimeFunctionCaller;
    let mut values = MultipleValues::new();
    runtime
        .context
        .set_multiple_value_area(&[Word::fixnum(42)])
        .unwrap_or_else(|error| panic!("multiple value area: {error:?}"));
    runtime.context.set_non_local_exit(true);

    assert_eq!(
        caller.call_function(
            &mut runtime.context,
            &runtime.object,
            FunctionDesignator::Function(function),
            FunctionArguments::new(&[]),
            &mut values,
        ),
        Err(ncl_object::ObjectError::NonLocalExit)
    );
    assert_eq!(values.as_slice(), &[Word::fixnum(42)]);
    assert!(runtime.context.take_non_local_exit());
}
