use super::*;

#[test]
fn published_machine_code_returns_42() {
    #[cfg(target_arch = "aarch64")]
    let bytes = [0x40, 0x05, 0x80, 0xd2, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "x86_64")]
    let bytes = [0xb8, 0x2a, 0, 0, 0, 0xc3];
    let Ok(mut code) = alloc_code(bytes.len()) else {
        return;
    };
    assert!(code.write_code(0, &bytes).is_ok());
    assert!(publish_code(&mut code).is_ok());
    let mut thread = crate::Thread::new();
    let (value, _count) = crate::invoke_entry(&code, 0, &raw mut thread, 0, [0; 4], 0);
    assert_eq!(value, 42);
}

/// Generated code reads the pinned context register and the callee function
/// object that a native call supplies, so the invoke entry must deliver both.
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
#[test]
fn invoke_entry_delivers_context_and_function_object() {
    #[cfg(target_arch = "aarch64")]
    let context_bytes = [0xe0, 0x03, 0x15, 0xaa, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "aarch64")]
    let function_bytes = [0xe0, 0x03, 0x10, 0xaa, 0xc0, 0x03, 0x5f, 0xd6];
    #[cfg(target_arch = "x86_64")]
    let context_bytes = [0x4c, 0x89, 0xf8, 0xc3];
    #[cfg(target_arch = "x86_64")]
    let function_bytes = [0x4c, 0x89, 0xd0, 0xc3];
    let mut thread = crate::Thread::new();
    let context = &raw mut thread;
    let function_object = 0x1234_5678_9abc_def0_u64;

    let Ok(mut context_code) = alloc_code(context_bytes.len()) else {
        return;
    };
    assert!(context_code.write_code(0, &context_bytes).is_ok());
    assert!(publish_code(&mut context_code).is_ok());
    let (returned_context, _) = crate::invoke_entry(&context_code, 0, context, 0, [0; 4], 0);
    assert_eq!(returned_context, context as usize as u64);

    let Ok(mut function_code) = alloc_code(function_bytes.len()) else {
        return;
    };
    assert!(function_code.write_code(0, &function_bytes).is_ok());
    assert!(publish_code(&mut function_code).is_ok());
    let (returned_function, _) = crate::invoke_entry_with_function(
        &function_code,
        0,
        context,
        function_object,
        0,
        [0; 4],
        0,
    );
    assert_eq!(returned_function, function_object);
}
