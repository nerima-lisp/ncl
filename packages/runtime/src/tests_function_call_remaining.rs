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
